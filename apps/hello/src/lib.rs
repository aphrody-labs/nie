//! hello: command line app on the nie framework.

#![forbid(unsafe_code)]

/// The plugin declaration of this app: what the nie plugin registry reads.
pub struct HelloPlugin;

impl nie_plugin::Plugin for HelloPlugin {
    fn info(&self) -> nie_plugin::PluginInfo {
        nie_plugin::PluginInfo {
            id: "hello",
            summary: "Command line app",
            package: "hello",
            commands: &["hello"],
        }
    }
}

/// Runs the command line: returns the exit code and the text for stdout.
#[must_use]
pub fn run(args: &[String]) -> (i32, String) {
    match args.first().map(String::as_str) {
        None | Some("hello") => (0, format!("hello from {}\n", env!("CARGO_PKG_NAME"))),
        Some("--version") => (0, format!("{}\n", env!("CARGO_PKG_VERSION"))),
        Some(other) => (2, format!("unknown command `{other}`\n")),
    }
}
