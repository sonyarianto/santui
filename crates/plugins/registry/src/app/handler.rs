use santui_ipc::protocol::{HostMsg, IpcKey, PluginMsg, PluginRequest};
use santui_registry::{plugin_filename, PluginManifest};

use super::state::{Action, App, DownloadEvent};

use std::sync::mpsc;

impl App {
    pub fn handle(&mut self, msg: HostMsg) -> PluginMsg {
        let mut request = None;

        match msg {
            HostMsg::Init {
                theme,
                area,
                data_dir,
            } => {
                self.theme = theme;
                self.area = area;
                let data = std::path::PathBuf::from(&data_dir);
                self.plugins_dir = data.join("plugins");
                let reg = santui_registry::Registry::new(data);
                self.registry = Some(reg);
                self.set_status("Fetching plugins…".to_string());
                if let Some(ref mut reg) = self.registry {
                    let dev = std::env::var("SANTUI_DEV").as_deref() == Ok("1");
                    if dev {
                        reg.set_dev_mode(true);
                        let path = std::env::var("SANTUI_DEV_MANIFEST")
                            .map(std::path::PathBuf::from)
                            .unwrap_or_else(|_| std::path::PathBuf::from("plugins.json"));
                        match reg.load_local_manifest(&path) {
                            Ok(()) => {
                                let s = reg.status.clone();
                                self.set_status(s);
                            }
                            Err(e) => self.set_status(format!("Error: {e}")),
                        }
                    } else {
                        match reg.fetch_manifest() {
                            Ok(()) => {
                                let s = reg.status.clone();
                                self.set_status(s);
                            }
                            Err(e) => self.set_status(format!("Error: {e}")),
                        }
                    }
                }
            }

            HostMsg::Focus => {
                self.status.clear();
                self.status_ticks = 0;
            }
            HostMsg::Blur => {}

            HostMsg::Key { key } => self.handle_key(key, &mut request),

            HostMsg::Tick => {
                self.tick_status();
                let rx = self.download_rx.take();
                if let Some(rx) = rx {
                    let mut done = false;
                    let mut error = None;
                    let mut progress = None;

                    loop {
                        match rx.try_recv() {
                            Ok(DownloadEvent::Progress(d, t)) => {
                                progress = Some((d, t));
                            }
                            Ok(DownloadEvent::Done) => {
                                done = true;
                                break;
                            }
                            Ok(DownloadEvent::Error(e)) => {
                                done = true;
                                error = Some(e);
                                break;
                            }
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                done = true;
                                break;
                            }
                        }
                    }

                    if let Some(p) = progress {
                        self.download_progress = Some(p);
                    }

                    if done {
                        self.download_rx = None;
                        self.download_progress = None;
                        if let Some(e) = error {
                            self.set_status(format!("Error: {e}"));
                            self.pending_install_id = None;
                            self.pending_install_name = None;
                            self.pending_install_version = None;
                        } else if let Some(ref mut reg) = self.registry {
                            if let (Some(id), Some(name), Some(version)) = (
                                self.pending_install_id.take(),
                                self.pending_install_name.take(),
                                self.pending_install_version.take(),
                            ) {
                                let target_path = self.plugins_dir.join(plugin_filename(&id));
                                match reg.add_installed(&id, &name, &version, target_path) {
                                    Ok(()) => {
                                        self.set_status(format!("{name} installed and enabled"));
                                        request = Some(PluginRequest::PluginsChanged);
                                    }
                                    Err(e) => self.set_status(format!("Error: {e}")),
                                }
                            }
                        }
                    } else {
                        self.download_rx = Some(rx);
                    }
                }
            }

            HostMsg::ThemeChange { theme } => {
                self.theme = theme;
            }

            HostMsg::Resize { area } => {
                self.area = area;
            }

            HostMsg::Shutdown => {
                return PluginMsg {
                    commands: vec![],
                    hints: vec![],
                    palette_commands: vec![],
                    request: None,
                }
            }

            HostMsg::UserUpdate { .. } => {}

            HostMsg::PaletteCommand { index: _ } => {}

            HostMsg::PluginMessage { .. } => {}
        }

        let commands = self.render_commands();
        let hints = self.hints();
        let palette_commands = vec![("System".into(), "Plugin Registry".into())];

        PluginMsg {
            commands,
            hints,
            palette_commands,
            request,
        }
    }

    fn handle_key(&mut self, key: IpcKey, request: &mut Option<PluginRequest>) {
        self.status.clear();
        if let Some(detail_idx) = self.detail_idx {
            self.handle_detail_key(key, detail_idx, request);
        } else {
            self.handle_list_key(key, request);
        }
    }

    fn handle_list_key(&mut self, key: IpcKey, _request: &mut Option<PluginRequest>) {
        match key {
            IpcKey::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.ensure_scroll_visible();
                }
            }
            IpcKey::Down => {
                let count = self.available_count().saturating_sub(1);
                if self.cursor < count {
                    self.cursor += 1;
                    self.ensure_scroll_visible();
                }
            }
            IpcKey::Enter | IpcKey::Char('d') | IpcKey::Char('D') => {
                let count = self.available_count();
                if self.cursor < count {
                    self.detail_idx = Some(self.cursor);
                    self.action_cursor = 0;
                }
            }
            IpcKey::Esc => {}
            IpcKey::Char('q') => {}
            _ => {}
        }
    }

    fn handle_detail_key(
        &mut self,
        key: IpcKey,
        detail_idx: usize,
        request: &mut Option<PluginRequest>,
    ) {
        let actions = self.available_actions(detail_idx);
        match key {
            IpcKey::Up => {
                if self.action_cursor > 0 {
                    self.action_cursor -= 1;
                }
            }
            IpcKey::Down => {
                let last = actions.len().saturating_sub(1);
                if self.action_cursor < last {
                    self.action_cursor += 1;
                }
            }
            IpcKey::Enter => {
                if self.action_cursor < actions.len() {
                    self.execute_action(detail_idx, actions[self.action_cursor], request);
                }
            }
            IpcKey::Esc | IpcKey::Backspace => {
                self.detail_idx = None;
            }
            _ => {}
        }
    }

    pub(super) fn available_actions(&self, idx: usize) -> Vec<Action> {
        let reg = match &self.registry {
            Some(r) => r,
            None => return vec![],
        };
        let plugin = match reg.available.get(idx) {
            Some(p) => p,
            None => return vec![],
        };
        let installed_idx = reg.installed.iter().position(|p| {
            p.path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.trim_end_matches(".exe"))
                == Some(&plugin.id)
        });
        let mut actions = Vec::new();
        match installed_idx {
            Some(i) => {
                if reg.installed[i].enabled {
                    actions.push(Action::Disable);
                } else {
                    actions.push(Action::Enable);
                }
                if reg.installed[i].version != plugin.version {
                    actions.push(Action::Update);
                }
                actions.push(Action::Delete);
            }
            None => {
                actions.push(Action::Install);
            }
        }
        actions
    }

    fn execute_action(&mut self, idx: usize, action: Action, request: &mut Option<PluginRequest>) {
        let reg = match self.registry.as_mut() {
            Some(r) => r,
            None => return,
        };
        let plugin = match reg.available.get(idx) {
            Some(p) => p.clone(),
            None => return,
        };
        let installed_idx = reg.installed.iter().position(|p| {
            p.path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.trim_end_matches(".exe"))
                == Some(&plugin.id)
        });

        match action {
            Action::Enable => {
                if let Some(i) = installed_idx {
                    if reg.set_enabled(i, true).is_ok() {
                        self.set_status(format!("{} enabled", plugin.name));
                        *request = Some(PluginRequest::PluginsChanged);
                    }
                }
                self.detail_idx = None;
            }
            Action::Disable => {
                if let Some(i) = installed_idx {
                    if reg.set_enabled(i, false).is_ok() {
                        self.set_status(format!("{} disabled", plugin.name));
                        *request = Some(PluginRequest::PluginsChanged);
                    }
                }
                self.detail_idx = None;
            }
            Action::Install | Action::Update => {
                self.spawn_install(&plugin);
                self.detail_idx = None;
            }
            Action::Delete => {
                if let Some(i) = installed_idx {
                    if reg.remove_installed(i).is_ok() {
                        self.set_status(format!("{} deleted", plugin.name));
                        *request = Some(PluginRequest::PluginsChanged);
                    }
                }
                self.detail_idx = None;
            }
        }
    }

    fn spawn_install(&mut self, plugin: &PluginManifest) {
        if self.download_rx.is_some() {
            self.set_status("Already downloading…".to_string());
            return;
        }

        let id = plugin.id.clone();
        let name = plugin.name.clone();
        let version = plugin.version.clone();
        let url = plugin.download_url.clone();
        let sha256 = plugin.sha256.clone();
        let dest = self.plugins_dir.join(plugin_filename(&id));
        let dev_mode = self.registry.as_ref().map(|r| r.dev_mode).unwrap_or(false);

        let (tx, rx) = mpsc::channel();
        self.download_rx = Some(rx);
        self.pending_install_id = Some(id.clone());
        self.pending_install_name = Some(name.clone());
        self.pending_install_version = Some(version.clone());
        self.download_progress = Some((0, 0));
        self.set_status(format!("Downloading {name}…"));

        std::thread::spawn(move || {
            if let Some(parent) = dest.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    log::warn!("failed to create plugin download directory: {e}");
                }
            }
            let result = if dev_mode {
                let src = std::path::Path::new(&url);
                std::fs::copy(src, &dest).map(|_| ()).map_err(|e| {
                    format!("Failed to copy plugin binary from {}: {e}", src.display())
                })
            } else {
                santui_registry::download_plugin(&url, &sha256, &dest, &|downloaded, total| {
                    let _ = tx.send(DownloadEvent::Progress(downloaded, total));
                })
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(DownloadEvent::Done);
                }
                Err(e) => {
                    let _ = tx.send(DownloadEvent::Error(e));
                }
            }
        });
    }
}
