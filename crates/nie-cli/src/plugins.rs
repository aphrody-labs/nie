//! The plugins of this build and the `nie plugins` command.

use nie_plugin::Registry;

/// Registers every plugin compiled into this build.
///
/// # Panics
/// Never in a shipped build: the unit tests prove that no id or command is claimed twice.
#[must_use]
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    #[cfg(feature = "cloud")]
    {
        registry
            .register(Box::new(nie_save::SavePlugin))
            .expect("save plugin");
        registry
            .register(Box::new(nie_steam::SteamPlugin))
            .expect("steam plugin");
    }
    #[cfg(feature = "host")]
    {
        registry
            .register(Box::new(nie_re_core::ExePlugin))
            .expect("exe plugin");
        registry
            .register(Box::new(nie_launcher::LauncherPlugin))
            .expect("launcher plugin");
        registry
            .register(Box::new(nie_shell_desktop::ShellDesktopPlugin))
            .expect("shell-desktop plugin");
        registry
            .register(Box::new(nie_tui::TuiPlugin))
            .expect("tui plugin");
    }
    registry
}

/// Prints the registered plugins, as text or JSON.
pub fn run(json: bool) -> anyhow::Result<()> {
    let infos = registry().infos();
    if json {
        let value: Vec<_> = infos
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "summary": p.summary,
                    "package": p.package,
                    "commands": p.commands,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        for p in infos {
            println!("{:<10} {}  [{}]", p.id, p.summary, p.commands.join(" "));
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "host"))]
mod tests {
    use super::*;
    use clap::CommandFactory as _;

    #[test]
    fn every_plugin_command_is_a_cli_command() {
        let commands: Vec<String> = crate::Cli::command()
            .get_subcommands()
            .map(|c| c.get_name().to_owned())
            .collect();
        for info in registry().infos() {
            for command in info.commands {
                assert!(
                    commands.iter().any(|c| c == command),
                    "plugin `{}` declares `{command}`, which `nie --help` does not list",
                    info.id
                );
            }
        }
    }

    #[test]
    fn the_plugins_are_registered() {
        let ids: Vec<_> = registry().infos().iter().map(|p| p.id).collect();
        assert_eq!(ids, ["save", "steam", "exe", "launcher", "shell-desktop", "tui"]);
    }
}
