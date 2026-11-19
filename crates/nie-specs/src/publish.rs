use std::{path::Path, time::Duration};

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

/// Commit changed files to `branch` through the GitHub contents API (no checkout needed), used by
/// the fr1 timer. Returns the paths actually written. Token: `GH_TOKEN` or `GITHUB_TOKEN`.
pub fn publish(
    repo: &str,
    branch: &str,
    message: &str,
    prefix: &str,
    files: &[(String, String)],
) -> Result<Vec<String>> {
    let token = ["GH_TOKEN", "GITHUB_TOKEN"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .filter(|t| !t.is_empty())
        .context("GH_TOKEN or GITHUB_TOKEN is required to publish")?;
    let agent = ureq::Agent::config_builder()
        .user_agent("nie-specs")
        .timeout_global(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .build()
        .new_agent();
    let auth = format!("Bearer {token}");
    let mut written = Vec::new();
    for (rel, content) in files {
        let path = format!("{prefix}/{rel}");
        let url = format!("https://api.github.com/repos/{repo}/contents/{path}");
        let mut res = agent
            .get(&format!("{url}?ref={branch}"))
            .header("Authorization", &auth)
            .header("Accept", "application/vnd.github+json")
            .call()?;
        let (sha, same) = match res.status().as_u16() {
            200 => {
                let v: Value = serde_json::from_slice(&res.body_mut().read_to_vec()?)?;
                let old = v["content"].as_str().unwrap_or("").replace('\n', "");
                let old = STANDARD.decode(old).unwrap_or_default();
                (
                    v["sha"].as_str().map(str::to_owned),
                    old == content.as_bytes(),
                )
            }
            404 => (None, false),
            s => bail!("GET {path}: HTTP {s}"),
        };
        if same {
            continue;
        }
        let mut body = json!({
            "message": message,
            "branch": branch,
            "content": STANDARD.encode(content.as_bytes()),
        });
        if let Some(sha) = sha {
            body["sha"] = Value::String(sha);
        }
        let put = agent
            .put(&url)
            .header("Authorization", &auth)
            .header("Accept", "application/vnd.github+json")
            .header("Content-Type", "application/json")
            .send(serde_json::to_vec(&body)?)?;
        if !put.status().is_success() {
            bail!("PUT {path}: HTTP {}", put.status());
        }
        written.push(path);
    }
    Ok(written)
}

/// Download files of the repository into `root` (public raw URLs): the timer on fr1 has no
/// checkout, so `sources.toml` and the host JSON of the machines it cannot reach come from here.
/// A missing file (404) is skipped.
pub fn pull(repo: &str, branch: &str, prefix: &str, root: &Path, rels: &[String]) -> Result<()> {
    let agent = ureq::Agent::config_builder()
        .user_agent("nie-specs")
        .timeout_global(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .build()
        .new_agent();
    for rel in rels {
        let url = format!("https://raw.githubusercontent.com/{repo}/{branch}/{prefix}/{rel}");
        let mut res = agent.get(&url).call()?;
        match res.status().as_u16() {
            200 => {
                let path = root.join(rel);
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&path, res.body_mut().read_to_vec()?)?;
            }
            404 => {}
            s => bail!("GET {url}: HTTP {s}"),
        }
    }
    Ok(())
}
