//! The Cloudflare side of the public ingress, as steps that can be planned, applied and rolled
//! back: Origin CA certificate, Authenticated Origin Pulls with a client CA that belongs to nie
//! alone, the edge configuration, the firewall (Cloudflare ranges only), the SSL mode and origin
//! port rules of the hostname, and finally the proxied DNS record that switches traffic.
//!
//! `plan` is offline. `apply` and `rollback` call the Cloudflare API with
//! `CLOUDFLARE_API_TOKEN` and write under the edge directory; nothing runs unless asked.

use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyUsagePurpose, date_time_ymd,
};
use serde_json::{Value, json};

use crate::host::{SERVICE_ID, chown, set_mode, write_if_changed};
use crate::keys::P256Key;
use crate::nftables;

/// Production API base.
pub const API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// The ordered steps of the switch. `Dns` is the one that moves traffic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Step {
    /// Origin CA certificate and key for the hostname.
    OriginCert,
    /// Client CA, client certificate uploaded to Cloudflare, hostname-level Authenticated Origin Pulls.
    ClientAuth,
    /// `edge.toml` with the TLS listener.
    EdgeConfig,
    /// Firewall: the origin port open to Cloudflare ranges only.
    Firewall,
    /// SSL mode `strict` for the hostname (configuration rule).
    SslMode,
    /// Origin rule: Cloudflare connects to the origin port.
    OriginRule,
    /// Proxied A record to the origin IP (replaces the tunnel CNAME).
    Dns,
}

impl Step {
    /// Every step in execution order.
    pub const ALL: [Step; 7] = [
        Step::OriginCert,
        Step::ClientAuth,
        Step::EdgeConfig,
        Step::Firewall,
        Step::SslMode,
        Step::OriginRule,
        Step::Dns,
    ];

    /// The name used on the command line.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Step::OriginCert => "origin-cert",
            Step::ClientAuth => "client-auth",
            Step::EdgeConfig => "edge-config",
            Step::Firewall => "firewall",
            Step::SslMode => "ssl-mode",
            Step::OriginRule => "origin-rule",
            Step::Dns => "dns",
        }
    }
}

/// Everything the steps need.
#[derive(Clone, Debug)]
pub struct Params {
    /// Zone name, `aphrody.app`.
    pub zone_name: String,
    /// Public hostname, `nie.aphrody.app`.
    pub hostname: String,
    /// Origin TLS port.
    pub port: u16,
    /// Public IPv4 of the origin.
    pub origin_ip: String,
    /// `/etc/nie/edge`.
    pub edge_dir: PathBuf,
    /// Origin CA validity in days (7, 30, 90, 365, 730, 1095, 5475).
    pub validity_days: u32,
    /// Cloudflare API base.
    pub api_base: String,
    /// nftables table of an existing firewall to extend (`inet fleet`); own table when absent.
    pub firewall_table: Option<String>,
}

const REF_SSL: &str = "nie_edge_ssl_strict";
const REF_PORT: &str = "nie_edge_origin_port";
const STATE_FILE: &str = "cloudflare-state.json";

/// The default zone of a hostname: its last two labels.
#[must_use]
pub fn zone_of(hostname: &str) -> String {
    let labels: Vec<&str> = hostname.split('.').collect();
    labels[labels.len().saturating_sub(2)..].join(".")
}

/// The IPv4 address the host uses to reach the internet.
pub fn discover_origin_ip() -> Result<String> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("1.1.1.1:53")?;
    Ok(socket.local_addr()?.ip().to_string())
}

// ---------------------------------------------------------------------------------------------
// plan

/// The plan as text: every step with the exact calls, files and rollback.
#[must_use]
pub fn plan(p: &Params) -> String {
    let h = &p.hostname;
    let z = &p.zone_name;
    let d = p.edge_dir.display();
    let port = p.port;
    let ip = &p.origin_ip;
    let fw = p.firewall_table.as_deref().map_or_else(
        || "own table `inet nie_edge`".to_owned(),
        |t| format!("rule inserted in the input chain of `{t}`"),
    );
    format!(
        "Cloudflare ingress plan for {h} (zone {z}), origin {ip}:{port}\n\
         token: CLOUDFLARE_API_TOKEN with SSL and Certificates Write, DNS Write, Origin Write,\n\
         \x20      Config Settings Write, Zone Read, Zone Settings Read (zone {z} only)\n\n\
         1 origin-cert   generate an ECDSA P-256 key and CSR locally, POST /certificates\n\
         \x20               {{hostnames:[\"{h}\"], request_type:\"origin-ecc\", requested_validity:{validity}, csr}}\n\
         \x20               writes {d}/origin.pem (0644) and {d}/origin.key (0600, uid {uid})\n\
         \x20               rollback: none needed (the certificate is only used once the listener runs)\n\
         2 client-auth   generate a private client CA and a client certificate (EKU clientAuth); the CA key is\n\
         \x20               dropped after signing. POST /zones/{{zone}}/origin_tls_client_auth/hostnames/certificates\n\
         \x20               {{certificate: client chain, private_key}} then PUT .../origin_tls_client_auth/hostnames\n\
         \x20               {{config:[{{hostname:\"{h}\", cert_id, enabled:true}}]}}. {d}/origin-pull-ca.pem holds only the CA:\n\
         \x20               nie-edge accepts a client certificate signed by it and nothing else (the shared Cloudflare\n\
         \x20               origin-pull CA would admit any Cloudflare customer's zone).\n\
         \x20               rollback: PUT .../hostnames {{config:[{{hostname, cert_id, enabled:false}}]}}\n\
         3 edge-config   write {d}/edge.toml with [listen.tls] addr 0.0.0.0:{port}, cert, key, client_ca, hosts [\"{h}\"];\n\
         \x20               `nie-edge check`, then the supervised `nie-edge` service restarts (`nie-deploy up`)\n\
         \x20               rollback: restore the previous edge.toml (kept as edge.toml.bak)\n\
         4 firewall      {fw}: tcp dport {port} accepted from the Cloudflare ranges (GET /ips) only\n\
         \x20               (prd's `inet fleet` chain ends in a drop, so the accept rule must live in that table)\n\
         \x20               rollback: `nie-deploy cloudflare rollback --step firewall`\n\
         5 ssl-mode      GET /zones/{{zone}}/settings/ssl; when not strict, configuration rule `{REF_SSL}` in phase\n\
         \x20               http_config_settings: (http.host eq \"{h}\") -> set_config {{ssl:\"strict\"}} (an Origin CA\n\
         \x20               certificate needs Full strict)\n\
         \x20               rollback: remove rule `{REF_SSL}`\n\
         6 origin-rule   rule `{REF_PORT}` in phase http_request_origin: (http.host eq \"{h}\") ->\n\
         \x20               route {{origin:{{port:{port}}}}}; the other rules of the phase are kept\n\
         \x20               rollback: remove rule `{REF_PORT}`\n\
         7 dns           the switch: replace the proxied tunnel CNAME of {h} by a proxied A record to {ip} (ttl auto);\n\
         \x20               the previous record is saved in {d}/{STATE_FILE}\n\
         \x20               rollback: `nie-deploy cloudflare rollback --step dns` restores it (cloudflared keeps running\n\
         \x20               until the tunnel is deleted, so rollback is immediate)\n\n\
         before step 7: `nie-edge check`; `curl --resolve {h}:{port}:{ip} https://{h}:{port}/health` must fail the\n\
         handshake (no client certificate); `nie-deploy cloudflare status` must show steps 1 to 6 done.\n\
         after step 7: https://{h}/health answers 200 {{\"service\":\"nie-edge\"}}; keep the tunnel for the rollback\n\
         window, then remove the supervised cloudflared service and the tunnel.\n",
        validity = p.validity_days,
        uid = SERVICE_ID,
    )
}

// ---------------------------------------------------------------------------------------------
// certificates

fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let y = (yoe + era * 400 + i64::from(m <= 2)) as i32;
    (y, m, d)
}

/// Calendar date `days` from today (UTC).
fn ymd_in(days: i64) -> (i32, u8, u8) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    civil_from_days(now.div_euclid(86_400) + days)
}

macro_rules! date_in {
    ($days:expr) => {{
        let (y, m, d) = ymd_in($days);
        date_time_ymd(y, m, d)
    }};
}

/// A new origin key and a CSR for `hostname`, both PEM.
pub fn origin_csr(hostname: &str) -> Result<(String, String)> {
    let key = P256Key::generate()?;
    let mut params = CertificateParams::new(vec![hostname.to_owned()])?;
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, hostname);
    let csr = params.serialize_request(&key)?.pem()?;
    Ok((key.private_pem()?, csr))
}

/// The client side of Authenticated Origin Pulls.
pub struct ClientAuth {
    /// The private CA certificate nie-edge trusts.
    pub ca_pem: String,
    /// Client certificate followed by the CA certificate: what Cloudflare presents.
    pub chain_pem: String,
    /// The client private key.
    pub key_pem: String,
}

/// A fresh client CA and a client certificate signed by it. The CA key is not returned.
pub fn client_auth(hostname: &str) -> Result<ClientAuth> {
    let ca_key = P256Key::generate()?;
    let mut ca_params = CertificateParams::new(Vec::<String>::new())?;
    ca_params.distinguished_name = DistinguishedName::new();
    ca_params
        .distinguished_name
        .push(DnType::CommonName, format!("nie origin-pull CA {hostname}"));
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    ca_params.not_before = date_in!(-1);
    ca_params.not_after = date_in!(3650);
    let ca_cert = ca_params.self_signed(&ca_key)?;

    let leaf_key = P256Key::generate()?;
    let mut leaf = CertificateParams::new(Vec::<String>::new())?;
    leaf.distinguished_name = DistinguishedName::new();
    leaf.distinguished_name
        .push(DnType::CommonName, format!("cloudflare origin pull {hostname}"));
    leaf.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    leaf.not_before = date_in!(-1);
    leaf.not_after = date_in!(3650);
    let issuer = Issuer::new(ca_params, ca_key);
    let leaf_cert = leaf.signed_by(&leaf_key, &issuer)?;
    Ok(ClientAuth {
        ca_pem: ca_cert.pem(),
        chain_pem: format!("{}{}", leaf_cert.pem(), ca_cert.pem()),
        key_pem: leaf_key.private_pem()?,
    })
}

/// A self-signed origin certificate and key for local tests of the TLS listener.
pub fn self_signed_origin(hostname: &str) -> Result<(String, String)> {
    let key = P256Key::generate()?;
    let params = CertificateParams::new(vec![hostname.to_owned()])?;
    let cert = params.self_signed(&key)?;
    Ok((cert.pem(), key.private_pem()?))
}

// ---------------------------------------------------------------------------------------------
// firewall

/// The nftables script for the origin port. With `table = Some("inet fleet")` the Cloudflare sets
/// go into that existing table (whose input chain ends in a drop, so the accept rules are
/// inserted there by [`nft_insert_rules`]); otherwise a table of its own holds sets, accept rules
/// for Cloudflare and a drop for everyone else.
#[must_use]
pub fn nft_script(port: u16, v4: &[String], v6: &[String], table: Option<&str>) -> String {
    let (name, own) = table.map_or(("inet nie_edge", true), |t| (t, false));
    let mut s = format!(
        "add table {name}\n\
         add set {name} nie_cf4 {{ type ipv4_addr; flags interval; }}\n\
         add set {name} nie_cf6 {{ type ipv6_addr; flags interval; }}\n\
         flush set {name} nie_cf4\n\
         flush set {name} nie_cf6\n\
         add element {name} nie_cf4 {{ {} }}\n\
         add element {name} nie_cf6 {{ {} }}\n",
        v4.join(", "),
        v6.join(", ")
    );
    if own {
        s.push_str(&format!(
            "add chain {name} input {{ type filter hook input priority -20; policy accept; }}\n\
             flush chain {name} input\n\
             add rule {name} input tcp dport {port} ip saddr @nie_cf4 accept comment \"nie-edge\"\n\
             add rule {name} input tcp dport {port} ip6 saddr @nie_cf6 accept comment \"nie-edge\"\n\
             add rule {name} input tcp dport {port} drop comment \"nie-edge\"\n"
        ));
    }
    s
}

/// The `insert rule` lines of the shared-table mode.
#[must_use]
pub fn nft_insert_rules(port: u16, table: &str) -> [String; 2] {
    [
        format!("insert rule {table} input tcp dport {port} ip saddr @nie_cf4 accept comment \"nie-edge\""),
        format!("insert rule {table} input tcp dport {port} ip6 saddr @nie_cf6 accept comment \"nie-edge\""),
    ]
}

/// The comment every nie firewall rule carries; deletion finds the rules by it.
const NFT_COMMENT: &str = "nie-edge";

fn apply_firewall(p: &Params, ranges: &(Vec<String>, Vec<String>)) -> Result<()> {
    nftables::apply(&nft_script(p.port, &ranges.0, &ranges.1, p.firewall_table.as_deref()))?;
    if let Some(table) = &p.firewall_table {
        nftables::delete_commented_rules(table, "input", NFT_COMMENT)?;
        nftables::apply(&nft_insert_rules(p.port, table).join("\n"))?;
    }
    Ok(())
}

fn remove_firewall(p: &Params) -> Result<()> {
    match &p.firewall_table {
        Some(table) => nftables::delete_commented_rules(table, "input", NFT_COMMENT).map(drop),
        None => nftables::delete_table("inet nie_edge"),
    }
}

// ---------------------------------------------------------------------------------------------
// API

/// A blocking Cloudflare API client.
pub struct Api {
    base: String,
    token: Option<String>,
    http: reqwest::blocking::Client,
}

impl Api {
    /// A client; `token` may be absent for the public endpoints.
    pub fn new(base: &str, token: Option<String>) -> Result<Self> {
        nie_tls::install();
        Ok(Self {
            base: base.trim_end_matches('/').to_owned(),
            token,
            http: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("nie-deploy")
                .build()?,
        })
    }

    /// One call; returns the `result` of the envelope, `None` on HTTP 404.
    pub fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Option<Value>> {
        let method = reqwest::Method::from_bytes(method.as_bytes())?;
        let mut req = self.http.request(method.clone(), format!("{}{path}", self.base));
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        if let Some(body) = body {
            req = req
                .header("Content-Type", "application/json")
                .body(serde_json::to_vec(body)?);
        }
        let response = req.send().with_context(|| format!("{method} {path}"))?;
        let status = response.status();
        let text = response.text()?;
        if status.as_u16() == 404 {
            return Ok(None);
        }
        let envelope: Value = serde_json::from_str(&text)
            .with_context(|| format!("{method} {path}: HTTP {status}: {text:.200}"))?;
        if !status.is_success() || envelope["success"] == json!(false) {
            bail!("{method} {path}: HTTP {status}: {}", envelope["errors"]);
        }
        Ok(Some(envelope["result"].clone()))
    }

    fn need(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
        self.call(method, path, body)?
            .ok_or_else(|| anyhow!("{method} {path}: not found"))
    }

    fn zone_id(&self, name: &str) -> Result<String> {
        let zones = self.need("GET", &format!("/zones?name={name}"), None)?;
        zones[0]["id"]
            .as_str()
            .map(str::to_owned)
            .with_context(|| format!("zone {name} not found for this token"))
    }

    fn ranges(&self) -> Result<(Vec<String>, Vec<String>)> {
        let ips = self.need("GET", "/ips", None)?;
        let list = |k: &str| -> Vec<String> {
            ips[k]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                .unwrap_or_default()
        };
        let (v4, v6) = (list("ipv4_cidrs"), list("ipv6_cidrs"));
        if v4.is_empty() || v6.is_empty() {
            bail!("Cloudflare returned no IP ranges");
        }
        Ok((v4, v6))
    }

    fn upsert_rule(&self, zone: &str, phase: &str, rule: &Value) -> Result<()> {
        let path = format!("/zones/{zone}/rulesets/phases/{phase}/entrypoint");
        let mut rules = self.existing_rules(&path)?;
        rules.retain(|r| r["ref"] != rule["ref"]);
        rules.push(rule.clone());
        self.need("PUT", &path, Some(&json!({ "rules": rules })))?;
        Ok(())
    }

    fn remove_rule(&self, zone: &str, phase: &str, reference: &str) -> Result<()> {
        let path = format!("/zones/{zone}/rulesets/phases/{phase}/entrypoint");
        let mut rules = self.existing_rules(&path)?;
        let before = rules.len();
        rules.retain(|r| r["ref"] != json!(reference));
        if rules.len() != before {
            self.need("PUT", &path, Some(&json!({ "rules": rules })))?;
        }
        Ok(())
    }

    fn existing_rules(&self, path: &str) -> Result<Vec<Value>> {
        let current = self.call("GET", path, None)?;
        Ok(current
            .and_then(|c| c["rules"].as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .map(|mut r| {
                if let Some(o) = r.as_object_mut() {
                    for k in ["version", "last_updated"] {
                        o.remove(k);
                    }
                }
                r
            })
            .collect())
    }
}

fn read_state(dir: &Path) -> Value {
    std::fs::read_to_string(dir.join(STATE_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}))
}

fn write_state(dir: &Path, state: &Value) -> Result<()> {
    write_if_changed(
        &dir.join(STATE_FILE),
        serde_json::to_string_pretty(state)?.as_bytes(),
        0o600,
    )?;
    Ok(())
}

fn write_secret(path: &Path, content: &str, mode: u32) -> Result<()> {
    write_if_changed(path, content.as_bytes(), mode)?;
    set_mode(path, mode)?;
    let _ = chown(path, SERVICE_ID, SERVICE_ID);
    Ok(())
}

fn host_expr(hostname: &str) -> String {
    format!("(http.host eq \"{hostname}\")")
}

fn dns_body(record: &Value) -> Value {
    json!({
        "type": record["type"], "name": record["name"], "content": record["content"],
        "proxied": record["proxied"], "ttl": record["ttl"],
    })
}

// ---------------------------------------------------------------------------------------------
// apply, rollback, status

/// Run one step. Returns a one-line report.
pub fn apply(p: &Params, step: Step, token: Option<String>) -> Result<String> {
    let api = Api::new(&p.api_base, token)?;
    let dir = &p.edge_dir;
    std::fs::create_dir_all(dir)?;
    match step {
        Step::OriginCert => {
            if dir.join("origin.pem").exists() && dir.join("origin.key").exists() {
                return Ok("origin certificate present, kept".into());
            }
            let (key, csr) = origin_csr(&p.hostname)?;
            let cert = api.need(
                "POST",
                "/certificates",
                Some(&json!({
                    "hostnames": [p.hostname],
                    "request_type": "origin-ecc",
                    "requested_validity": p.validity_days,
                    "csr": csr,
                })),
            )?;
            let pem = cert["certificate"].as_str().context("no certificate in the answer")?;
            write_secret(&dir.join("origin.key"), &key, 0o600)?;
            write_secret(&dir.join("origin.pem"), pem, 0o644)?;
            Ok(format!("origin certificate {} written", cert["id"]))
        }
        Step::ClientAuth => {
            let zone = api.zone_id(&p.zone_name)?;
            let auth = client_auth(&p.hostname)?;
            let uploaded = api.need(
                "POST",
                &format!("/zones/{zone}/origin_tls_client_auth/hostnames/certificates"),
                Some(&json!({ "certificate": auth.chain_pem, "private_key": auth.key_pem })),
            )?;
            let id = uploaded["id"].as_str().context("no certificate id")?.to_owned();
            api.need(
                "PUT",
                &format!("/zones/{zone}/origin_tls_client_auth/hostnames"),
                Some(&json!({ "config": [{ "hostname": p.hostname, "cert_id": id, "enabled": true }] })),
            )?;
            write_secret(&dir.join("origin-pull-ca.pem"), &auth.ca_pem, 0o644)?;
            let mut state = read_state(dir);
            state["aop_cert_id"] = json!(id);
            write_state(dir, &state)?;
            Ok(format!("client certificate {id} uploaded, hostname AOP enabled"))
        }
        Step::EdgeConfig => {
            let path = dir.join("edge.toml");
            let mut config = if path.exists() {
                let text = std::fs::read_to_string(&path)?;
                std::fs::write(dir.join("edge.toml.bak"), &text)?;
                toml::from_str::<nie_edge::config::Config>(&text)?
            } else {
                nie_edge::config::Config::default()
            };
            config.listen.tls = Some(nie_edge::config::Tls {
                addr: format!("0.0.0.0:{}", p.port),
                cert: dir.join("origin.pem"),
                key: dir.join("origin.key"),
                client_ca: dir.join("origin-pull-ca.pem"),
                hosts: vec![p.hostname.clone()],
                h2: true,
            });
            config.validate()?;
            write_if_changed(&path, toml::to_string_pretty(&config)?.as_bytes(), 0o644)?;
            Ok(format!("{} lists the TLS listener on port {}", path.display(), p.port))
        }
        Step::Firewall => {
            let ranges = api.ranges()?;
            apply_firewall(p, &ranges)?;
            Ok(format!(
                "port {} open to {} IPv4 and {} IPv6 Cloudflare ranges",
                p.port,
                ranges.0.len(),
                ranges.1.len()
            ))
        }
        Step::SslMode => {
            let zone = api.zone_id(&p.zone_name)?;
            let current = api.need("GET", &format!("/zones/{zone}/settings/ssl"), None)?;
            if current["value"] == json!("strict") {
                return Ok("zone SSL mode already strict".into());
            }
            api.upsert_rule(
                &zone,
                "http_config_settings",
                &json!({
                    "ref": REF_SSL,
                    "description": "nie-edge: Full (strict) for the nie hostname",
                    "expression": host_expr(&p.hostname),
                    "action": "set_config",
                    "action_parameters": { "ssl": "strict" },
                }),
            )?;
            Ok(format!("configuration rule {REF_SSL} set (zone mode was {})", current["value"]))
        }
        Step::OriginRule => {
            let zone = api.zone_id(&p.zone_name)?;
            api.upsert_rule(
                &zone,
                "http_request_origin",
                &json!({
                    "ref": REF_PORT,
                    "description": "nie-edge: origin TLS port",
                    "expression": host_expr(&p.hostname),
                    "action": "route",
                    "action_parameters": { "origin": { "port": p.port } },
                }),
            )?;
            Ok(format!("origin rule {REF_PORT}: port {}", p.port))
        }
        Step::Dns => {
            let zone = api.zone_id(&p.zone_name)?;
            let path = format!("/zones/{zone}/dns_records");
            let existing = api.need("GET", &format!("{path}?name={}", p.hostname), None)?;
            let records = existing.as_array().cloned().unwrap_or_default();
            if records.iter().any(|r| {
                r["type"] == "A" && r["content"] == json!(p.origin_ip) && r["proxied"] == json!(true)
            }) {
                return Ok("proxied A record already points at the origin".into());
            }
            let mut state = read_state(dir);
            state["dns_before"] = json!(records);
            write_state(dir, &state)?;
            let record = json!({
                "type": "A", "name": p.hostname, "content": p.origin_ip, "proxied": true, "ttl": 1
            });
            match records.first().and_then(|r| r["id"].as_str()) {
                Some(id) => api.need("PUT", &format!("{path}/{id}"), Some(&record))?,
                None => api.need("POST", &path, Some(&record))?,
            };
            Ok(format!("{} now proxies to {}", p.hostname, p.origin_ip))
        }
    }
}

/// Undo a step that changed Cloudflare, the firewall or the edge configuration.
pub fn rollback(p: &Params, step: Step, token: Option<String>) -> Result<String> {
    let api = Api::new(&p.api_base, token)?;
    let state = read_state(&p.edge_dir);
    match step {
        Step::Dns => {
            let zone = api.zone_id(&p.zone_name)?;
            let path = format!("/zones/{zone}/dns_records");
            let before = state["dns_before"].as_array().cloned().unwrap_or_default();
            let current = api.need("GET", &format!("{path}?name={}", p.hostname), None)?;
            let current_id = current[0]["id"].as_str().map(str::to_owned);
            match (before.first(), current_id) {
                (Some(old), Some(id)) => {
                    api.need("PUT", &format!("{path}/{id}"), Some(&dns_body(old)))?;
                    Ok(format!("{} restored to {} {}", p.hostname, old["type"], old["content"]))
                }
                (Some(old), None) => {
                    api.need("POST", &path, Some(&dns_body(old)))?;
                    Ok("record recreated".into())
                }
                (None, Some(id)) => {
                    api.need("DELETE", &format!("{path}/{id}"), None)?;
                    Ok("record created by the switch removed".into())
                }
                (None, None) => bail!("no saved record and no current record"),
            }
        }
        Step::OriginRule => {
            let zone = api.zone_id(&p.zone_name)?;
            api.remove_rule(&zone, "http_request_origin", REF_PORT)?;
            Ok(format!("rule {REF_PORT} removed"))
        }
        Step::SslMode => {
            let zone = api.zone_id(&p.zone_name)?;
            api.remove_rule(&zone, "http_config_settings", REF_SSL)?;
            Ok(format!("rule {REF_SSL} removed"))
        }
        Step::ClientAuth => {
            let zone = api.zone_id(&p.zone_name)?;
            let id = state["aop_cert_id"].as_str().context("no recorded client certificate")?;
            api.need(
                "PUT",
                &format!("/zones/{zone}/origin_tls_client_auth/hostnames"),
                Some(&json!({ "config": [{ "hostname": p.hostname, "cert_id": id, "enabled": false }] })),
            )?;
            Ok("hostname AOP disabled".into())
        }
        Step::Firewall => {
            remove_firewall(p)?;
            Ok("firewall rules removed".into())
        }
        Step::EdgeConfig => {
            let bak = p.edge_dir.join("edge.toml.bak");
            std::fs::copy(&bak, p.edge_dir.join("edge.toml"))
                .with_context(|| format!("restore {}", bak.display()))?;
            Ok("edge.toml restored".into())
        }
        Step::OriginCert => Ok("nothing to undo".into()),
    }
}

/// Read-only view of every step: what Cloudflare and the host show now.
pub fn status(p: &Params, token: Option<String>) -> Result<Value> {
    let api = Api::new(&p.api_base, token)?;
    let dir = &p.edge_dir;
    let zone = api.zone_id(&p.zone_name)?;
    let has_rule = |phase: &str, reference: &str| -> Result<bool> {
        let path = format!("/zones/{zone}/rulesets/phases/{phase}/entrypoint");
        Ok(api.existing_rules(&path)?.iter().any(|r| r["ref"] == json!(reference)))
    };
    let dns = api.need("GET", &format!("/zones/{zone}/dns_records?name={}", p.hostname), None)?;
    let aop = api.call(
        "GET",
        &format!("/zones/{zone}/origin_tls_client_auth/hostnames/{}", p.hostname),
        None,
    )?;
    let ssl = api.need("GET", &format!("/zones/{zone}/settings/ssl"), None)?;
    let table = p.firewall_table.as_deref().unwrap_or("inet nie_edge");
    Ok(json!({
        "origin-cert": dir.join("origin.pem").exists() && dir.join("origin.key").exists(),
        "client-auth": { "ca_file": dir.join("origin-pull-ca.pem").exists(), "hostname_aop": aop },
        "edge-config": std::fs::read_to_string(dir.join("edge.toml")).is_ok_and(|t| t.contains("[listen.tls]")),
        "firewall": nftables::has_set(table, "nie_cf4"),
        "ssl-mode": { "zone": ssl["value"], "rule": has_rule("http_config_settings", REF_SSL)? },
        "origin-rule": has_rule("http_request_origin", REF_PORT)?,
        "dns": dns,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    type Calls = Arc<Mutex<Vec<(String, String, String)>>>;

    fn params(dir: &Path, base: &str) -> Params {
        Params {
            zone_name: "aphrody.app".into(),
            hostname: "nie.aphrody.app".into(),
            port: 2053,
            origin_ip: "203.0.113.7".into(),
            edge_dir: dir.into(),
            validity_days: 1095,
            api_base: base.into(),
            firewall_table: Some("inet fleet".into()),
        }
    }

    /// A fake API on loopback: answers by `(method, path)` from `routes` (null otherwise) and
    /// records `(method, path, body)` of every request.
    fn fake_api(routes: Vec<(&'static str, &'static str, Value)>) -> (String, Calls) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log: Calls = Arc::new(Mutex::new(Vec::new()));
        let seen = log.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut s = stream.unwrap();
                let mut buf = vec![0u8; 1 << 16];
                let mut n = 0;
                loop {
                    let k = s.read(&mut buf[n..]).unwrap();
                    n += k;
                    let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                    if let Some(split) = text.find("\r\n\r\n") {
                        let len = text[..split]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if n >= split + 4 + len {
                            break;
                        }
                    }
                    if k == 0 {
                        break;
                    }
                }
                let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
                let mut first = head.lines().next().unwrap().split(' ');
                let (method, path) = (first.next().unwrap().to_owned(), first.next().unwrap().to_owned());
                seen.lock().unwrap().push((method.clone(), path.clone(), body.to_owned()));
                let result = routes
                    .iter()
                    .find(|(m, p, _)| *m == method && *p == path)
                    .map_or(json!(null), |(_, _, v)| v.clone());
                let payload = json!({"success": true, "errors": [], "result": result}).to_string();
                write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                )
                .unwrap();
            }
        });
        (base, log)
    }

    #[test]
    fn zone_is_the_registrable_suffix() {
        assert_eq!(zone_of("nie.aphrody.app"), "aphrody.app");
        assert_eq!(zone_of("aphrody.app"), "aphrody.app");
    }

    #[test]
    fn plan_names_every_step_and_the_port() {
        let text = plan(&params(Path::new("/etc/nie/edge"), API_BASE));
        for step in Step::ALL {
            assert!(text.contains(step.name()), "{}", step.name());
        }
        assert!(text.contains("2053") && text.contains("nie_edge_origin_port") && text.contains("inet fleet"));
    }

    #[test]
    fn origin_csr_and_client_chain_are_pem() {
        if !graviola::util::is_cpu_supported() {
            let err = origin_csr("nie.aphrody.app").unwrap_err().to_string();
            assert!(err.contains("adx"), "{err}");
            eprintln!("skipped: {err} (the CPU of this host lacks one of them)");
            return;
        }
        let (key, csr) = origin_csr("nie.aphrody.app").unwrap();
        assert!(key.contains("PRIVATE KEY") && csr.contains("CERTIFICATE REQUEST"));
        let auth = client_auth("nie.aphrody.app").unwrap();
        assert_eq!(auth.ca_pem.matches("BEGIN CERTIFICATE").count(), 1);
        assert_eq!(auth.chain_pem.matches("BEGIN CERTIFICATE").count(), 2);
        assert!(auth.key_pem.contains("PRIVATE KEY"));
    }

    #[test]
    fn dates_are_calendar_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_453), (2025, 12, 31));
    }

    #[test]
    fn nft_scripts_name_the_cloudflare_ranges() {
        let v4 = vec!["173.245.48.0/20".to_owned()];
        let v6 = vec!["2400:cb00::/32".to_owned()];
        let own = nft_script(2053, &v4, &v6, None);
        assert!(own.contains("add table inet nie_edge") && own.contains("tcp dport 2053 drop"));
        let shared = nft_script(2053, &v4, &v6, Some("inet fleet"));
        assert!(shared.contains("add set inet fleet nie_cf4") && !shared.contains("add chain"));
        assert!(nft_insert_rules(2053, "inet fleet")[0].starts_with("insert rule inet fleet input"));
    }

    #[test]
    fn dns_switch_saves_the_tunnel_record_then_rollback_restores_it() {
        let dir = tempfile::tempdir().unwrap();
        let tunnel = json!({"id": "rec1", "type": "CNAME", "name": "nie.aphrody.app",
            "content": "x.cfargotunnel.com", "proxied": true, "ttl": 1});
        let (base, log) = fake_api(vec![
            ("GET", "/zones?name=aphrody.app", json!([{"id": "z1"}])),
            ("GET", "/zones/z1/dns_records?name=nie.aphrody.app", json!([tunnel])),
        ]);
        let p = params(dir.path(), &base);
        let report = apply(&p, Step::Dns, Some("t".into())).unwrap();
        assert!(report.contains("203.0.113.7"), "{report}");
        let calls = log.lock().unwrap().clone();
        let put = calls.iter().find(|c| c.0 == "PUT").expect("PUT record");
        assert_eq!(put.1, "/zones/z1/dns_records/rec1");
        assert!(put.2.contains("\"type\":\"A\"") && put.2.contains("\"proxied\":true"), "{}", put.2);
        assert_eq!(read_state(dir.path())["dns_before"][0]["type"], "CNAME");
        let report = rollback(&p, Step::Dns, Some("t".into())).unwrap();
        assert!(report.contains("cfargotunnel"), "{report}");
        let calls = log.lock().unwrap().clone();
        let last = calls.iter().rfind(|c| c.0 == "PUT").unwrap();
        assert!(last.2.contains("\"type\":\"CNAME\""), "{}", last.2);
    }

    #[test]
    fn origin_rule_keeps_other_rules_of_the_phase() {
        let dir = tempfile::tempdir().unwrap();
        let existing = json!({"rules": [{"id": "r9", "ref": "other", "version": "3", "last_updated": "x",
            "action": "route", "expression": "true", "action_parameters": {"origin": {"port": 8443}}}]});
        let (base, log) = fake_api(vec![
            ("GET", "/zones?name=aphrody.app", json!([{"id": "z1"}])),
            ("GET", "/zones/z1/rulesets/phases/http_request_origin/entrypoint", existing),
        ]);
        let p = params(dir.path(), &base);
        apply(&p, Step::OriginRule, Some("t".into())).unwrap();
        let calls = log.lock().unwrap().clone();
        let put = calls.iter().find(|c| c.0 == "PUT").unwrap();
        let body: Value = serde_json::from_str(&put.2).unwrap();
        let rules = body["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["ref"], "other");
        assert!(rules[0].get("version").is_none());
        assert_eq!(rules[1]["action_parameters"]["origin"]["port"], 2053);
        assert_eq!(rules[1]["expression"], "(http.host eq \"nie.aphrody.app\")");
    }

    #[test]
    fn edge_config_step_adds_the_tls_listener() {
        let dir = tempfile::tempdir().unwrap();
        let p = params(dir.path(), "http://127.0.0.1:1");
        let report = apply(&p, Step::EdgeConfig, None).unwrap();
        assert!(report.contains("2053"));
        let text = std::fs::read_to_string(dir.path().join("edge.toml")).unwrap();
        let config: nie_edge::config::Config = toml::from_str(&text).unwrap();
        let tls = config.listen.tls.unwrap();
        assert_eq!(tls.addr, "0.0.0.0:2053");
        assert_eq!(tls.hosts, ["nie.aphrody.app"]);
    }
}
