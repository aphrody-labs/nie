//! `rclone.conf` parsed in Rust: the remotes behind `rclone://`, `r2://`, `s3://` and `gdrive://`
//! resolve to an object store (`opendal`) or an SFTP host (`russh`); no `rclone` binary is run.
//!
//! Configuration comes from `RCLONE_CONFIG` (default `~/.config/rclone/rclone.conf`), and
//! `RCLONE_CONFIG_<REMOTE>_<KEY>` environment variables override or define remotes the way
//! rclone reads them.

use super::Result;
use super::ssh::SshSpec;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const ALIAS_DEPTH: usize = 8;

/// Object store a remote resolves to; `root` is the key prefix (or folder path) inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ObjectSpec {
    pub service: Service,
    pub root: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Service {
    S3 {
        bucket: String,
        endpoint: Option<String>,
        region: Option<String>,
        access_key_id: Option<String>,
        secret_access_key: Option<String>,
        session_token: Option<String>,
        /// Credentials and region come from the environment (`env_auth = true`).
        env_auth: bool,
    },
    Drive {
        client_id: Option<String>,
        client_secret: Option<String>,
        access_token: Option<String>,
        refresh_token: Option<String>,
    },
    Http {
        endpoint: String,
    },
}

pub(super) enum Target {
    Object(ObjectSpec),
    Sftp(SshSpec),
}

type Section = BTreeMap<String, String>;

#[derive(Default)]
pub(super) struct Conf {
    sections: BTreeMap<String, Section>,
}

pub(super) fn conf_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RCLONE_CONFIG").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let home_config =
        super::ssh::home_dir().map(|h| h.join(".config").join("rclone").join("rclone.conf"));
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| !p.is_empty())
        .map(|p| PathBuf::from(p).join("rclone").join("rclone.conf"));
    let appdata = std::env::var_os("APPDATA")
        .filter(|p| !p.is_empty())
        .map(|p| PathBuf::from(p).join("rclone").join("rclone.conf"));
    [xdg, home_config, appdata]
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
}

fn env_name(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

impl Conf {
    /// The configuration file (absent means empty) overlaid with `RCLONE_CONFIG_*` variables.
    pub fn load() -> Result<Self> {
        let Some(path) = conf_path() else {
            return Ok(Self::default());
        };
        Self::from_file(&path)
    }

    fn from_file(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        if text
            .trim_start()
            .starts_with("# Encrypted rclone configuration File")
        {
            return Err(
                "encrypted rclone configuration is unsupported: use a plain rclone.conf".to_owned(),
            );
        }
        let mut sections: BTreeMap<String, Section> = BTreeMap::new();
        let mut current: Option<String> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                let name = name.trim().to_owned();
                sections.entry(name.clone()).or_default();
                current = Some(name);
            } else if let Some(section) = &current
                && let Some((key, value)) = line.split_once('=')
                && let Some(entries) = sections.get_mut(section)
            {
                entries.insert(key.trim().to_owned(), value.trim().to_owned());
            }
        }
        Ok(Self { sections })
    }

    /// Value of `key` for `remote`: environment first, then the file.
    fn get(&self, remote: &str, key: &str) -> Option<String> {
        let var = format!("RCLONE_CONFIG_{}_{}", env_name(remote), env_name(key));
        std::env::var(var)
            .ok()
            .or_else(|| self.sections.get(remote)?.get(key).cloned())
            .filter(|v| !v.is_empty())
    }

    /// Remote names: the file's sections plus `RCLONE_CONFIG_<NAME>_TYPE` variables.
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.sections.keys().cloned().collect();
        for (key, _) in std::env::vars() {
            if let Some(name) = key
                .strip_prefix("RCLONE_CONFIG_")
                .and_then(|k| k.strip_suffix("_TYPE"))
                .filter(|n| !n.is_empty())
            {
                let name = name.to_ascii_lowercase();
                if !names.iter().any(|n| env_name(n) == env_name(&name)) {
                    names.push(name);
                }
            }
        }
        names.sort();
        names
    }

    /// One `{name, type, supported, reason?}` entry per remote, what `/v1/vfs/remotes` serves.
    pub fn remotes_json(&self) -> Value {
        Value::Array(
            self.names()
                .into_iter()
                .map(|name| {
                    let kind = self.get(&name, "type").unwrap_or_default();
                    match self.resolve(&name, "x") {
                        Ok(_) => json!({"name": name, "type": kind, "supported": true}),
                        Err(reason) => json!({
                            "name": name, "type": kind, "supported": false, "reason": reason
                        }),
                    }
                })
                .collect(),
        )
    }

    /// Resolves `remote` and a path (`bucket/key` for object stores) to its backend.
    pub fn resolve(&self, remote: &str, path: &str) -> Result<Target> {
        self.resolve_at(remote, path.trim_matches('/'), 0)
    }

    fn resolve_at(&self, remote: &str, path: &str, depth: usize) -> Result<Target> {
        let kind = self
            .get(remote, "type")
            .ok_or_else(|| format!("rclone remote `{remote}` is not defined in rclone.conf"))?;
        match kind.as_str() {
            "s3" => self.s3(remote, path).map(Target::Object),
            "drive" => self.drive(remote, path).map(Target::Object),
            "sftp" => self.sftp(remote, path).map(Target::Sftp),
            "http" => {
                let endpoint = self
                    .get(remote, "url")
                    .ok_or_else(|| format!("rclone remote `{remote}` has no url"))?;
                Ok(Target::Object(ObjectSpec {
                    service: Service::Http { endpoint },
                    root: format!("/{path}"),
                }))
            }
            "alias" => {
                if depth >= ALIAS_DEPTH {
                    return Err(format!("rclone alias `{remote}` loops"));
                }
                let target = self
                    .get(remote, "remote")
                    .ok_or_else(|| format!("rclone alias `{remote}` has no remote"))?;
                let (name, base) = target.split_once(':').ok_or_else(|| {
                    format!("rclone alias `{remote}` targets `{target}`, not a remote:path")
                })?;
                let joined = [base.trim_matches('/'), path]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("/");
                self.resolve_at(name, &joined, depth + 1)
            }
            "crypt" => Err(format!(
                "rclone crypt remote `{remote}` is unsupported: point at the underlying remote"
            )),
            other => Err(format!(
                "rclone remote `{remote}` has unsupported type `{other}` (supported: s3, drive, sftp, http, alias)"
            )),
        }
    }

    fn s3(&self, remote: &str, path: &str) -> Result<ObjectSpec> {
        let (bucket, key) = path.split_once('/').unwrap_or((path, ""));
        if bucket.is_empty() {
            return Err(format!(
                "rclone s3 remote `{remote}` needs a bucket: use scheme://{remote}/<bucket>/<path>"
            ));
        }
        let get = |key: &str| self.get(remote, key);
        let cloudflare = get("provider").is_some_and(|p| p.eq_ignore_ascii_case("cloudflare"));
        let region = get("region").or_else(|| cloudflare.then(|| "auto".to_owned()));
        Ok(ObjectSpec {
            service: Service::S3 {
                bucket: bucket.to_owned(),
                endpoint: get("endpoint"),
                region,
                access_key_id: get("access_key_id"),
                secret_access_key: get("secret_access_key"),
                session_token: get("session_token"),
                env_auth: get("env_auth").is_some_and(|v| v.eq_ignore_ascii_case("true")),
            },
            root: format!("/{key}"),
        })
    }

    fn drive(&self, remote: &str, path: &str) -> Result<ObjectSpec> {
        let get = |key: &str| self.get(remote, key);
        for unsupported in [
            "root_folder_id",
            "service_account_file",
            "service_account_credentials",
            "team_drive",
        ] {
            if get(unsupported).is_some() {
                return Err(format!(
                    "rclone drive remote `{remote}` sets `{unsupported}`, which is unsupported: use a path from My Drive"
                ));
            }
        }
        let token: Value = get("token")
            .map(|t| {
                serde_json::from_str(&t)
                    .map_err(|e| format!("rclone drive remote `{remote}`: bad token JSON: {e}"))
            })
            .transpose()?
            .unwrap_or(Value::Null);
        let field = |name: &str| {
            token
                .get(name)
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        };
        let (client_id, client_secret) = (get("client_id"), get("client_secret"));
        let refresh_token =
            field("refresh_token").filter(|_| client_id.is_some() && client_secret.is_some());
        let access_token = field("access_token");
        if access_token.is_none() && refresh_token.is_none() {
            return Err(format!(
                "rclone drive remote `{remote}` has no usable token (a refresh token needs client_id and client_secret)"
            ));
        }
        Ok(ObjectSpec {
            service: Service::Drive {
                client_id,
                client_secret,
                access_token,
                refresh_token,
            },
            root: format!("/{path}"),
        })
    }

    fn sftp(&self, remote: &str, path: &str) -> Result<SshSpec> {
        let get = |key: &str| self.get(remote, key);
        let host =
            get("host").ok_or_else(|| format!("rclone sftp remote `{remote}` has no host"))?;
        let port = get("port")
            .map(|p| {
                p.parse()
                    .map_err(|_| format!("rclone sftp remote `{remote}`: invalid port"))
            })
            .transpose()?;
        if get("pass").is_some()
            && get("key_file").is_none()
            && !get("use_agent").is_some_and(|v| v == "true")
        {
            return Err(format!(
                "rclone sftp remote `{remote}` uses a password, which is unsupported: use key_file or ssh-agent"
            ));
        }
        Ok(SshSpec {
            user: get("user"),
            host,
            port,
            root: path.to_owned(),
            use_config: false,
            identity_files: get("key_file").map(PathBuf::from).into_iter().collect(),
            known_hosts: get("known_hosts_file").map(PathBuf::from),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONF: &str = r#"
# a comment
[r2]
type = s3
provider = Cloudflare
access_key_id = AKIDEXAMPLE
secret_access_key = secretexample
endpoint = https://acct.r2.cloudflarestorage.com

[aws]
type = s3
provider = AWS
env_auth = true
region = eu-west-3

[gd]
type = drive
client_id = cid
client_secret = csecret
scope = drive.readonly
token = {"access_token":"at","token_type":"Bearer","refresh_token":"rt","expiry":"2026-11-19T12:00:00Z"}

[gdroot]
type = drive
root_folder_id = 1AbC

[box]
type = sftp
host = files.example.org
user = nie
port = 2222
key_file = /keys/nie

[boxpw]
type = sftp
host = files.example.org
pass = obscured

[games]
type = alias
remote = r2:ievr/games

[locked]
type = crypt
remote = r2:ievr

[mega]
type = mega
"#;

    fn conf() -> Conf {
        Conf::parse(CONF).unwrap()
    }

    fn object(target: Result<Target>) -> ObjectSpec {
        match target {
            Ok(Target::Object(spec)) => spec,
            _ => panic!("expected an object store"),
        }
    }

    #[test]
    fn r2_remote_resolves_to_a_cloudflare_s3_bucket() {
        let spec = object(conf().resolve("r2", "ievr/data/packs"));
        assert_eq!(spec.root, "/data/packs");
        assert_eq!(
            spec.service,
            Service::S3 {
                bucket: "ievr".into(),
                endpoint: Some("https://acct.r2.cloudflarestorage.com".into()),
                region: Some("auto".into()),
                access_key_id: Some("AKIDEXAMPLE".into()),
                secret_access_key: Some("secretexample".into()),
                session_token: None,
                env_auth: false,
            }
        );
        assert!(
            conf()
                .resolve("r2", "")
                .err()
                .unwrap()
                .contains("needs a bucket")
        );
    }

    #[test]
    fn aws_remote_uses_environment_credentials() {
        let Service::S3 {
            region,
            env_auth,
            bucket,
            ..
        } = object(conf().resolve("aws", "b/k")).service
        else {
            panic!("s3 expected")
        };
        assert_eq!(
            (region.as_deref(), env_auth, bucket.as_str()),
            (Some("eu-west-3"), true, "b")
        );
    }

    #[test]
    fn drive_remote_reads_the_token_json() {
        let spec = object(conf().resolve("gd", "Games/ievr"));
        assert_eq!(spec.root, "/Games/ievr");
        assert_eq!(
            spec.service,
            Service::Drive {
                client_id: Some("cid".into()),
                client_secret: Some("csecret".into()),
                access_token: Some("at".into()),
                refresh_token: Some("rt".into()),
            }
        );
        assert!(
            conf()
                .resolve("gdroot", "x")
                .err()
                .unwrap()
                .contains("root_folder_id")
        );
    }

    #[test]
    fn sftp_remote_resolves_to_an_ssh_spec_and_refuses_passwords() {
        let Ok(Target::Sftp(spec)) = conf().resolve("box", "srv/game") else {
            panic!("sftp expected")
        };
        assert_eq!(
            (
                spec.user.as_deref(),
                spec.host.as_str(),
                spec.port,
                spec.root.as_str(),
                spec.use_config
            ),
            (
                Some("nie"),
                "files.example.org",
                Some(2222),
                "srv/game",
                false
            )
        );
        assert_eq!(spec.identity_files, vec![PathBuf::from("/keys/nie")]);
        assert!(
            conf()
                .resolve("boxpw", "x")
                .err()
                .unwrap()
                .contains("password")
        );
    }

    #[test]
    fn alias_follows_its_target_and_crypt_is_unsupported() {
        let spec = object(conf().resolve("games", "x/y"));
        assert_eq!(spec.root, "/games/x/y");
        assert!(matches!(&spec.service, Service::S3 { bucket, .. } if bucket == "ievr"));
        let locked = conf().resolve("locked", "x").err().unwrap();
        assert!(locked.contains("crypt"), "{locked}");
        assert!(
            conf()
                .resolve("mega", "x")
                .err()
                .unwrap()
                .contains("unsupported type `mega`")
        );
        assert!(
            conf()
                .resolve("nope", "x")
                .err()
                .unwrap()
                .contains("not defined")
        );
    }

    #[test]
    fn alias_loops_are_cut() {
        let looped =
            Conf::parse("[a]\ntype = alias\nremote = b:x\n[b]\ntype = alias\nremote = a:y\n")
                .unwrap();
        assert!(looped.resolve("a", "").err().unwrap().contains("loops"));
    }

    #[test]
    fn remotes_json_lists_every_remote_with_support() {
        let remotes = conf().remotes_json();
        let by_name = |name: &str| {
            remotes
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["name"] == name)
                .cloned()
                .unwrap()
        };
        assert_eq!(by_name("r2")["supported"], true);
        assert_eq!(by_name("r2")["type"], "s3");
        assert_eq!(by_name("box")["supported"], true);
        assert_eq!(by_name("locked")["supported"], false);
        assert!(by_name("mega")["reason"].as_str().unwrap().contains("mega"));
    }

    #[test]
    fn encrypted_config_is_reported() {
        let err = Conf::parse("# Encrypted rclone configuration File\n\nRCLONE_ENCRYPT_V0:\nabc\n")
            .err()
            .unwrap();
        assert!(err.contains("encrypted"));
    }

    #[test]
    fn config_file_is_read_from_rclone_config_path_and_missing_means_empty() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("rclone.conf");
        std::fs::write(&file, CONF).unwrap();
        assert_eq!(Conf::from_file(&file).unwrap().names().len(), 9);
        assert!(
            Conf::from_file(&dir.path().join("absent.conf"))
                .unwrap()
                .names()
                .is_empty()
        );
    }
}
