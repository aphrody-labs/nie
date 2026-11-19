//! The plugin contract of nie.
//!
//! The core of nie reads two artifacts of the user's installation: the game binary `nie.exe` and
//! its data VFS. Everything else (the nie.exe reverse-engineering stack, saves, the Ultimate Team
//! launcher and its mods, Steam and Proton) is a plugin on top of that core. A plugin is a crate
//! that implements [`Plugin`] and declares which top-level CLI commands (each of them is also an
//! MCP tool, `cli_<command>`) it owns. The [`Registry`] refuses two plugins with the same id and
//! two plugins that claim the same command, so ownership stays unambiguous.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

/// What a plugin declares about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginInfo {
    /// Stable identifier, lowercase ASCII (`exe`, `save`, `launcher`, `steam`).
    pub id: &'static str,
    /// One line saying what the plugin does.
    pub summary: &'static str,
    /// Cargo package that implements the plugin.
    pub package: &'static str,
    /// Top-level CLI commands the plugin owns, as `nie --help` lists them.
    pub commands: &'static [&'static str],
}

/// A feature crate that plugs into the nie core.
pub trait Plugin: Send + Sync {
    /// The declaration of this plugin.
    fn info(&self) -> PluginInfo;
}

/// Why a plugin could not be registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// A plugin with this id is already registered.
    DuplicateId(&'static str),
    /// Another plugin already owns this command.
    DuplicateCommand {
        /// The contested command.
        command: &'static str,
        /// The plugin that owns it.
        owner: &'static str,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "plugin `{id}` is registered twice"),
            Self::DuplicateCommand { command, owner } => {
                write!(
                    f,
                    "command `{command}` is already owned by plugin `{owner}`"
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// The plugins of one nie build, in registration order.
#[derive(Default)]
pub struct Registry {
    plugins: Vec<Box<dyn Plugin>>,
    owners: BTreeMap<&'static str, &'static str>,
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a plugin.
    ///
    /// # Errors
    /// Returns [`RegistryError`] when the id or one of the commands is already taken; the
    /// registry is left unchanged.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<(), RegistryError> {
        let info = plugin.info();
        if self.plugins.iter().any(|p| p.info().id == info.id) {
            return Err(RegistryError::DuplicateId(info.id));
        }
        if let Some(command) = info.commands.iter().find(|c| self.owners.contains_key(*c)) {
            return Err(RegistryError::DuplicateCommand {
                command,
                owner: self.owners[command],
            });
        }
        for command in info.commands {
            self.owners.insert(command, info.id);
        }
        self.plugins.push(plugin);
        Ok(())
    }

    /// The declarations of every registered plugin.
    #[must_use]
    pub fn infos(&self) -> Vec<PluginInfo> {
        self.plugins.iter().map(|p| p.info()).collect()
    }

    /// The id of the plugin that owns a CLI command, if any.
    #[must_use]
    pub fn owner_of(&self, command: &str) -> Option<&'static str> {
        self.owners.get(command).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(PluginInfo);

    impl Plugin for Fake {
        fn info(&self) -> PluginInfo {
            self.0
        }
    }

    const fn info(id: &'static str, commands: &'static [&'static str]) -> PluginInfo {
        PluginInfo {
            id,
            summary: "test",
            package: "nie-test",
            commands,
        }
    }

    #[test]
    fn registers_and_resolves_owners() {
        let mut registry = Registry::new();
        registry
            .register(Box::new(Fake(info("a", &["x", "y"]))))
            .unwrap();
        registry
            .register(Box::new(Fake(info("b", &["z"]))))
            .unwrap();
        assert_eq!(registry.owner_of("y"), Some("a"));
        assert_eq!(registry.owner_of("z"), Some("b"));
        assert_eq!(registry.owner_of("w"), None);
        assert_eq!(registry.infos().len(), 2);
    }

    #[test]
    fn refuses_duplicate_ids_and_commands_without_changing_the_registry() {
        let mut registry = Registry::new();
        registry
            .register(Box::new(Fake(info("a", &["x"]))))
            .unwrap();
        assert_eq!(
            registry.register(Box::new(Fake(info("a", &["q"])))),
            Err(RegistryError::DuplicateId("a"))
        );
        assert_eq!(
            registry.register(Box::new(Fake(info("b", &["q", "x"])))),
            Err(RegistryError::DuplicateCommand {
                command: "x",
                owner: "a"
            })
        );
        assert_eq!(registry.owner_of("q"), None);
        assert_eq!(registry.infos().len(), 1);
    }
}
