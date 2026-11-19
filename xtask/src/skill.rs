//! `cargo xtask skill [--check]`: writes `.claude/skills/nie/references/commands.md`, the table of every
//! `nie` command with its MCP tool, its owning plugin and the plugins of this build, or fails when the
//! committed table differs (`--check`, and the unit test that runs in the `ci-workspace` job).
//!
//! Three sources, read as text so the task needs no build of `nie-cli`:
//! - `crates/nie-cli/src/main.rs`: the `enum Cmd` of clap (name, alias, `cfg(feature)`);
//! - `crates/nie-cli/src/mcp.rs`: the `define_cli_tools!` table (tool, command) and the `#[tool]` tools;
//! - `plugins/*/*/src/lib.rs`: `Plugin::info` (id, package, commands), and `crates/nie-cli/src/plugins.rs`
//!   for the registration and its feature.
//!
//! The task fails when an MCP tool or a plugin names a command that clap does not have, when two
//! plugins own one command, or when a declared plugin is not registered.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};
use regex::Regex;

use crate::util::repo_root;

/// The generated file, relative to the repository root.
const OUTPUT: &str = ".claude/skills/nie/references/commands.md";

/// Arguments of `cargo xtask skill`.
#[derive(clap::Args)]
pub struct Args {
    /// Fail when the committed table is stale instead of rewriting it.
    #[arg(long)]
    check: bool,
}

/// One top-level clap command.
#[derive(Debug, PartialEq, Eq)]
struct Command {
    name: String,
    alias: Option<String>,
    feature: Option<String>,
}

/// One MCP tool.
#[derive(Debug, PartialEq, Eq)]
struct Tool {
    name: String,
    /// The CLI command the tool runs, `None` for a native tool.
    command: Option<String>,
    description: Option<String>,
}

/// One `Plugin::info` declaration.
#[derive(Debug, PartialEq, Eq)]
struct Plugin {
    id: String,
    package: String,
    commands: BTreeSet<String>,
}

/// kebab-case of a variant name, the clap default.
fn kebab(variant: &str) -> String {
    let mut out = String::new();
    for (i, c) in variant.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The variants of `enum Cmd` in `main.rs`.
fn cli_commands(main_rs: &str) -> Result<Vec<Command>> {
    let name_re = Regex::new(r#"\bname = "([^"]+)""#)?;
    let alias_re = Regex::new(r#"visible_alias = "([^"]+)""#)?;
    let feature_re = Regex::new(r#"^\s{4}#\[cfg\(feature = "([^"]+)"\)\]"#)?;
    let variant_re = Regex::new(r"^ {4}([A-Z][A-Za-z0-9]*)(\(|\s*\{|,)")?;
    let mut lines = main_rs.lines();
    if !lines.any(|l| l.trim_end() == "enum Cmd {") {
        bail!("main.rs has no `enum Cmd {{`");
    }
    let mut out = Vec::new();
    let (mut name, mut alias, mut feature) = (None, None, None);
    for line in lines {
        if line.trim_end() == "}" {
            return Ok(out);
        }
        if let Some(c) = feature_re.captures(line) {
            feature = Some(c[1].to_owned());
        } else if line.starts_with("    #[command(") {
            if let Some(c) = name_re.captures(line) {
                name = Some(c[1].to_owned());
            }
            if let Some(c) = alias_re.captures(line) {
                alias = Some(c[1].to_owned());
            }
        } else if let Some(c) = variant_re.captures(line) {
            out.push(Command {
                name: name.take().unwrap_or_else(|| kebab(&c[1])),
                alias: alias.take(),
                feature: feature.take(),
            });
        }
    }
    bail!("`enum Cmd` of main.rs is not closed")
}

/// First sentence of an MCP description.
fn first_sentence(text: &str) -> String {
    let end = text.find(". ").unwrap_or_else(|| text.trim_end_matches('.').len());
    text[..end].trim_end_matches('.').to_owned()
}

/// The MCP tools of `mcp.rs`: the `define_cli_tools!` rows, then the `#[tool]` ones.
fn mcp_tools(mcp_rs: &str) -> Result<Vec<Tool>> {
    let cli = Regex::new(r#"\(\s*\w+,\s*"(cli_[a-z0-9_]+)",\s*"([a-z0-9-]+)",\s*"((?:[^"\\]|\\.)*)"\s*\)"#)?;
    let native = Regex::new(
        r#"#\[tool\(\s*name = "([a-z0-9_]+)",\s*description = "((?:[^"\\]|\\.)*)"\s*\)\]"#,
    )?;
    let mut tools: Vec<Tool> = cli
        .captures_iter(mcp_rs)
        .map(|c| Tool {
            name: c[1].to_owned(),
            command: Some(c[2].to_owned()),
            description: Some(first_sentence(&c[3])),
        })
        .collect();
    tools.extend(native.captures_iter(mcp_rs).map(|c| Tool {
        name: c[1].to_owned(),
        command: None,
        description: Some(first_sentence(&c[2])),
    }));
    if tools.is_empty() {
        bail!("mcp.rs declares no tool");
    }
    Ok(tools)
}

/// The string literals of a `&[...]` list.
fn literals(list: &str, strings: &Regex) -> Vec<String> {
    strings.captures_iter(list).map(|c| c[1].to_owned()).collect()
}

/// The `Plugin::info` of one `lib.rs`, when it has one.
fn plugin_info(lib_rs: &str) -> Result<Option<Plugin>> {
    let Some(at) = lib_rs.find("PluginInfo {") else {
        return Ok(None);
    };
    let body = &lib_rs[at..];
    let body = &body[..body.find('}').unwrap_or(body.len())];
    let field = |name: &str| -> Result<Option<String>> {
        let re = Regex::new(&format!(r#"\b{name}:\s*"([^"]+)""#))?;
        Ok(re.captures(body).map(|c| c[1].to_owned()))
    };
    let (Some(id), Some(package)) = (field("id")?, field("package")?) else {
        return Ok(None);
    };
    let strings = Regex::new(r#""([^"]+)""#)?;
    let Some(cmds) = Regex::new(r"commands:\s*(&\[[^\]]*\]|[A-Z][A-Z0-9_]*)")?.captures(body) else {
        bail!("plugin `{id}` has no `commands` field");
    };
    let mut commands: BTreeSet<String> = BTreeSet::new();
    let spec = &cmds[1];
    if spec.starts_with('&') {
        commands.extend(literals(spec, &strings));
    } else {
        // A named constant, possibly one per cfg: every definition counts.
        let def = Regex::new(&format!(r"const {spec}:[^=]*=\s*&\[([^\]]*)\]"))?;
        for d in def.captures_iter(lib_rs) {
            commands.extend(literals(&d[1], &strings));
        }
        if commands.is_empty() {
            bail!("plugin `{id}`: constant `{spec}` not found");
        }
    }
    Ok(Some(Plugin {
        id,
        package,
        commands,
    }))
}

/// Every plugin under `plugins/<group>/<crate>/src/lib.rs`, by package name.
fn plugins(root: &Path) -> Result<Vec<Plugin>> {
    let mut out = Vec::new();
    let mut groups: Vec<_> = std::fs::read_dir(root.join("plugins"))
        .context("read plugins/")?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    groups.sort();
    for group in groups {
        let mut members: Vec<_> = std::fs::read_dir(&group)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        members.sort();
        for member in members {
            let lib = member.join("src/lib.rs");
            let Ok(text) = std::fs::read_to_string(&lib) else {
                continue;
            };
            if let Some(p) = plugin_info(&text).with_context(|| lib.display().to_string())? {
                out.push(p);
            }
        }
    }
    Ok(out)
}

/// `package -> feature` of the plugins that `plugins.rs` registers (`None`: always).
fn registrations(plugins_rs: &str) -> Result<BTreeMap<String, Option<String>>> {
    let cfg = Regex::new(r#"^\s*#\[cfg\(feature = "([^"]+)"\)\]"#)?;
    let boxed = Regex::new(r"Box::new\(\s*([a-z0-9_]+)::")?;
    let mut out = BTreeMap::new();
    let (mut pending, mut active): (Option<String>, Option<String>) = (None, None);
    for line in plugins_rs.lines() {
        if let Some(c) = cfg.captures(line) {
            pending = Some(c[1].to_owned());
        } else if line.trim() == "{" && pending.is_some() {
            active = pending.take();
        } else if line.starts_with("    }") {
            active = None;
        } else if let Some(c) = boxed.captures(line) {
            out.insert(c[1].replace('_', "-"), active.clone());
        }
    }
    Ok(out)
}

/// Escapes the cell separator.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// The table, or the first inconsistency between the three sources.
fn render_tables(
    commands: &[Command],
    tools: &[Tool],
    plugins: &[Plugin],
    registered: &BTreeMap<String, Option<String>>,
) -> Result<String> {
    let mut by_name: BTreeMap<&str, &Command> = BTreeMap::new();
    for c in commands {
        by_name.insert(&c.name, c);
        if let Some(a) = &c.alias {
            by_name.insert(a, c);
        }
    }
    let canonical = |name: &str| by_name.get(name).map(|c| c.name.clone());
    let mut tool_of: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for t in tools {
        if let Some(command) = &t.command {
            let Some(name) = canonical(command) else {
                bail!("MCP tool `{}` runs `{command}`, which `enum Cmd` does not have", t.name);
            };
            tool_of.entry(name).or_default().push(&t.name);
        }
    }
    let mut owner: BTreeMap<String, &str> = BTreeMap::new();
    for p in plugins {
        for command in &p.commands {
            let Some(name) = canonical(command) else {
                bail!("plugin `{}` owns `{command}`, which `enum Cmd` does not have", p.id);
            };
            if let Some(first) = owner.insert(name, &p.id) {
                bail!("`{command}` is owned by plugins `{first}` and `{}`", p.id);
            }
        }
        if !registered.contains_key(&p.package) {
            bail!("plugin `{}` ({}) is not registered in plugins.rs", p.id, p.package);
        }
    }
    for package in registered.keys() {
        if !plugins.iter().any(|p| &p.package == package) {
            bail!("plugins.rs registers `{package}`, which declares no Plugin::info");
        }
    }

    let mut text = String::new();
    text.push_str(
        "<!-- Generated by `cargo xtask skill` (`just skill`) from crates/nie-cli/src/main.rs, mcp.rs and\n     plugins.rs and from plugins/*/*/src/lib.rs. Do not edit; run the task. -->\n\n",
    );
    text.push_str("# nie commands, MCP tools and plugins\n\n");
    text.push_str("## Commands\n\n");
    text.push_str("`core` is a command of `nie-cli` that no plugin owns. Build feature: `always` is in every build, `host` is the full CLI, `term` adds the GPU terminal.\n\n");
    text.push_str("| command | build feature | MCP tool | owner |\n|---|---|---|---|\n");
    let mut sorted: Vec<&Command> = commands.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for c in sorted {
        let name = c
            .alias
            .as_ref()
            .map_or_else(|| format!("`{}`", c.name), |a| format!("`{}` (alias `{a}`)", c.name));
        let mcp = tool_of
            .get(&c.name)
            .map_or_else(|| "-".to_owned(), |t| t.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", "));
        let _ = writeln!(
            text,
            "| {name} | {} | {mcp} | {} |",
            c.feature.as_deref().unwrap_or("always"),
            owner.get(&c.name).copied().unwrap_or("core"),
        );
    }
    text.push_str("\n## Plugins\n\n");
    text.push_str("| plugin | package | registered with feature | commands |\n|---|---|---|---|\n");
    let mut sorted: Vec<&Plugin> = plugins.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    for p in sorted {
        let commands = if p.commands.is_empty() {
            "none (a program, not a CLI command)".to_owned()
        } else {
            p.commands.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(" ")
        };
        let feature = registered
            .get(&p.package)
            .and_then(Option::as_deref)
            .unwrap_or("always");
        let _ = writeln!(text, "| `{}` | `{}` | {feature} | {commands} |", p.id, p.package);
    }
    text.push_str("\n## MCP tools without a CLI command\n\n");
    text.push_str("| tool | what it does |\n|---|---|\n");
    let mut native: Vec<&Tool> = tools.iter().filter(|t| t.command.is_none()).collect();
    native.sort_by(|a, b| a.name.cmp(&b.name));
    for t in native {
        let _ = writeln!(text, "| `{}` | {} |", t.name, cell(t.description.as_deref().unwrap_or("")));
    }
    Ok(text)
}

/// The text of the generated file for the tree under `root`.
fn render(root: &Path) -> Result<String> {
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).with_context(|| format!("read {rel}"));
    render_tables(
        &cli_commands(&read("crates/nie-cli/src/main.rs")?)?,
        &mcp_tools(&read("crates/nie-cli/src/mcp.rs")?)?,
        &plugins(root)?,
        &registrations(&read("crates/nie-cli/src/plugins.rs")?)?,
    )
}

/// The committed file, with Windows checkouts normalised to `\n`.
fn committed(root: &Path) -> String {
    std::fs::read_to_string(root.join(OUTPUT))
        .unwrap_or_default()
        .replace("\r\n", "\n")
}

/// Writes or checks the table.
pub fn run(args: Args) -> Result<()> {
    let root = repo_root();
    let text = render(&root)?;
    if args.check {
        if committed(&root) == text {
            return Ok(());
        }
        bail!("{OUTPUT} is stale, run `just skill`");
    }
    let path = root.join(OUTPUT);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, &text).with_context(|| format!("write {}", path.display()))?;
    println!("{OUTPUT}: {} lines", text.lines().count());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: &str = r#"
enum Cmd {
    /// Doc.
    #[command(name = "serve")]
    Serve(Args),
    #[cfg(feature = "host")]
    #[command(name = "oc", visible_alias = "ocgen")]
    Ocgen {
        #[command(subcommand)]
        op: Op,
    },
    #[cfg(feature = "host")]
    RefreshTypedJson {
        #[arg(long)]
        force: bool,
    },
    Mcp,
}
"#;

    #[test]
    fn clap_names_aliases_and_features() {
        let got = cli_commands(MAIN).unwrap();
        let names: Vec<_> = got.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["serve", "oc", "refresh-typed-json", "mcp"]);
        assert_eq!(got[1].alias.as_deref(), Some("ocgen"));
        assert_eq!(got[1].feature.as_deref(), Some("host"));
        assert_eq!(got[0].feature, None);
    }

    #[test]
    fn mcp_rows_and_native_tools() {
        let src = r#"
define_cli_tools!(
    (
        CliOc,
        "cli_oc",
        "oc",
        "Generate a character. Second sentence."
    ),
);
    #[tool(
        name = "vfs_list",
        description = "List a prefix. More."
    )]
"#;
        let tools = mcp_tools(src).unwrap();
        assert_eq!(tools[0].name, "cli_oc");
        assert_eq!(tools[0].command.as_deref(), Some("oc"));
        assert_eq!(tools[1].name, "vfs_list");
        assert_eq!(tools[1].description.as_deref(), Some("List a prefix"));
    }

    #[test]
    fn plugin_commands_from_a_list_and_from_a_constant() {
        let list = r#"PluginInfo { id: "save", summary: "S", package: "nie-save", commands: &["save"], }"#;
        let p = plugin_info(list).unwrap().unwrap();
        assert_eq!((p.id.as_str(), p.package.as_str()), ("save", "nie-save"));
        assert!(p.commands.contains("save"));
        let konst = r#"
#[cfg(feature = "term")]
const COMMANDS: &[&str] = &["fm", "term"];
#[cfg(not(feature = "term"))]
const COMMANDS: &[&str] = &["fm"];
fn info() -> PluginInfo { PluginInfo { id: "tui", summary: "T", package: "nie-tui", commands: COMMANDS, } }
"#;
        let p = plugin_info(konst).unwrap().unwrap();
        assert_eq!(p.commands.iter().map(String::as_str).collect::<Vec<_>>(), ["fm", "term"]);
        assert_eq!(plugin_info("fn nothing() {}").unwrap(), None);
    }

    #[test]
    fn registrations_carry_their_feature() {
        let src = r#"
    let mut registry = Registry::new();
    #[cfg(feature = "cloud")]
    {
        registry
            .register(Box::new(nie_save::SavePlugin))
            .expect("save plugin");
    }
    #[cfg(feature = "host")]
    {
        registry
            .register(Box::new(nie_re_core::ExePlugin))
            .expect("exe plugin");
    }
"#;
        let got = registrations(src).unwrap();
        assert_eq!(got["nie-save"].as_deref(), Some("cloud"));
        assert_eq!(got["nie-re-core"].as_deref(), Some("host"));
    }

    #[test]
    fn a_tool_for_a_missing_command_fails() {
        let commands = cli_commands(MAIN).unwrap();
        let tools = vec![Tool {
            name: "cli_gone".into(),
            command: Some("gone".into()),
            description: None,
        }];
        let err = render_tables(&commands, &tools, &[], &BTreeMap::new()).unwrap_err();
        assert!(err.to_string().contains("cli_gone"));
    }

    #[test]
    fn a_command_with_two_owners_fails() {
        let commands = cli_commands(MAIN).unwrap();
        let plugin = |id: &str| Plugin {
            id: id.into(),
            package: format!("nie-{id}"),
            commands: ["mcp".to_owned()].into(),
        };
        let registered = BTreeMap::from([("nie-a".to_owned(), None), ("nie-b".to_owned(), None)]);
        let err = render_tables(&commands, &[], &[plugin("a"), plugin("b")], &registered).unwrap_err();
        assert!(err.to_string().contains("owned by plugins"));
    }

    #[test]
    fn the_committed_table_is_current() {
        let root = repo_root();
        assert_eq!(
            committed(&root),
            render(&root).unwrap(),
            "{OUTPUT} is stale, run `just skill`"
        );
    }
}
