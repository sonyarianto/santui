use crate::plugin::PluginFactory;
use santui_registry::Registry as PluginRegistry;
use std::path::PathBuf;

use super::Santui;

impl Santui {
    /// Set the registry directory (called from main.rs before run()).
    pub fn set_registry_dir(&mut self, dir: PathBuf) {
        self.registry = Some(PluginRegistry::new(dir));
    }

    /// Set the plugin factory (called from main.rs before run()).
    pub fn set_plugin_factory(&mut self, factory: PluginFactory) {
        self.plugin_factory = Some(factory);
    }

    pub(super) fn ensure_registry_scroll_visible(&mut self) {
        let available = self
            .registry
            .as_ref()
            .map(|r| r.available.len())
            .unwrap_or(0);
        self.registry_screen.ensure_scroll_visible(available);
    }
    /// Fetch plugin manifest and prepare the registry screen.
    /// If `SANTUI_DEV=1` env is set, loads a local manifest from `SANTUI_DEV_MANIFEST`
    /// (defaults to `plugins.json` in cwd) and enables dev mode (local file copy).
    pub(super) fn open_registry(&mut self) {
        let rs = &mut self.registry_screen;
        rs.open = true;
        rs.status = "Fetching plugins…".to_string();
        rs.cursor = 0;
        rs.scroll = 0;

        if let Some(ref mut reg) = self.registry {
            // Check if we're in dev mode.
            if std::env::var("SANTUI_DEV").as_deref() == Ok("1") {
                reg.set_dev_mode(true);
                let manifest_path = std::env::var("SANTUI_DEV_MANIFEST")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("plugins.json"));
                rs.status = format!("[DEV] Loading {}…", manifest_path.display());
                match reg.load_local_manifest(&manifest_path) {
                    Ok(()) => {
                        if let Err(e) = reg.sync_all_native_deps() {
                            rs.status = format!("[DEV] Warning: {e}");
                        } else {
                            rs.status = reg.status.clone();
                        }
                    }
                    Err(e) => {
                        rs.status = format!("[DEV] Error: {e}");
                    }
                }
            } else {
                match reg.fetch_manifest() {
                    Ok(()) => {
                        rs.status = reg.status.clone();
                    }
                    Err(e) => {
                        rs.status = format!("Error: {e}");
                    }
                }
            }
        }
    }
}
