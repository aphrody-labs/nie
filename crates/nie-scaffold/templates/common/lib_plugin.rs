/// The plugin declaration of this app: what the nie plugin registry reads.
pub struct @Pascal@Plugin;

impl nie_plugin::Plugin for @Pascal@Plugin {
    fn info(&self) -> nie_plugin::PluginInfo {
        nie_plugin::PluginInfo {
            id: "@id@",
            summary: "@summary@",
            package: "@name@",
            commands: &["@id@"],
        }
    }
}
