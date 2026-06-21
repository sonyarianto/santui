use crate::plugin::PluginFactory;
use std::path::{Path, PathBuf};

use super::Santui;

impl Santui {
    /// Set the santui data directory (e.g. `~/.santui`).
    /// Called from main.rs before `run()`.
    pub fn set_data_dir(&mut self, dir: PathBuf) {
        self.plugin_manager.set_data_dir(dir);
    }

    /// Set the plugin factory (called from main.rs before run()).
    /// Used by PluginManager for hot-reload and on-demand plugin creation.
    pub fn set_plugin_factory(&mut self, factory: PluginFactory) {
        self.plugin_manager.set_factory(factory);
    }

    /// Register a default plugin (bundled with santui, e.g. the registry plugin).
    /// The plugin is created via the factory and registered without initialising.
    /// It will be initialised during `init_all` in `run()`.
    pub fn register_default_plugin(&mut self, id: &str, name: &str, path: &Path) {
        self.plugin_manager.register_new(id, name, path);
    }
}
