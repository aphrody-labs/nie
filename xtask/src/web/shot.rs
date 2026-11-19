//! `web shot`: headless Chrome proof of a page: `navigator.gpu`, console errors and a screenshot.
//!
//! Prints one JSON object `{ gpu, adapter, title, canvases, errors, warnings, logs, extra, screenshot }` and
//! exits 1 when `navigator.gpu` is missing or a console error or exception was raised. Chrome comes from
//! `NIE_CHROME`, else `google-chrome` / `chromium` on `PATH`, else the usual install paths; WebGPU runs on
//! SwiftShader (software Vulkan), so a host without a GPU works.
//!
//! Environment: `NIE_HEADED` (show the window), `NIE_CHROME_FLAGS` (extra flags, space separated), `NIE_KEYS`
//! (comma separated key names pressed after the page settled: `Enter,Down,Up,Left,Right` or a character),
//! `NIE_KEY_WAIT` (ms after each key, 700), `NIE_PROBE_JS` (expression evaluated after the keys, reported as
//! `extra`), `NIE_SHOT_PARAMS` (JSON merged into the `Page.captureScreenshot` parameters).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

use super::cdp::{Cdp, http_get};

/// Arguments of `web shot`.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// Page to load.
    pub url: String,
    /// PNG file to write.
    pub out: PathBuf,
    /// Milliseconds to wait after navigation before probing.
    #[arg(long, default_value_t = 8000)]
    pub wait_ms: u64,
    #[arg(long, default_value_t = 1280)]
    pub width: u32,
    #[arg(long, default_value_t = 760)]
    pub height: u32,
    /// Command that starts the server under test (split on whitespace, no shell); killed at the end.
    #[arg(long)]
    pub spawn: Option<String>,
    /// URL polled until it answers 200 (60 s) before the page is loaded; used with `--spawn`.
    #[arg(long)]
    pub ready: Option<String>,
}

/// Kills and reaps a child process when dropped.
struct Reaper(Child);

impl Drop for Reaper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Removes a directory tree when dropped.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Console and log findings, gathered from DevTools events exactly as the page reported them.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Collector {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub logs: Vec<String>,
}

/// `value ?? description ?? ""` of a console argument, as JavaScript would print it.
fn arg_text(arg: &Value) -> String {
    match arg.get("value") {
        None | Some(Value::Null) => arg["description"].as_str().unwrap_or("").to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
    }
}

impl Collector {
    pub fn handle(&mut self, event: &Value) {
        let params = &event["params"];
        match event["method"].as_str().unwrap_or("") {
            "Runtime.consoleAPICalled" => {
                let text = params["args"]
                    .as_array()
                    .map(|a| a.iter().map(arg_text).collect::<Vec<_>>().join(" "))
                    .unwrap_or_default();
                match params["type"].as_str().unwrap_or("") {
                    "error" => self.errors.push(text),
                    "warning" => self.warnings.push(text),
                    kind if self.logs.len() < 40 => {
                        self.logs
                            .push(format!("{kind}: {text}").chars().take(300).collect());
                    }
                    _ => {}
                }
            }
            "Runtime.exceptionThrown" => {
                let details = &params["exceptionDetails"];
                let text = details["exception"]["description"]
                    .as_str()
                    .or_else(|| details["text"].as_str())
                    .unwrap_or("");
                self.errors.push(text.to_owned());
            }
            "Log.entryAdded" if params["entry"]["level"] == "error" => {
                let entry = &params["entry"];
                self.errors.push(format!(
                    "{}: {} {}",
                    entry["source"].as_str().unwrap_or(""),
                    entry["text"].as_str().unwrap_or(""),
                    entry["url"].as_str().unwrap_or("")
                ));
            }
            _ => {}
        }
    }
}

/// A key press: DevTools `Input.dispatchKeyEvent` fields.
#[derive(Debug, PartialEq, Eq)]
pub struct Key {
    pub key: String,
    pub code: String,
    pub vk: u32,
    pub text: Option<String>,
}

pub fn key_for(name: &str) -> Key {
    let named = |key: &str, vk: u32, text: Option<&str>| Key {
        key: key.to_owned(),
        code: key.to_owned(),
        vk,
        text: text.map(str::to_owned),
    };
    match name {
        "Enter" => named("Enter", 13, Some("\r")),
        "Down" => named("ArrowDown", 40, None),
        "Up" => named("ArrowUp", 38, None),
        "Right" => named("ArrowRight", 39, None),
        "Left" => named("ArrowLeft", 37, None),
        other => Key {
            key: other.to_owned(),
            code: format!("Key{}", other.to_uppercase()),
            vk: other.to_uppercase().chars().next().map_or(0, u32::from),
            text: Some(other.to_owned()),
        },
    }
}

/// Standard-alphabet base64 (padding optional, whitespace ignored).
pub fn base64_decode(text: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' | b' ' => continue,
            _ => bail!("invalid base64 byte {c:#x}"),
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

fn which(name: &str) -> Option<PathBuf> {
    let suffixes: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd"]
    } else {
        &[""]
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|dir| suffixes.iter().map(move |s| dir.join(format!("{name}{s}"))))
        .find(|p| p.is_file())
}

/// `NIE_CHROME`, else the first Chrome or Chromium found on `PATH` or at a standard install path.
pub fn find_chrome() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("NIE_CHROME") {
        return Ok(PathBuf::from(path));
    }
    for name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ] {
        if let Some(path) = which(name) {
            return Ok(path);
        }
    }
    let known = [
        "C:/Program Files/Google/Chrome/Application/chrome.exe",
        "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
    ];
    known
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .context("no Chrome found: set NIE_CHROME or put google-chrome on PATH")
}

/// Chrome command-line flags for a headless WebGPU run on SwiftShader.
pub fn chrome_flags(
    profile: &Path,
    port: u16,
    width: u32,
    height: u32,
    headed: bool,
    extra: &str,
) -> Vec<String> {
    let mut flags = Vec::new();
    if !headed {
        flags.push("--headless=new".to_owned());
    }
    flags.extend([
        "--no-sandbox".to_owned(),
        format!("--user-data-dir={}", profile.display()),
        format!("--remote-debugging-port={port}"),
        format!("--window-size={width},{height}"),
        "--enable-unsafe-webgpu".to_owned(),
        "--enable-features=Vulkan,WebGPU".to_owned(),
        "--use-angle=swiftshader".to_owned(),
        "--enable-unsafe-swiftshader".to_owned(),
        "--ignore-gpu-blocklist".to_owned(),
    ]);
    flags.extend(
        extra
            .split(' ')
            .filter(|f| !f.is_empty())
            .map(str::to_owned),
    );
    flags.push("about:blank".to_owned());
    flags
}

fn free_port() -> Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port())
}

/// Polls `url` until it answers 200.
fn wait_ready(url: &str, server: &mut Reaper) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    for _ in 0..60 {
        if let Some(status) = server.0.try_wait()? {
            bail!("the server exited early ({status})");
        }
        if client
            .get(url)
            .send()
            .is_ok_and(|r| r.status().as_u16() == 200)
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    bail!("{url} did not answer 200 within 60 s")
}

fn debugger_url(port: u16) -> Result<String> {
    for _ in 0..100 {
        if let Ok(body) = http_get(&format!("127.0.0.1:{port}"), "/json")
            && let Ok(Value::Array(targets)) = serde_json::from_str::<Value>(&body)
            && let Some(url) = targets
                .iter()
                .find(|t| t["type"] == "page")
                .and_then(|t| t["webSocketDebuggerUrl"].as_str())
        {
            return Ok(url.to_owned());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("Chrome did not start")
}

const PROBE: &str = r#"(async () => {
    const gpu = typeof navigator.gpu !== "undefined";
    let adapter = null;
    if (gpu) { const a = await navigator.gpu.requestAdapter(); adapter = a ? (a.info?.description || a.info?.vendor || "adapter") : null; }
    return { gpu, adapter, title: document.title, canvases: document.querySelectorAll("canvas").length };
  })()"#;

/// `Runtime.evaluate` of an expression by value; the evaluated value, or an error for an exception.
fn evaluate(cdp: &mut Cdp, expression: &str) -> Result<Value> {
    let reply = cdp.call(
        "Runtime.evaluate",
        &json!({ "awaitPromise": true, "returnByValue": true, "expression": expression }),
    )?;
    if let Some(details) = reply.get("exceptionDetails") {
        bail!("evaluation failed: {details}");
    }
    Ok(reply["result"]["value"].clone())
}

fn shoot(args: &Args) -> Result<bool> {
    let mut server = match &args.spawn {
        Some(command) => {
            let mut words = command.split_whitespace();
            let program = words.next().context("--spawn is empty")?;
            Some(Reaper(
                Command::new(program)
                    .args(words)
                    .stdin(Stdio::null())
                    .spawn()
                    .with_context(|| format!("spawn {command}"))?,
            ))
        }
        None => None,
    };
    if let (Some(server), Some(ready)) = (server.as_mut(), &args.ready) {
        wait_ready(ready, server)?;
    }

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let profile =
        TempDir(std::env::temp_dir().join(format!("nie-chrome-{}-{nanos}", std::process::id())));
    let port = free_port()?;
    let flags = chrome_flags(
        &profile.0,
        port,
        args.width,
        args.height,
        std::env::var_os("NIE_HEADED").is_some(),
        &std::env::var("NIE_CHROME_FLAGS").unwrap_or_default(),
    );
    let chrome = find_chrome()?;
    let _chrome = Reaper(
        Command::new(&chrome)
            .args(&flags)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("spawn {}", chrome.display()))?,
    );

    let mut cdp = Cdp::connect(&debugger_url(port)?)?;
    for domain in ["Runtime.enable", "Log.enable", "Page.enable"] {
        cdp.call(domain, &json!({}))?;
    }
    cdp.call(
        "Emulation.setDeviceMetricsOverride",
        &json!({ "width": args.width, "height": args.height, "deviceScaleFactor": 1, "mobile": false }),
    )?;
    cdp.call("Page.navigate", &json!({ "url": args.url }))?;
    cdp.pump(Duration::from_millis(args.wait_ms))?;
    let probe = evaluate(&mut cdp, PROBE)?;

    let key_wait = std::env::var("NIE_KEY_WAIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(700);
    for name in std::env::var("NIE_KEYS")
        .unwrap_or_default()
        .split(',')
        .filter(|n| !n.is_empty())
    {
        let key = key_for(name);
        let mut base = json!({ "key": key.key, "code": key.code, "windowsVirtualKeyCode": key.vk, "nativeVirtualKeyCode": key.vk });
        let mut down = base.clone();
        down["type"] = json!(if key.text.is_some() {
            "keyDown"
        } else {
            "rawKeyDown"
        });
        if let Some(text) = &key.text {
            down["text"] = json!(text);
        }
        base["type"] = json!("keyUp");
        cdp.call("Input.dispatchKeyEvent", &down)?;
        cdp.call("Input.dispatchKeyEvent", &base)?;
        cdp.pump(Duration::from_millis(key_wait))?;
    }
    let extra = match std::env::var("NIE_PROBE_JS") {
        Ok(js) if !js.is_empty() => Some(evaluate(&mut cdp, &js)?),
        _ => None,
    };
    let mut shot_params = json!({ "format": "png" });
    if let Ok(Value::Object(more)) = std::env::var("NIE_SHOT_PARAMS")
        .map(|v| serde_json::from_str::<Value>(&v).unwrap_or(Value::Null))
    {
        for (k, v) in more {
            shot_params[k] = v;
        }
    }
    let shot = cdp.call("Page.captureScreenshot", &shot_params)?;
    std::fs::write(
        &args.out,
        base64_decode(shot["data"].as_str().context("no screenshot data")?)?,
    )
    .with_context(|| args.out.display().to_string())?;

    cdp.pump(Duration::from_millis(50))?;
    let mut found = Collector::default();
    for event in cdp.take_events() {
        found.handle(&event);
    }
    cdp.close();

    let mut result: Map<String, Value> = probe.as_object().cloned().unwrap_or_default();
    result.insert("errors".into(), json!(found.errors));
    result.insert("warnings".into(), json!(found.warnings));
    result.insert("logs".into(), json!(found.logs));
    if let Some(extra) = extra {
        result.insert("extra".into(), extra);
    }
    result.insert("screenshot".into(), json!(args.out.display().to_string()));
    println!("{}", Value::Object(result));
    Ok(probe["gpu"] == true && found.errors.is_empty())
}

pub fn run(args: &Args) -> Result<()> {
    if !shoot(args)? {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn console(kind: &str, args: Value) -> Value {
        json!({ "method": "Runtime.consoleAPICalled", "params": { "type": kind, "args": args } })
    }

    #[test]
    fn console_events_split_into_errors_warnings_and_logs() {
        let mut c = Collector::default();
        c.handle(&console(
            "error",
            json!([{ "value": "boom" }, { "value": 5 }, { "description": "Object" }]),
        ));
        c.handle(&console("warning", json!([{ "value": "careful" }])));
        c.handle(&console(
            "log",
            json!([{ "value": true }, { "value": null, "description": "d" }]),
        ));
        assert_eq!(c.errors, ["boom 5 Object"]);
        assert_eq!(c.warnings, ["careful"]);
        assert_eq!(c.logs, ["log: true d"]);
    }

    #[test]
    fn logs_are_capped_at_forty_entries_of_three_hundred_characters() {
        let mut c = Collector::default();
        for _ in 0..50 {
            c.handle(&console("info", json!([{ "value": "x".repeat(500) }])));
        }
        assert_eq!(c.logs.len(), 40);
        assert_eq!(c.logs[0].chars().count(), 300);
    }

    #[test]
    fn exceptions_and_error_log_entries_are_errors() {
        let mut c = Collector::default();
        c.handle(&json!({ "method": "Runtime.exceptionThrown", "params": { "exceptionDetails": { "text": "Uncaught", "exception": { "description": "TypeError: x" } } } }));
        c.handle(&json!({ "method": "Runtime.exceptionThrown", "params": { "exceptionDetails": { "text": "Uncaught" } } }));
        c.handle(&json!({ "method": "Log.entryAdded", "params": { "entry": { "level": "error", "source": "network", "text": "404", "url": "http://x/a" } } }));
        c.handle(&json!({ "method": "Log.entryAdded", "params": { "entry": { "level": "info", "source": "other", "text": "fine" } } }));
        c.handle(&json!({ "method": "Page.loadEventFired", "params": {} }));
        assert_eq!(
            c.errors,
            ["TypeError: x", "Uncaught", "network: 404 http://x/a"]
        );
        assert!(c.warnings.is_empty() && c.logs.is_empty());
    }

    #[test]
    fn keys_map_named_and_plain() {
        assert_eq!(
            key_for("Enter"),
            Key {
                key: "Enter".into(),
                code: "Enter".into(),
                vk: 13,
                text: Some("\r".into())
            }
        );
        assert_eq!(key_for("Down").key, "ArrowDown");
        assert_eq!(key_for("Left").vk, 37);
        assert!(key_for("Up").text.is_none());
        assert_eq!(
            key_for("a"),
            Key {
                key: "a".into(),
                code: "KeyA".into(),
                vk: 65,
                text: Some("a".into())
            }
        );
    }

    #[test]
    fn base64_decodes_with_and_without_padding() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGVsbG8").unwrap(), b"hello");
        assert_eq!(base64_decode("aGk=\n").unwrap(), b"hi");
        assert_eq!(base64_decode("").unwrap(), b"");
        assert_eq!(base64_decode("+/+/").unwrap(), [0xfb, 0xff, 0xbf]);
        assert!(base64_decode("a*b").is_err());
    }

    #[test]
    fn chrome_flags_are_headless_swiftshader_webgpu() {
        let flags = chrome_flags(Path::new("/tmp/p"), 9333, 1280, 760, false, "--a  --b");
        assert_eq!(flags[0], "--headless=new");
        assert!(flags.contains(&"--user-data-dir=/tmp/p".to_owned()));
        assert!(flags.contains(&"--remote-debugging-port=9333".to_owned()));
        assert!(flags.contains(&"--window-size=1280,760".to_owned()));
        assert!(flags.contains(&"--use-angle=swiftshader".to_owned()));
        let tail: Vec<_> = flags.iter().rev().take(3).collect();
        assert_eq!(tail, ["about:blank", "--b", "--a"]);
        assert!(
            !chrome_flags(Path::new("/tmp/p"), 1, 1, 1, true, "")
                .contains(&"--headless=new".to_owned())
        );
    }
}
