//! Read the game binary in place: a local path or `ssh://host/absolute/path`.
//! Nothing is written to disk; the bytes live in memory for the run.

use anyhow::{Context, Result, bail};

/// Quote for a POSIX shell: single quotes, embedded quotes closed and reopened.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Split `ssh://host/path` into host and absolute path (spaces stay raw).
pub fn parse_ssh(spec: &str) -> Option<(&str, String)> {
    let rest = spec.strip_prefix("ssh://")?;
    let (host, path) = rest.split_once('/')?;
    Some((host, format!("/{path}")))
}

/// Load the binary named by `spec`.
pub fn load(spec: &str) -> Result<Vec<u8>> {
    if let Some((host, path)) = parse_ssh(spec) {
        let out = nie_ssh::exec_blocking(host, &format!("cat -- {}", sh_quote(&path)), Vec::new())
            .with_context(|| format!("ssh {host}"))?;
        if out.status != Some(0) {
            bail!("ssh {host} cat {path}: {}", out.stderr_text().trim());
        }
        return Ok(out.stdout);
    }
    std::fs::read(spec).with_context(|| format!("read {spec}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_spec_keeps_spaces() {
        let (h, p) = parse_ssh("ssh://prd/srv/nie/steam/INAZUMA ELEVEN Victory Road/nie.exe").unwrap();
        assert_eq!(h, "prd");
        assert_eq!(p, "/srv/nie/steam/INAZUMA ELEVEN Victory Road/nie.exe");
    }

    #[test]
    fn quote_escapes_single_quotes() {
        assert_eq!(sh_quote("a'b c"), r"'a'\''b c'");
    }

    #[test]
    fn local_path_is_not_ssh() {
        assert!(parse_ssh("/tmp/x.exe").is_none());
    }
}
