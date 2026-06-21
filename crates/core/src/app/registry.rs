use crate::plugin::PluginFactory;
use std::path::PathBuf;

use super::Santui;

impl Santui {
    /// Set the registry directory (called from main.rs before run()).
    pub fn set_registry_dir(&mut self, dir: PathBuf) {
        self.registry_controller.set_dir(dir);
    }

    /// Set the plugin factory (called from main.rs before run()).
    /// Also forwards to `PluginManager` so it can recreate plugins during hot-reload.
    pub fn set_plugin_factory(&mut self, factory: PluginFactory) {
        self.plugin_factory = Some(factory.clone());
        self.plugin_manager.set_factory(factory);
    }

    /// Fetch plugin manifest and prepare the registry screen.
    /// If `SANTUI_DEV=1` env is set, loads a local manifest from `SANTUI_DEV_MANIFEST`
    /// (defaults to `plugins.json` in cwd) and enables dev mode (local file copy).
    pub(super) fn open_registry(&mut self) {
        self.app_state.registry_open = true;
        self.registry_controller.open();
    }
}
