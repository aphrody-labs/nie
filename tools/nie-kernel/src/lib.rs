//! Mainline kernel tag watcher: parsing and the decision, with no network access (the binary feeds it the live bytes).
//!
//! Sources: `releases.json` of kernel.org (the announced mainline version) and the git smart-HTTP ref advertisement of
//! torvalds/linux.git (the tags that exist). A build starts only when both name the same newest mainline tag and it
//! is not the one already recorded. Signature checks happen where the source is fetched (`deploy/kernel/build.sh source`,
//! `git verify-tag` against the fingerprints pinned in `deploy/kernel/keys.toml`).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// A mainline tag: `v7.4-rc3` or `v7.4` (patch releases such as `v7.3.5` belong to stable, not here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag {
    pub major: u32,
    pub minor: u32,
    /// `Some(n)` for `-rcN`, `None` for the final release.
    pub rc: Option<u32>,
}

impl Tag {
    /// Parse `v7.4-rc3`, `7.4-rc3`, `v7.4`. Returns `None` for anything else (stable `7.3.5`, `v2.6.39.4`, `-next`).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let (base, rc) = match text.split_once("-rc") {
            Some((base, rc)) => (base, Some(rc.parse().ok()?)),
            None => (text, None),
        };
        let (major, minor) = base.split_once('.')?;
        if minor.contains('.') {
            return None;
        }
        Some(Self { major: major.parse().ok()?, minor: minor.parse().ok()?, rc })
    }

    fn key(&self) -> (u32, u32, u32) {
        // A final release sorts after every release candidate of the same version.
        (self.major, self.minor, self.rc.unwrap_or(u32::MAX))
    }

    /// The git tag name, `v7.4-rc3`.
    pub fn name(&self) -> String {
        format!("v{self}")
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.rc {
            Some(rc) => write!(f, "{}.{}-rc{rc}", self.major, self.minor),
            None => write!(f, "{}.{}", self.major, self.minor),
        }
    }
}

impl Ord for Tag {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl PartialOrd for Tag {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Deserialize)]
struct Releases {
    releases: Vec<Release>,
}

#[derive(Deserialize)]
struct Release {
    moniker: String,
    version: String,
    source: Option<String>,
}

/// The mainline entry of `releases.json`: its tag and the snapshot tarball URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Announced {
    pub tag: Tag,
    pub source: Option<String>,
}

/// Read the mainline release out of `https://www.kernel.org/releases.json`.
pub fn announced_mainline(json: &[u8]) -> Result<Announced> {
    let releases: Releases = serde_json::from_slice(json).context("releases.json")?;
    let mainline = releases
        .releases
        .into_iter()
        .find(|r| r.moniker == "mainline")
        .context("releases.json has no mainline entry")?;
    let tag = Tag::parse(&mainline.version)
        .with_context(|| format!("mainline version {:?} is not a mainline tag", mainline.version))?;
    Ok(Announced { tag, source: mainline.source })
}

/// A tag of the git ref advertisement: the annotated tag object and the commit it points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagRef {
    pub tag: Tag,
    pub tag_oid: String,
    pub commit_oid: Option<String>,
}

fn pkt_lines(mut data: &[u8]) -> Result<Vec<Option<&[u8]>>> {
    let mut lines = Vec::new();
    while !data.is_empty() {
        let len = data.get(..4).context("truncated pkt-line length")?;
        let len = usize::from_str_radix(std::str::from_utf8(len)?, 16).context("pkt-line length")?;
        if len == 0 {
            lines.push(None);
            data = &data[4..];
            continue;
        }
        if len < 4 || len > data.len() {
            bail!("pkt-line length {len} out of range ({} bytes left)", data.len());
        }
        lines.push(Some(&data[4..len]));
        data = &data[len..];
    }
    Ok(lines)
}

/// Tags of a protocol v0 `info/refs?service=git-upload-pack` response, mainline tags only.
pub fn mainline_tags(advertisement: &[u8]) -> Result<Vec<TagRef>> {
    let mut found: BTreeMap<Tag, TagRef> = BTreeMap::new();
    for line in pkt_lines(advertisement)?.into_iter().flatten() {
        let line = String::from_utf8_lossy(line);
        let line = line.split('\0').next().unwrap_or("").trim_end();
        let Some((oid, name)) = line.split_once(' ') else { continue };
        let Some(name) = name.strip_prefix("refs/tags/") else { continue };
        let (name, peeled) = match name.strip_suffix("^{}") {
            Some(name) => (name, true),
            None => (name, false),
        };
        if !name.starts_with('v') {
            continue;
        }
        let Some(tag) = Tag::parse(name) else { continue };
        let entry = found.entry(tag).or_insert_with(|| TagRef { tag, tag_oid: String::new(), commit_oid: None });
        if peeled {
            entry.commit_oid = Some(oid.to_string());
        } else {
            entry.tag_oid = oid.to_string();
        }
    }
    Ok(found.into_values().filter(|t| !t.tag_oid.is_empty()).collect())
}

/// What the watcher concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Decision {
    /// Both sources name `tag` and it was not built yet.
    Build { tag: String, tag_oid: String, commit_oid: Option<String>, source: Option<String> },
    /// Both sources name `tag` and it is the recorded one.
    UpToDate { tag: String },
    /// The sources disagree, or one is empty: nothing starts, the reason says why.
    Wait { reason: String },
}

/// Decide from the two sources and the last recorded tag.
pub fn decide(announced: &Announced, tags: &[TagRef], recorded: Option<Tag>) -> Decision {
    let Some(newest) = tags.iter().max_by_key(|t| t.tag) else {
        return Decision::Wait { reason: "the git advertisement has no mainline tag".into() };
    };
    if newest.tag != announced.tag {
        return Decision::Wait {
            reason: format!("releases.json announces {} but the newest tag is {}", announced.tag, newest.tag),
        };
    }
    if recorded.is_some_and(|r| r >= newest.tag) {
        return Decision::UpToDate { tag: newest.tag.name() };
    }
    Decision::Build {
        tag: newest.tag.name(),
        tag_oid: newest.tag_oid.clone(),
        commit_oid: newest.commit_oid.clone(),
        source: announced.source.clone(),
    }
}

/// One recorded outcome of a pipeline run, kept as a JSON line in `<state>/history.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub tag: String,
    pub status: String,
    pub at: String,
    #[serde(default)]
    pub bzimage_sha256: Option<String>,
}

/// The newest tag among records whose status is `ok`.
pub fn last_ok(history: &str) -> Option<Tag> {
    history
        .lines()
        .filter_map(|l| serde_json::from_str::<Record>(l).ok())
        .filter(|r| r.status == "ok")
        .filter_map(|r| Tag::parse(&r.tag))
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkt(s: &str) -> Vec<u8> {
        format!("{:04x}{s}", s.len() + 4).into_bytes()
    }

    fn advertisement() -> Vec<u8> {
        let mut out = pkt("# service=git-upload-pack\n");
        out.extend_from_slice(b"0000");
        out.extend(pkt("1111111111111111111111111111111111111111 HEAD\0multi_ack thin-pack\n"));
        for (oid, name) in [
            ("2222222222222222222222222222222222222222", "refs/tags/v7.3"),
            ("3333333333333333333333333333333333333333", "refs/tags/v7.3^{}"),
            ("4444444444444444444444444444444444444444", "refs/tags/v7.4-rc2"),
            ("5555555555555555555555555555555555555555", "refs/tags/v7.4-rc3"),
            ("6666666666666666666666666666666666666666", "refs/tags/v7.4-rc3^{}"),
            ("7777777777777777777777777777777777777777", "refs/tags/v7.3.5"),
            ("8888888888888888888888888888888888888888", "refs/tags/v2.6.39-rc1"),
            ("9999999999999999999999999999999999999999", "refs/tags/v7.4-rc10"),
            ("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "refs/tags/next-20261118"),
        ] {
            out.extend(pkt(&format!("{oid} {name}\n")));
        }
        out.extend_from_slice(b"0000");
        out
    }

    #[test]
    fn release_candidates_sort_below_the_final_and_numerically() {
        let t = |s| Tag::parse(s).unwrap();
        assert!(t("v7.4-rc3") < t("v7.4-rc10"));
        assert!(t("v7.4-rc10") < t("v7.4"));
        assert!(t("v7.4") < t("v7.5-rc1"));
        assert!(t("v7.9") < t("v7.10-rc1"));
        assert_eq!(t("7.4-rc3").name(), "v7.4-rc3");
        assert_eq!(t("v7.4").to_string(), "7.4");
    }

    #[test]
    fn stable_and_legacy_names_are_not_mainline_tags() {
        for s in ["7.3.5", "v7.3.5", "next-20261118", "v7.4-rc", "v7", "v2.6.39.4", "v7.4-rcx"] {
            assert_eq!(Tag::parse(s), None, "{s}");
        }
    }

    #[test]
    fn ref_advertisement_gives_tags_with_their_commit() {
        let tags = mainline_tags(&advertisement()).unwrap();
        let rc3 = tags.iter().find(|t| t.tag == Tag::parse("v7.4-rc3").unwrap()).unwrap();
        assert_eq!(rc3.tag_oid, "5555555555555555555555555555555555555555");
        assert_eq!(rc3.commit_oid.as_deref(), Some("6666666666666666666666666666666666666666"));
        assert_eq!(tags.iter().map(|t| t.tag).max().unwrap(), Tag::parse("v7.4-rc10").unwrap());
        assert!(tags.iter().all(|t| t.tag.major >= 2));
    }

    #[test]
    fn truncated_advertisement_is_an_error() {
        assert!(mainline_tags(b"00ff# service").is_err());
        assert!(mainline_tags(b"zz").is_err());
    }

    #[test]
    fn mainline_entry_of_releases_json() {
        let json = br#"{"releases":[{"moniker":"mainline","version":"7.4-rc3","source":"https://git.kernel.org/torvalds/t/linux-7.4-rc3.tar.gz"},
            {"moniker":"stable","version":"7.3.5","source":"x"}]}"#;
        let a = announced_mainline(json).unwrap();
        assert_eq!(a.tag, Tag::parse("v7.4-rc3").unwrap());
        assert!(a.source.unwrap().ends_with("linux-7.4-rc3.tar.gz"));
        assert!(announced_mainline(br#"{"releases":[{"moniker":"stable","version":"7.3.5"}]}"#).is_err());
    }

    fn refs(names: &[&str]) -> Vec<TagRef> {
        names
            .iter()
            .map(|n| TagRef { tag: Tag::parse(n).unwrap(), tag_oid: "ab".repeat(20), commit_oid: None })
            .collect()
    }

    fn announced(v: &str) -> Announced {
        Announced { tag: Tag::parse(v).unwrap(), source: None }
    }

    #[test]
    fn decision_builds_only_when_both_sources_agree_and_the_tag_is_new() {
        let tags = refs(&["v7.4-rc2", "v7.4-rc3"]);
        assert!(matches!(decide(&announced("7.4-rc3"), &tags, None), Decision::Build { tag, .. } if tag == "v7.4-rc3"));
        assert!(matches!(
            decide(&announced("7.4-rc3"), &tags, Tag::parse("v7.4-rc2")),
            Decision::Build { .. }
        ));
        assert_eq!(
            decide(&announced("7.4-rc3"), &tags, Tag::parse("v7.4-rc3")),
            Decision::UpToDate { tag: "v7.4-rc3".into() }
        );
        assert!(matches!(decide(&announced("7.4-rc2"), &tags, None), Decision::Wait { .. }));
        assert!(matches!(decide(&announced("7.4-rc3"), &[], None), Decision::Wait { .. }));
    }

    #[test]
    fn history_keeps_the_newest_successful_tag() {
        let history = concat!(
            "{\"tag\":\"v7.4-rc2\",\"status\":\"ok\",\"at\":\"2026-11-12T00:00:00Z\"}\n",
            "{\"tag\":\"v7.4-rc3\",\"status\":\"failed\",\"at\":\"2026-11-19T00:00:00Z\"}\n",
            "not json\n"
        );
        assert_eq!(last_ok(history), Tag::parse("v7.4-rc2"));
        assert_eq!(last_ok(""), None);
    }
}
