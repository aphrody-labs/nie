use std::path::PathBuf;

use anyhow::{Result, bail};
use nie_specs::{hosts, publish, sync};

const USAGE: &str = "usage:
  nie-specs sync  [--root DIR] [--check] [SPEC_ID...]
  nie-specs hosts [--root DIR] [--local-as NAME] [--publish OWNER/REPO[@BRANCH] [--prefix PATH]] [NAME...]
default root: docs/specs";

fn main() {
    nie_tls::install();
    if let Err(e) = run() {
        eprintln!("nie-specs: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        bail!("{USAGE}")
    };
    if matches!(cmd.as_str(), "-h" | "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    let mut root = PathBuf::from("docs/specs");
    let mut check = false;
    let mut local_as = None;
    let mut publish_to = None;
    let mut prefix = String::from("docs/specs");
    let mut rest = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                root = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or_else(|| anyhow::anyhow!("--root needs a value"))?
            }
            "--check" => check = true,
            "--local-as" => local_as = args.next(),
            "--publish" => publish_to = args.next(),
            "--prefix" => {
                prefix = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--prefix needs a value"))?
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => rest.push(a),
        }
    }
    match cmd.as_str() {
        "sync" => sync::run(&root, &rest, check),
        "hosts" => {
            let target = publish_to
                .as_deref()
                .map(|t| t.split_once('@').unwrap_or((t, "main")));
            if let Some((repo, branch)) = target {
                std::fs::create_dir_all(&root)?;
                publish::pull(repo, branch, &prefix, &root, &["sources.toml".to_owned()])?;
                let names: Vec<String> =
                    nie_specs::config::Sources::load(&root.join("sources.toml"))?
                        .host
                        .into_iter()
                        .map(|h| format!("hosts/{}.json", h.name))
                        .collect();
                publish::pull(repo, branch, &prefix, &root, &names)?;
            }
            let files = hosts::run(&root, &rest, local_as.as_deref())?;
            if let Some((repo, branch)) = target {
                let written =
                    publish::publish(repo, branch, "Host specs refresh", &prefix, &files)?;
                println!("published {} file(s)", written.len());
            }
            Ok(())
        }
        _ => bail!("{USAGE}"),
    }
}
