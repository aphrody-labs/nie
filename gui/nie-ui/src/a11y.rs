//! Accessibility: the reader's preferences and the audit of the AccessKit tree.
//!
//! GPUI builds an AccessKit tree from every element that has an id and a role. This module adds
//! two things on top. [`Settings`] is the global the theme and the components read for high
//! contrast, scaling and reduced motion, and [`audit`] checks a dumped tree
//! (`Window::debug_a11y_tree_json`) so that CI fails when a node that a screen reader announces has
//! no accessible name.

use gpui::{App, Global};
use serde_json::Value;

use crate::theme::{Preferences, Theme};

/// The theme and the preferences of the running app.
#[derive(Debug, Clone)]
pub struct Settings {
    /// What the reader asked for.
    pub preferences: Preferences,
    /// The theme derived for those preferences.
    pub theme: Theme,
}

impl Global for Settings {}

impl Settings {
    /// Installs the settings as the app global.
    pub fn install(cx: &mut App, theme: Theme) {
        cx.set_global(Self { preferences: theme.preferences, theme });
    }

    /// The installed theme.
    pub fn theme(cx: &App) -> &Theme {
        &cx.global::<Self>().theme
    }
}

/// Roles that a screen reader announces by name: a node with one of them and no name is a defect.
const NAMED_ROLES: &[&str] = &[
    "Button", "CheckBox", "ColumnHeader", "ComboBox", "Cell", "Heading", "Image", "Link", "List",
    "ListBox", "ListBoxOption", "ListItem", "Menu", "MenuItem", "RadioButton", "Row", "SearchInput",
    "Slider", "SpinButton", "Switch", "Tab", "TabList", "TabPanel", "Table", "TextInput", "Tree",
    "TreeItem", "Terminal", "Document", "Status", "Application", "Window", "Dialog", "Toolbar", "Log",
];

/// The result of [`audit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Nodes in the tree.
    pub nodes: usize,
    /// Nodes with a role that must be named.
    pub named_roles: usize,
    /// Keys of the nodes that have such a role and no name, with the role.
    pub unnamed: Vec<(String, String)>,
}

impl Report {
    /// Whether every node that must be named has a name.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.unnamed.is_empty()
    }
}

/// Checks a tree dumped by `Window::debug_a11y_tree_json`.
///
/// # Errors
/// The text is not the JSON of a dumped tree.
pub fn audit(json: &str) -> anyhow::Result<Report> {
    let doc: Value = serde_json::from_str(json)?;
    let nodes = doc["nodes"].as_object().ok_or_else(|| anyhow::anyhow!("no `nodes` object"))?;
    let mut report = Report { nodes: nodes.len(), named_roles: 0, unnamed: Vec::new() };
    for (key, node) in nodes {
        let aria = node.get("aria").unwrap_or(node);
        let role = aria["role"].as_str().unwrap_or_default();
        if !NAMED_ROLES.contains(&role) {
            continue;
        }
        report.named_roles += 1;
        let named = ["label", "value"]
            .iter()
            .any(|k| aria[*k].as_str().is_some_and(|s| !s.trim().is_empty()));
        if !named {
            report.unnamed.push((key.clone(), role.to_owned()));
        }
    }
    report.unnamed.sort();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_button_without_a_name_is_reported() {
        let json = r#"{"nodes":{
            "n0":{"aria":{"role":"Application","label":"nie"}},
            "n1":{"aria":{"role":"Button","label":"Open"}},
            "n2":{"aria":{"role":"Button"}},
            "n3":{"aria":{"role":"GenericContainer"}},
            "n4":{"aria":{"role":"TextInput","value":"abc"}}}}"#;
        let r = audit(json).unwrap();
        assert_eq!(r.nodes, 5);
        assert_eq!(r.named_roles, 4);
        assert_eq!(r.unnamed, vec![("n2".to_owned(), "Button".to_owned())]);
        assert!(!r.is_clean());
    }
}
