use std::path::PathBuf;

use santui_registry::Registry as PluginRegistry;

use super::registry_screen::RegistryScreen;
use crate::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

pub(super) enum RegistryAction {
    Close,
    ItemsChanged,
    None,
}

pub(super) struct RegistryController {
    registry: Option<PluginRegistry>,
    screen: RegistryScreen,
}

impl RegistryController {
    pub fn new() -> Self {
        Self {
            registry: None,
            screen: RegistryScreen::new(),
        }
    }

    pub fn set_dir(&mut self, dir: PathBuf) {
        self.registry = Some(PluginRegistry::new(dir));
    }

    pub fn registry_ref(&self) -> &Option<PluginRegistry> {
        &self.registry
    }

    pub fn open(&mut self) {
        let rs = &mut self.screen;
        rs.status = "Fetching plugins…".to_string();
        rs.cursor = 0;
        rs.scroll = 0;

        if let Some(ref mut reg) = self.registry {
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

    pub fn ensure_scroll_visible(&mut self) {
        let available = self
            .registry
            .as_ref()
            .map(|r| r.available.len())
            .unwrap_or(0);
        self.screen.ensure_scroll_visible(available);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> RegistryAction {
        match key.code {
            KeyCode::Esc => RegistryAction::Close,
            KeyCode::Down => {
                if let Some(ref reg) = self.registry {
                    if !reg.available.is_empty() {
                        let max = reg.available.len().saturating_sub(1);
                        self.screen.cursor = (self.screen.cursor + 1).min(max);
                        self.ensure_scroll_visible();
                    }
                }
                RegistryAction::None
            }
            KeyCode::Up => {
                if self.screen.cursor > 0 {
                    self.screen.cursor -= 1;
                }
                self.ensure_scroll_visible();
                RegistryAction::None
            }
            KeyCode::Enter => {
                let plugin = self
                    .registry
                    .as_ref()
                    .and_then(|reg| reg.available.get(self.screen.cursor).cloned());
                if let Some(plugin) = plugin {
                    if let Some(ref mut reg) = self.registry {
                        let installed_idx = reg.installed.iter().position(|p| {
                            p.path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .map(|s| s.trim_end_matches(".exe"))
                                == Some(&plugin.id)
                        });
                        if let Some(idx) = installed_idx {
                            let current = reg.installed[idx].enabled;
                            let _ = reg.set_enabled(idx, !current);
                            self.screen.status = if !current {
                                format!("{} enabled", plugin.name)
                            } else {
                                format!("{} disabled", plugin.name)
                            };
                        } else {
                            self.screen.status = format!("Downloading {}…", plugin.name);
                            match reg.install(&plugin) {
                                Ok(()) => {
                                    self.screen.status =
                                        format!("{} installed and enabled", plugin.name);
                                }
                                Err(e) => {
                                    self.screen.status = format!("Error: {e}");
                                }
                            }
                        }
                        return RegistryAction::ItemsChanged;
                    }
                }
                RegistryAction::None
            }
            _ => RegistryAction::None,
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect, theme: &Theme) {
        self.screen.render(f, area, theme, &self.registry);
    }
}

impl Default for RegistryController {
    fn default() -> Self {
        Self::new()
    }
}
