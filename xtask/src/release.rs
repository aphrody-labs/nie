//! `cargo xtask release archive`: packs the release binaries with the licences and the README. The version
//! check, the builds and the publication are `just ci-release-*` recipes.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::util::{repo_root, run as sh};

/// Arguments of `cargo xtask release`.
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// Pack binaries plus the licences and README into `<out>/<name>.tar.gz` (or `.zip` with `--zip`).
    Archive {
        /// Archive stem and top-level folder, for example `nie-v2.0.0-x86_64-unknown-linux-musl`.
        #[arg(long)]
        name: String,
        /// Folder that holds the binaries.
        #[arg(long)]
        bin_dir: PathBuf,
        /// Binary file names, comma separated.
        #[arg(long, value_delimiter = ',', required = true)]
        bins: Vec<String>,
        /// Output folder.
        #[arg(long, default_value = "dist")]
        out: PathBuf,
        /// Make a zip instead of a tar.gz.
        #[arg(long)]
        zip: bool,
    },
}

fn archive(
    root: &Path,
    name: &str,
    bin_dir: &Path,
    bins: &[String],
    out: &Path,
    zip: bool,
) -> Result<()> {
    let stage = out.join(name);
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)?;
    for bin in bins {
        std::fs::copy(bin_dir.join(bin), stage.join(bin))
            .with_context(|| format!("copy {bin} from {}", bin_dir.display()))?;
    }
    for extra in ["LICENSE", "LICENSE-APACHE", "README.md"] {
        std::fs::copy(root.join(extra), stage.join(extra)).with_context(|| extra.to_owned())?;
    }
    if zip {
        let file = out.join(format!("{name}.zip"));
        sh(Command::new("powershell")
            .args(["-NoProfile", "-Command", "Compress-Archive", "-Path"])
            .arg(&stage)
            .arg("-DestinationPath")
            .arg(&file))?;
    } else {
        let file = out.join(format!("{name}.tar.gz"));
        sh(Command::new("tar")
            .arg("-C")
            .arg(out)
            .arg("-czf")
            .arg(&file)
            .arg(name))?;
    }
    std::fs::remove_dir_all(&stage)?;
    Ok(())
}

/// Runs a release helper.
pub fn run(args: Args) -> Result<()> {
    let root = repo_root();
    match args.cmd {
        Cmd::Archive {
            name,
            bin_dir,
            bins,
            out,
            zip,
        } => archive(&root, &name, &bin_dir, &bins, &out, zip),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archives_hold_the_binaries_and_the_licences() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        let bins = dir.path().join("bin");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&bins).unwrap();
        for f in ["LICENSE", "LICENSE-APACHE", "README.md"] {
            std::fs::write(root.join(f), f).unwrap();
        }
        std::fs::write(bins.join("nie"), b"x").unwrap();
        let out = dir.path().join("dist");
        std::fs::create_dir_all(&out).unwrap();
        if crate::util::have("tar") {
            archive(&root, "nie-test", &bins, &["nie".to_owned()], &out, false).unwrap();
            assert!(out.join("nie-test.tar.gz").is_file());
            assert!(!out.join("nie-test").exists());
        }
        assert!(archive(&root, "bad", &bins, &["missing".to_owned()], &out, false).is_err());
    }
}
