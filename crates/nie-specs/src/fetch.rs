use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::Mode;

const MAX_BODY: u64 = 64 * 1024 * 1024;

pub struct Doc {
    pub content: String,
    pub sha256: String,
    pub bytes: usize,
}

pub struct Fetcher {
    agent: ureq::Agent,
    token: Option<String>,
    cache: HashMap<(String, Mode), std::rc::Rc<Doc>>,
    scripts: Regex,
    styles: Regex,
    tags: Regex,
    spaces: Regex,
}

impl Fetcher {
    pub fn new() -> Result<Self> {
        let agent = ureq::Agent::config_builder()
            .user_agent("nie-specs (https://github.com/aphrody-labs/nie)")
            .timeout_global(Some(Duration::from_secs(120)))
            .build()
            .new_agent();
        let token = ["GH_TOKEN", "GITHUB_TOKEN"]
            .iter()
            .find_map(|k| std::env::var(k).ok())
            .filter(|t| !t.is_empty());
        Ok(Self {
            agent,
            token,
            cache: HashMap::new(),
            scripts: Regex::new(r"(?s)<script\b.*?</script>")?,
            styles: Regex::new(r"(?s)<style\b.*?</style>")?,
            tags: Regex::new(r"(?s)<[^>]*>")?,
            spaces: Regex::new(r"\s+")?,
        })
    }

    fn get_bytes(&self, url: &str, github: bool) -> Result<Vec<u8>> {
        let mut req = self.agent.get(url);
        if github {
            req = req.header("Accept", "application/vnd.github+json");
            if let Some(t) = &self.token {
                req = req.header("Authorization", &format!("Bearer {t}"));
            }
        }
        let mut res = req.call().with_context(|| format!("GET {url}"))?;
        res.body_mut()
            .with_config()
            .limit(MAX_BODY)
            .read_to_vec()
            .with_context(|| format!("read {url}"))
    }

    /// Page content in the requested mode, fetched once per (url, mode).
    pub fn doc(&mut self, url: &str, mode: Mode) -> Result<std::rc::Rc<Doc>> {
        if let Some(d) = self.cache.get(&(url.to_owned(), mode)) {
            return Ok(d.clone());
        }
        let bytes = self.get_bytes(url, false)?;
        let raw = String::from_utf8_lossy(&bytes).into_owned();
        let content = match mode {
            Mode::Raw => raw,
            Mode::Text => self.strip(&raw),
        };
        let sha256 = hex(&Sha256::digest(content.as_bytes()));
        let doc = std::rc::Rc::new(Doc {
            sha256,
            bytes: bytes.len(),
            content,
        });
        self.cache.insert((url.to_owned(), mode), doc.clone());
        Ok(doc)
    }

    pub fn json(&self, url: &str) -> Result<Value> {
        let bytes = self.get_bytes(url, true)?;
        serde_json::from_slice(&bytes).with_context(|| format!("json {url}"))
    }

    fn strip(&self, html: &str) -> String {
        let s = self.scripts.replace_all(html, " ");
        let s = self.styles.replace_all(&s, " ");
        let s = self.tags.replace_all(&s, " ");
        let s = decode_entities(&s);
        self.spaces.replace_all(&s, " ").trim().to_owned()
    }

    /// `(sha, date)` of the latest commit, optionally restricted to a path on a branch.
    pub fn latest_commit(
        &self,
        repo: &str,
        path: Option<&str>,
        branch: Option<&str>,
    ) -> Result<(String, String)> {
        let mut url = format!("https://api.github.com/repos/{repo}/commits?per_page=1");
        if let Some(p) = path {
            url.push_str(&format!("&path={p}"));
        }
        if let Some(b) = branch {
            url.push_str(&format!("&sha={b}"));
        }
        let v = self.json(&url)?;
        let c = v.get(0).with_context(|| format!("no commit for {url}"))?;
        let sha = c["sha"].as_str().context("commit sha")?.to_owned();
        let date = c["commit"]["committer"]["date"]
            .as_str()
            .context("commit date")?;
        Ok((sha, date.to_owned()))
    }

    /// `(tag, date)` of the latest GitHub release.
    pub fn latest_release(&self, repo: &str) -> Result<(String, String)> {
        let v = self.json(&format!(
            "https://api.github.com/repos/{repo}/releases?per_page=1"
        ))?;
        let v = v.get(0).with_context(|| format!("no release in {repo}"))?;
        let tag = v["tag_name"].as_str().context("tag_name")?.to_owned();
        let date = v["published_at"].as_str().context("published_at")?;
        Ok((tag, date.get(..10).unwrap_or(date).to_owned()))
    }
}

pub fn extract(content: &str, pattern: &str, join: &str) -> Result<String> {
    let re = Regex::new(pattern).with_context(|| format!("pattern {pattern}"))?;
    let Some(c) = re.captures(content) else {
        bail!("pattern did not match: {pattern}");
    };
    let groups: Vec<&str> = c.iter().skip(1).flatten().map(|m| m.as_str()).collect();
    Ok(if groups.is_empty() {
        c[0].to_owned()
    } else {
        groups.join(join)
    })
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let decoded = rest.find(';').filter(|&j| j <= 10).and_then(|j| {
            let ent = &rest[1..j];
            let ch = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => ent
                    .strip_prefix('#')
                    .and_then(|n| match n.strip_prefix(['x', 'X']) {
                        Some(h) => u32::from_str_radix(h, 16).ok(),
                        None => n.parse().ok(),
                    })
                    .and_then(char::from_u32),
            }?;
            Some((ch, j + 1))
        });
        match decoded {
            Some((ch, n)) => {
                out.push(ch);
                rest = &rest[n..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_decode() {
        assert_eq!(
            decode_entities("a &lt;b&gt; &amp; &#65;&#x42; &bogus; &"),
            "a <b> & AB &bogus; &"
        );
    }

    #[test]
    fn extract_joins_groups() {
        assert_eq!(
            extract("'D', 'X', 'B', 'C'", r"'(.)', '(.)', '(.)', '(.)'", "").unwrap(),
            "DXBC"
        );
        assert_eq!(
            extract("v 1.17 - 6/20", r"v (\d+\.\d+)", "").unwrap(),
            "1.17"
        );
        assert!(extract("x", r"y(\d)", "").is_err());
    }
}
