use crate::plugin::PluginFactory;
use std::path::PathBuf;

use super::Santui;

impl Santui {
    /// Set the registry directory (called from main.rs before run()).
    pub fn set_registry_dir(&mut self, dir: PathBuf) {
        self.registry_controller.set_dir(dir);
    }

    /// Set the plugin factory (called from main.rs before run()).
    /// Used by PluginManager for hot-reload and on-demand plugin creation.
    pub fn set_plugin_factory(&mut self, factory: PluginFactory) {
        self.plugin_manager.set_factory(factory);
    }

    /// Fetch plugin manifest and prepare the registry screen.
    /// If `SANTUI_DEV=1` env is set, loads a local manifest from `SANTUI_DEV_MANIFEST`
    /// (defaults to `plugins.json` in cwd) and enables dev mode (local file copy).
    pub(super) fn open_registry(&mut self) {
        self.app_state.registry_open = true;
        self.registry_controller.open();
        self.plugin_manager
            .refresh_dynamic_items(self.registry_controller.registry_ref());
    }
}
