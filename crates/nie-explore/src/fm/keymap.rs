//! The keymap: key sequences to [`Action`]s, in yazi's `keymap.toml` layout.

use std::collections::HashMap;

use serde::Deserialize;

use super::keys::Key;

/// The embedded default keymap.
pub const DEFAULT_KEYMAP: &str = include_str!("keymap.toml");

/// How far a cursor moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A number of rows.
    Lines(i32),
    /// A percentage of the visible height (`50%` is half a page).
    Percent(i32),
    /// The first row.
    Top,
    /// The last row.
    Bottom,
}

/// How the current directory is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    /// By name.
    #[default]
    Name,
    /// By size, largest first.
    Size,
    /// By extension, then name.
    Ext,
}

/// What a key does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Move the cursor of the current column.
    Arrow(Step),
    /// Go to the parent directory.
    Leave,
    /// Enter the directory under the cursor.
    Enter,
    /// Go to the root.
    Cd,
    /// Quit.
    Quit,
    /// Clear the search or the filter, or close the input line.
    Escape,
    /// Mark or unmark the entry.
    Toggle,
    /// Mark or unmark every entry of the directory.
    ToggleAll,
    /// Open the fuzzy search line.
    Search,
    /// Open the glob filter line.
    Filter,
    /// Jump from a search result to its directory.
    Reveal,
    /// Switch between path and CPK grouping.
    Mode,
    /// Change the sort.
    Sort(SortOrder),
    /// Scroll the preview by rows.
    Preview(i32),
    /// Step the frame of an animated or gallery preview (sprite, film frame, turntable angle).
    Frame(i32),
    /// Step the item of a multi-part file: the texture of a G4TX, the record of a cfg.bin, the
    /// language of a text table.
    Item(i32),
    /// Play or pause the animation of the preview.
    Play,
    /// Toggle the hex view of the file.
    Hex,
    /// Show or hide the key help.
    Help,
    /// Input line: a typed character.
    Input(char),
    /// Input line: delete the previous character.
    Backspace,
    /// Input line: clear.
    Clear,
    /// Input line: accept.
    Commit,
    /// Input line: cancel.
    Cancel,
}

/// A sort change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Sort by this key.
    By(SortKey),
    /// Reverse the current order.
    Reverse,
}

impl Action {
    /// Parses a `run` string (`arrow 1`, `sort size`, `quit`).
    ///
    /// # Errors
    /// Fails on an unknown command or a bad argument.
    pub fn parse(run: &str) -> Result<Self, String> {
        let mut words = run.split_whitespace();
        let command = words.next().ok_or("empty command")?;
        let arg = words.next();
        let number = |what: &str| -> Result<i32, String> {
            arg.ok_or_else(|| format!("`{command}` needs {what}"))?
                .parse()
                .map_err(|_| format!("`{command}`: `{}` is not a number", arg.unwrap_or_default()))
        };
        Ok(match command {
            "arrow" => {
                let arg = arg.ok_or("`arrow` needs a step")?;
                Self::Arrow(match arg {
                    "top" => Step::Top,
                    "bot" | "bottom" => Step::Bottom,
                    _ => match arg.strip_suffix('%') {
                        Some(percent) => Step::Percent(
                            percent
                                .parse()
                                .map_err(|_| format!("`arrow`: bad percentage `{arg}`"))?,
                        ),
                        None => Step::Lines(
                            arg.parse()
                                .map_err(|_| format!("`arrow`: bad step `{arg}`"))?,
                        ),
                    },
                })
            }
            "leave" => Self::Leave,
            "enter" => Self::Enter,
            "cd" => Self::Cd,
            "quit" => Self::Quit,
            "escape" => Self::Escape,
            "toggle" => Self::Toggle,
            "toggle_all" => Self::ToggleAll,
            "search" => Self::Search,
            "filter" => Self::Filter,
            "reveal" => Self::Reveal,
            "mode" => Self::Mode,
            "sort" => Self::Sort(match arg {
                Some("name") => SortOrder::By(SortKey::Name),
                Some("size") => SortOrder::By(SortKey::Size),
                Some("ext") => SortOrder::By(SortKey::Ext),
                Some("reverse") => SortOrder::Reverse,
                other => return Err(format!("`sort`: unknown key `{}`", other.unwrap_or(""))),
            }),
            "preview" => Self::Preview(number("a row count")?),
            "frame" => Self::Frame(number("a frame step")?),
            "item" => Self::Item(number("an item step")?),
            "play" => Self::Play,
            "hex" => Self::Hex,
            "help" => Self::Help,
            "backspace" => Self::Backspace,
            "clear" => Self::Clear,
            "commit" => Self::Commit,
            "cancel" => Self::Cancel,
            other => return Err(format!("unknown command `{other}`")),
        })
    }
}

/// The two key tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyMode {
    /// The manager: navigation and commands.
    Manager,
    /// A text line (search, filter).
    Input,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(s) => vec![s],
            Self::Many(v) => v,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Binding {
    on: OneOrMany,
    run: OneOrMany,
    #[serde(default)]
    desc: String,
}

#[derive(Debug, Default, Deserialize)]
struct Section {
    #[serde(default)]
    keymap: Vec<Binding>,
}

#[derive(Debug, Default, Deserialize)]
struct File {
    #[serde(default)]
    mgr: Section,
    #[serde(default)]
    input: Section,
}

/// One bound key sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// The keys, in order.
    pub keys: Vec<Key>,
    /// The commands to run.
    pub actions: Vec<Action>,
    /// One line for the help screen.
    pub desc: String,
}

/// The result of feeding one key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feed {
    /// A binding completed.
    Run(Vec<Action>),
    /// The keys so far are the start of a longer binding.
    Wait,
    /// Nothing is bound to these keys.
    Miss,
}

/// The merged bindings of both tables.
#[derive(Debug, Clone, Default)]
pub struct Keymap {
    tables: HashMap<KeyMode, Vec<Bound>>,
}

impl Keymap {
    /// The embedded default.
    ///
    /// # Panics
    /// Never in a shipped build: a unit test parses the embedded file.
    #[must_use]
    pub fn builtin() -> Self {
        Self::from_toml(DEFAULT_KEYMAP).expect("the embedded keymap parses")
    }

    /// Parses a keymap file.
    ///
    /// # Errors
    /// Fails on invalid TOML, an unknown key or an unknown command.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let file: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut keymap = Self::default();
        for (mode, section) in [(KeyMode::Manager, file.mgr), (KeyMode::Input, file.input)] {
            for binding in section.keymap {
                let keys = Key::parse_sequence(&binding.on.into_vec().concat())?;
                let actions = binding
                    .run
                    .into_vec()
                    .iter()
                    .map(|run| Action::parse(run))
                    .collect::<Result<Vec<_>, _>>()?;
                keymap.bind(
                    mode,
                    Bound {
                        keys,
                        actions,
                        desc: binding.desc,
                    },
                );
            }
        }
        Ok(keymap)
    }

    /// Adds a binding, replacing one with the same keys.
    pub fn bind(&mut self, mode: KeyMode, bound: Bound) {
        let table = self.tables.entry(mode).or_default();
        table.retain(|b| b.keys != bound.keys);
        table.push(bound);
    }

    /// Merges a user keymap on top of this one.
    pub fn merge(&mut self, other: Self) {
        for (mode, bounds) in other.tables {
            for bound in bounds {
                self.bind(mode, bound);
            }
        }
    }

    /// The bindings of one table, for the help screen.
    #[must_use]
    pub fn bindings(&self, mode: KeyMode) -> &[Bound] {
        self.tables.get(&mode).map_or(&[], Vec::as_slice)
    }

    /// Feeds one key to the sequence matcher. `pending` holds the keys of an unfinished
    /// sequence; it is cleared whenever the result is not [`Feed::Wait`].
    pub fn feed(&self, mode: KeyMode, pending: &mut Vec<Key>, key: Key) -> Feed {
        pending.push(key);
        let table = self.bindings(mode);
        if let Some(bound) = table.iter().find(|b| b.keys == *pending) {
            let actions = bound.actions.clone();
            pending.clear();
            return Feed::Run(actions);
        }
        if table
            .iter()
            .any(|b| b.keys.len() > pending.len() && b.keys.starts_with(pending))
        {
            return Feed::Wait;
        }
        // A failed sequence may still start a new one with its last key.
        let last = *pending.last().unwrap_or(&key);
        let restart = pending.len() > 1;
        pending.clear();
        if restart {
            return self.feed(mode, pending, last);
        }
        Feed::Miss
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_keymap_parses_and_has_no_conflict() {
        let keymap = Keymap::builtin();
        let table = keymap.bindings(KeyMode::Manager);
        assert!(table.len() > 40);
        for (i, a) in table.iter().enumerate() {
            for b in &table[i + 1..] {
                assert_ne!(a.keys, b.keys, "{:?} is bound twice", a.keys);
                let shorter = a.keys.len().min(b.keys.len());
                assert!(
                    a.keys[..shorter] != b.keys[..shorter] || a.keys.len() == b.keys.len(),
                    "{:?} shadows {:?}",
                    a.keys,
                    b.keys
                );
            }
        }
    }

    #[test]
    fn sequences_wait_then_run() {
        let keymap = Keymap::builtin();
        let mut pending = Vec::new();
        assert_eq!(
            keymap.feed(KeyMode::Manager, &mut pending, Key::char('g')),
            Feed::Wait
        );
        assert_eq!(
            keymap.feed(KeyMode::Manager, &mut pending, Key::char('g')),
            Feed::Run(vec![Action::Arrow(Step::Top)])
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn a_broken_sequence_restarts_with_its_last_key() {
        let keymap = Keymap::builtin();
        let mut pending = Vec::new();
        keymap.feed(KeyMode::Manager, &mut pending, Key::char('g'));
        assert_eq!(
            keymap.feed(KeyMode::Manager, &mut pending, Key::char('j')),
            Feed::Run(vec![Action::Arrow(Step::Lines(1))])
        );
    }

    #[test]
    fn a_user_binding_replaces_the_default() {
        let mut keymap = Keymap::builtin();
        let user = Keymap::from_toml("[mgr]\nkeymap = [{ on = \"q\", run = \"help\" }]\n").unwrap();
        keymap.merge(user);
        let mut pending = Vec::new();
        assert_eq!(
            keymap.feed(KeyMode::Manager, &mut pending, Key::char('q')),
            Feed::Run(vec![Action::Help])
        );
    }

    #[test]
    fn rejects_unknown_commands_and_keys() {
        assert!(Keymap::from_toml("[mgr]\nkeymap = [{ on = \"q\", run = \"frobnicate\" }]").is_err());
        assert!(Keymap::from_toml("[mgr]\nkeymap = [{ on = \"<Nope>\", run = \"quit\" }]").is_err());
    }
}
