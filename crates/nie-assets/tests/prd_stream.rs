//! Opt-in: streams the real installation from the `prd` host over SSH and lists a CPK through
//! the VFS, reading only ranges. Skipped (loudly) when `NIE_ASSETS_PRD_SOURCE` is unset or the
//! host is unreachable. Example:
//!
//! ```text
//! NIE_ASSETS_PRD_SOURCE="ssh://<game-host>/<game_dir>" cargo test -p nie-assets --test prd_stream -- --nocapture
//! ```
//!
//! Nothing is downloaded beyond the CPK tables and one small file; no game byte is stored.

use nie_assets::AssetsConfig;

#[test]
fn lists_and_reads_a_cpk_over_the_prd_path() {
    let Some(source) = std::env::var("NIE_ASSETS_PRD_SOURCE")
        .ok()
        .filter(|s| !s.trim().is_empty())
    else {
        eprintln!("SKIP: NIE_ASSETS_PRD_SOURCE is not set");
        return;
    };
    let vfs = match AssetsConfig::for_source(source.clone()).open_vfs() {
        Ok(vfs) => vfs,
        Err(error) => {
            eprintln!("SKIP: cannot reach {source}: {error}");
            return;
        }
    };
    assert!(vfs.cpk_count() > 0, "the cpk_list names at least one pack");
    assert!(vfs.asset_count() > 1000, "the index lists the game files");
    let (path, _) = vfs
        .iter()
        .find(|(path, entry)| path.ends_with(".cfg.bin") && entry.file_size > 0)
        .expect("an indexed .cfg.bin");
    let bytes = vfs.read(path).expect("range-read one file from its CPK");
    assert!(!bytes.is_empty());
    let (calls, transferred) = vfs.remote_stats().expect("remote mount");
    eprintln!(
        "{} CPK, {} files; read {path} ({} bytes) in {calls} calls, {transferred} bytes transferred",
        vfs.cpk_count(),
        vfs.asset_count(),
        bytes.len()
    );
}
