use std::path::Path;

use serde::Serialize;

use crate::{CraftError, Frontend, Manifest};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// A binary that runs on this machine.
    Native,
    /// A WebAssembly bundle served to a browser.
    Web,
}

/// The cargo package a frontend resolves to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    pub package: &'static str,
    pub features: &'static [&'static str],
    pub cargo_target: Option<&'static str>,
    pub kind: TargetKind,
}

impl Frontend {
    pub fn target(self) -> Target {
        match self {
            Frontend::Bevy3d => Target {
                package: "nie-football",
                features: &[],
                cargo_target: None,
                kind: TargetKind::Native,
            },
            Frontend::Studio => Target {
                package: "nie-studio",
                features: &[],
                cargo_target: None,
                kind: TargetKind::Native,
            },
            Frontend::BevyWeb => Target {
                package: "nie-football",
                features: &["webgpu"],
                cargo_target: Some("wasm32-unknown-unknown"),
                kind: TargetKind::Web,
            },
        }
    }
}

/// A resolved manifest: what to build and how to start it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub game: String,
    pub fingerprint: String,
    pub target: Target,
    /// Argument vector (cargo first) that builds the target.
    pub build: Vec<String>,
    /// Argument vector that builds and starts the game on the given manifest.
    pub launch: Vec<String>,
}

impl Manifest {
    pub fn plan(&self, manifest_path: &Path, release: bool) -> Result<Plan, CraftError> {
        let target = self.game.frontend.target();
        let mut build = vec![
            "cargo".to_owned(),
            "build".to_owned(),
            "-p".to_owned(),
            target.package.to_owned(),
        ];
        if release {
            build.push("--release".to_owned());
        }
        if !target.features.is_empty() {
            build.push("--no-default-features".to_owned());
            build.push("--features".to_owned());
            build.push(target.features.join(","));
        }
        if let Some(triple) = target.cargo_target {
            build.push("--target".to_owned());
            build.push(triple.to_owned());
        }

        let launch = match target.kind {
            TargetKind::Native => {
                let mut cmd = build.clone();
                cmd[1] = "run".to_owned();
                cmd.extend([
                    "--".to_owned(),
                    "--manifest".to_owned(),
                    manifest_path.display().to_string(),
                ]);
                cmd
            }
            // trunk builds the page's `<link data-trunk rel="rust">` target and serves the bundle.
            TargetKind::Web => vec![
                "trunk".to_owned(),
                "serve".to_owned(),
                format!("crates/{}/index.html", target.package),
            ],
        };
        Ok(Plan {
            game: self.game.id.clone(),
            fingerprint: format!("{:016x}", self.fingerprint()?),
            target,
            build,
            launch,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEB: &str = r#"
schema = 1
[game]
id = "web-cup"
name = "Web Cup"
version = "0.1.0"
mode = "exhibition"
frontend = "bevy_web"
[teams.home]
[teams.away]
"#;

    #[test]
    fn web_frontend_targets_wasm() {
        let m = Manifest::from_toml(WEB).unwrap();
        let plan = m.plan(Path::new("games/web-cup.toml"), true).unwrap();
        assert_eq!(plan.target.kind, TargetKind::Web);
        assert_eq!(plan.target.package, "nie-football");
        assert!(
            plan.build
                .join(" ")
                .contains("--features webgpu --target wasm32-unknown-unknown")
        );
        assert_eq!(plan.launch[0], "trunk");
    }

    #[test]
    fn native_launch_passes_the_manifest() {
        let m = Manifest::from_toml(&WEB.replace("bevy_web", "bevy3d")).unwrap();
        let plan = m.plan(Path::new("games/x.toml"), false).unwrap();
        assert_eq!(&plan.launch[1..4], ["run", "-p", "nie-football"]);
        assert!(
            plan.launch
                .ends_with(&["--manifest".to_owned(), "games/x.toml".to_owned()])
        );
    }
}
