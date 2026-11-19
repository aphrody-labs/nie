//! Steam data of the installation: the app manifest (build id, install dir) and Steam's own
//! depot manifest, which lists size and SHA-1 of every file.
use anyhow::{Context, Result, bail};
use nie_steam::install::AppManifest;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Install {
    pub app_id: u32,
    pub build_id: String,
    pub install_dir: String,
    /// Directory holding the installation files.
    pub root: PathBuf,
    pub acf: PathBuf,
    /// `<depot>_<manifest id>` of the installed depot.
    pub depot_manifest: Option<String>,
    /// Steam's depot manifest file when it exists on this host.
    pub depot_manifest_file: Option<PathBuf>,
}

/// Read `appmanifest_*.acf` from a `steamapps` directory. `app_id` picks one when several games live there.
pub fn read_install(steamapps: &Path, app_id: Option<u32>) -> Result<Install> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(steamapps).with_context(|| steamapps.display().to_string())? {
        let path = entry?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if let Some(id) = name
            .strip_prefix("appmanifest_")
            .and_then(|n| n.strip_suffix(".acf"))
            .and_then(|n| n.parse::<u32>().ok())
            && app_id.is_none_or(|want| want == id)
        {
            found.push(path);
        }
    }
    found.sort();
    let acf = match found.as_slice() {
        [one] => one.clone(),
        [] => bail!("no appmanifest_*.acf in {}", steamapps.display()),
        _ => bail!("several apps in {}, pass --app-id", steamapps.display()),
    };
    let text = std::fs::read_to_string(&acf)?;
    let app = AppManifest::parse(&text).map_err(|e| anyhow::anyhow!("{}: {e}", acf.display()))?;
    let root = steamapps.join("common").join(&app.install_dir);
    let vdf = nie_steam::parse_vdf(&text).map_err(|e| anyhow::anyhow!("{e}"))?;
    let depot_manifest = app.installed_depots.first().and_then(|depot| {
        let id = vdf
            .get_path(&["AppState", "InstalledDepots", &depot.to_string(), "manifest"])
            .and_then(|v| match v {
                nie_steam::VdfValue::Str(s) => Some(s.as_str()),
                _ => None,
            })?;
        Some(format!("{depot}_{id}"))
    });
    let steam_root = steamapps.parent().map(Path::to_path_buf).unwrap_or_default();
    let depot_manifest_file = depot_manifest.as_ref().and_then(|name| {
        [
            steam_root.join("depotcache"),
            steam_root.join(".depotdownloader").join("manifests"),
            nie_steam::state_dir().join(app.app_id.to_string()).join("manifests"),
            root.join(".nie-steam-cache").join("manifests"),
        ]
        .into_iter()
        .map(|dir| dir.join(format!("{name}.manifest")))
        .find(|p| p.is_file())
    });
    Ok(Install {
        app_id: app.app_id,
        build_id: app.build_id.to_string(),
        install_dir: app.install_dir,
        root,
        acf,
        depot_manifest,
        depot_manifest_file,
    })
}

/// Size and SHA-1 of every file Steam lists (directories excluded). Steam encrypts file names with a
/// depot key that is only available through a Steam session, so files are compared by content.
pub struct SteamFiles {
    pub files: Vec<(u64, [u8; 20])>,
    pub manifest_id: Option<u64>,
}

pub fn parse_depot_manifest(bytes: &[u8]) -> Result<SteamFiles> {
    let m = steamroom::depot::manifest::DepotManifest::parse(bytes)
        .map_err(|e| anyhow::anyhow!("steam depot manifest: {e}"))?;
    let files = m
        .files
        .iter()
        .filter_map(|f| f.sha_content.map(|sha| (f.size, sha)))
        .collect();
    Ok(SteamFiles {
        files,
        manifest_id: m.manifest_id.map(|id| id.0),
    })
}

/// Result of matching local `(size, sha1)` pairs against Steam's list as multisets.
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamMatch {
    pub steam_files: usize,
    pub matched: usize,
    /// Listed by Steam, absent from the link.
    pub missing: Vec<String>,
    /// Present locally with a content hash Steam does not list.
    pub unlisted: usize,
}

#[must_use]
pub fn match_steam(steam: &SteamFiles, local: &[(u64, [u8; 20])]) -> SteamMatch {
    let mut left: HashMap<(u64, [u8; 20]), usize> = HashMap::new();
    for f in &steam.files {
        *left.entry(*f).or_default() += 1;
    }
    let mut matched = 0;
    let mut unlisted = 0;
    for f in local {
        match left.get_mut(f) {
            Some(n) if *n > 0 => {
                *n -= 1;
                matched += 1;
            }
            _ => unlisted += 1,
        }
    }
    let mut missing: Vec<String> = left
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|((size, sha), n)| format!("{size} bytes sha1 {} x{n}", crate::hex(&sha)))
        .collect();
    missing.sort();
    SteamMatch {
        steam_files: steam.files.len(),
        matched,
        missing,
        unlisted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiset_match_counts_duplicates() {
        let a = (10, [1; 20]);
        let b = (20, [2; 20]);
        let steam = SteamFiles { files: vec![a, a, b], manifest_id: None };
        let m = match_steam(&steam, &[a, b, (30, [3; 20])]);
        assert_eq!((m.steam_files, m.matched, m.unlisted), (3, 2, 1));
        assert_eq!(m.missing.len(), 1);
    }

    #[test]
    fn reads_app_manifest_and_depot_file() {
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("steamapps");
        std::fs::create_dir_all(apps.join("common/Game")).unwrap();
        std::fs::create_dir_all(dir.path().join("depotcache")).unwrap();
        std::fs::write(dir.path().join("depotcache/5_99.manifest"), b"x").unwrap();
        std::fs::write(
            apps.join("appmanifest_4.acf"),
            "\"AppState\"\n{\n\"appid\" \"4\"\n\"installdir\" \"Game\"\n\"buildid\" \"123\"\n\"InstalledDepots\"\n{\n\"5\"\n{\n\"manifest\" \"99\"\n}\n}\n}\n",
        )
        .unwrap();
        let i = read_install(&apps, None).unwrap();
        assert_eq!((i.app_id, i.build_id.as_str(), i.install_dir.as_str()), (4, "123", "Game"));
        assert_eq!(i.depot_manifest.as_deref(), Some("5_99"));
        assert!(i.depot_manifest_file.is_some());
    }
}
