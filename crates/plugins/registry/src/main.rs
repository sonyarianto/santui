use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginMsg, PluginRequest, RenderCmd, ThemeData};
use santui_registry::{plugin_filename, Registry};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc;

enum DownloadEvent {
    Progress(u64, u64),
    Done,
    Error(String),
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .init();

    let mut app = RegistryApp::new();
    let stdin = io::stdin();
    let mut line = String::new();

    loop {
        line.clear();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: HostMsg = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("[registry-plugin] Failed to parse HostMsg: {e}");
                        continue;
                    }
                };
                let response = app.handle(msg);
                let json = serde_json::to_string(&response).expect("PluginMsg serialization");
                let mut out = io::stdout().lock();
                let _ = writeln!(out, "{json}");
                let _ = out.flush();
            }
        }
    }
}

// ── State ──────────────────────────────────────────────────────────────

struct RegistryApp {
    registry: Option<Registry>,
    cursor: usize,
    scroll: u16,
    status: String,
    detail_idx: Option<usize>,
    theme: ThemeData,
    area: Area,
    plugins_dir: PathBuf,
    download_rx: Option<mpsc::Receiver<DownloadEvent>>,
    download_progress: Option<(u64, u64)>,
    pending_install_id: Option<String>,
    pending_install_name: Option<String>,
    pending_install_version: Option<String>,
}

impl RegistryApp {
    fn new() -> Self {
        RegistryApp {
            registry: None,
            cursor: 0,
            scroll: 0,
            status: String::new(),
            detail_idx: None,
            theme: ThemeData {
                text: [220; 3],
                text_muted: [140; 3],
                accent: [150; 3],
                highlight: [250; 3],
                logo: [255; 3],
                background: [20; 3],
                background_panel: [20; 3],
                background_overlay: [10; 3],
                border: [250; 3],
                success: [120; 3],
                error: [220; 3],
                inverted_text: [20; 3],
            },
            area: Area { w: 80, h: 24 },
            plugins_dir: PathBuf::new(),
            download_rx: None,
            download_progress: None,
            pending_install_id: None,
            pending_install_name: None,
            pending_install_version: None,
        }
    }
}

// ── Message handling ───────────────────────────────────────────────────

impl RegistryApp {
    fn handle(&mut self, msg: HostMsg) -> PluginMsg {
        let mut request = None;

        match msg {
            HostMsg::Init {
                theme,
                area,
                data_dir,
            } => {
                self.theme = theme;
                self.area = area;
                let data = PathBuf::from(&data_dir);
                self.plugins_dir = data.join("plugins");
                let reg = Registry::new(data);
                self.registry = Some(reg);
                self.status = "Fetching plugins…".to_string();
                if let Some(ref mut reg) = self.registry {
                    let dev = std::env::var("SANTUI_DEV").as_deref() == Ok("1");
                    if dev {
                        let path = std::env::var("SANTUI_DEV_MANIFEST")
                            .map(PathBuf::from)
                            .unwrap_or_else(|_| PathBuf::from("plugins.json"));
                        match reg.load_local_manifest(&path) {
                            Ok(()) => self.status = reg.status.clone(),
                            Err(e) => self.status = format!("[DEV] Error: {e}"),
                        }
                    } else {
                        match reg.fetch_manifest() {
                            Ok(()) => self.status = reg.status.clone(),
                            Err(e) => self.status = format!("Error: {e}"),
                        }
                    }
                }
            }

            HostMsg::Focus | HostMsg::Blur => {}

            HostMsg::Key { key } => self.handle_key(key, &mut request),

            HostMsg::Tick => {
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
                            self.status = format!("Error: {e}");
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
                                        self.status = format!("{name} installed and enabled");
                                        request = Some(PluginRequest::PluginsChanged);
                                    }
                                    Err(e) => self.status = format!("Error: {e}"),
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

            HostMsg::PaletteCommand { index: _ } => {
                // Focus already activates us via the host.
            }

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
        if let Some(detail_idx) = self.detail_idx {
            self.handle_detail_key(key, detail_idx, request);
        } else {
            self.handle_list_key(key, request);
        }
    }

    fn handle_list_key(&mut self, key: IpcKey, request: &mut Option<PluginRequest>) {
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
            IpcKey::Enter => {
                if let Some(ref mut reg) = self.registry {
                    if let Some(plugin) = reg.available.get(self.cursor).cloned() {
                        let installed_idx = reg.installed.iter().position(|p| {
                            p.path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .map(|s| s.trim_end_matches(".exe"))
                                == Some(&plugin.id)
                        });
                        match installed_idx {
                            Some(idx) => {
                                let current = reg.installed[idx].enabled;
                                match reg.set_enabled(idx, !current) {
                                    Ok(()) => {
                                        self.status = if !current {
                                            format!("{} enabled", plugin.name)
                                        } else {
                                            format!("{} disabled", plugin.name)
                                        };
                                        *request = Some(PluginRequest::PluginsChanged);
                                    }
                                    Err(e) => self.status = format!("Error: {e}"),
                                }
                            }
                            None => self.spawn_install(&plugin),
                        }
                    }
                }
            }
            IpcKey::Esc => {
                // The host handles Esc to deactivate the plugin, so nothing here.
            }
            IpcKey::Backspace | IpcKey::Char('d') | IpcKey::Char('D') => {
                let count = self.available_count();
                if self.cursor < count {
                    self.detail_idx = Some(self.cursor);
                }
            }
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
        match key {
            IpcKey::Esc | IpcKey::Backspace => {
                self.detail_idx = None;
            }
            IpcKey::Enter => {
                if let Some(ref mut reg) = self.registry {
                    if let Some(plugin) = reg.available.get(detail_idx).cloned() {
                        let installed_idx = reg.installed.iter().position(|p| {
                            p.path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .map(|s| s.trim_end_matches(".exe"))
                                == Some(&plugin.id)
                        });
                        match installed_idx {
                            Some(idx) => {
                                let current = reg.installed[idx].enabled;
                                match reg.set_enabled(idx, !current) {
                                    Ok(()) => {
                                        self.status = if !current {
                                            format!("{} enabled", plugin.name)
                                        } else {
                                            format!("{} disabled", plugin.name)
                                        };
                                        *request = Some(PluginRequest::PluginsChanged);
                                    }
                                    Err(e) => self.status = format!("Error: {e}"),
                                }
                            }
                            None => self.spawn_install(&plugin),
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn available_count(&self) -> usize {
        self.registry
            .as_ref()
            .map(|r| r.available.len())
            .unwrap_or(0)
    }

    fn ensure_scroll_visible(&mut self) {
        let list_h = max_list_h(self.area.h)
            .min(self.available_count() as u16)
            .max(1);
        let cursor = self.cursor.min(self.available_count().saturating_sub(1)) as u16;
        if cursor < self.scroll {
            self.scroll = cursor;
        } else if cursor >= self.scroll + list_h {
            self.scroll = cursor.saturating_sub(list_h.saturating_sub(1));
        }
    }

    fn spawn_install(&mut self, plugin: &santui_registry::PluginManifest) {
        if self.download_rx.is_some() {
            self.status = "Already downloading…".to_string();
            return;
        }

        let id = plugin.id.clone();
        let name = plugin.name.clone();
        let version = plugin.version.clone();
        let url = plugin.download_url.clone();
        let sha256 = plugin.sha256.clone();
        let dest = self.plugins_dir.join(plugin_filename(&id));

        let (tx, rx) = mpsc::channel();
        self.download_rx = Some(rx);
        self.pending_install_id = Some(id.clone());
        self.pending_install_name = Some(name.clone());
        self.pending_install_version = Some(version.clone());
        self.download_progress = Some((0, 0));
        self.status = format!("Downloading {name}…");

        std::thread::spawn(move || {
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let result =
                santui_registry::download_plugin(&url, &sha256, &dest, &|downloaded, total| {
                    let _ = tx.send(DownloadEvent::Progress(downloaded, total));
                });
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

// ── Rendering ──────────────────────────────────────────────────────────

impl RegistryApp {
    fn fg(&self, color: [u8; 3]) -> Option<[u8; 3]> {
        Some(color)
    }

    fn bg(&self) -> Option<[u8; 3]> {
        Some(self.theme.background_panel)
    }

    fn hints(&self) -> Vec<(String, String)> {
        if self.detail_idx.is_some() {
            vec![
                ("Enter".into(), "Install/Toggle".into()),
                ("Esc".into(), "Back".into()),
            ]
        } else {
            vec![
                ("↑↓".into(), "Navigate".into()),
                ("Enter".into(), "Install/Toggle".into()),
                ("d".into(), "Details".into()),
            ]
        }
    }

    fn render_commands(&self) -> Vec<RenderCmd> {
        let mut cmds = Vec::new();

        if let Some(detail_idx) = self.detail_idx {
            self.render_detail(detail_idx, &mut cmds);
        } else {
            self.render_list(&mut cmds);
        }

        cmds
    }

    fn render_list(&self, cmds: &mut Vec<RenderCmd>) {
        let t = &self.theme;
        let aw = self.area.w;
        let ah = self.area.h;
        if aw < 10 || ah < 3 {
            return;
        }
        let inner_w = aw.saturating_sub(4) as usize;

        santui_ipc::ui::draw_panel(cmds, t, 0, 0, aw, ah, "Plugins");

        // Status at top-right, same row as title
        let status_x = aw.saturating_sub(self.status.len() as u16 + 1);
        cmds.push(RenderCmd::Text {
            x: status_x,
            y: 0,
            text: self.status.clone(),
            fg: Some(t.text_muted),
            bg: Some(t.background_panel),
            bold: false,
        });

        // Table column layout: Status | Name | Description | Version | Action
        let status_w: usize = 7;
        let ver_w: usize = 10;
        let act_w: usize = 7;
        let rem = inner_w.saturating_sub(status_w + ver_w + act_w + 4);
        let name_w = (rem * 3 / 10).max(5);
        let desc_w = rem.saturating_sub(name_w);
        let sep = " ";

        // Header row
        let hdr = format!(
            "{:<sw$}{sep}{:<nw$}{sep}{:<dw$}{sep}{:>vw$}{sep}{:<aw$}",
            "Status",
            "Name",
            "Description",
            "Version",
            "Action",
            sw = status_w,
            nw = name_w,
            dw = desc_w,
            vw = ver_w,
            aw = act_w,
            sep = sep
        );
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 2,
            text: hdr,
            fg: Some(t.text_muted),
            bg: Some(t.background_panel),
            bold: true,
        });

        // Separator line
        let sep_line = format!("{:_<iw$}", "", iw = inner_w.saturating_sub(1));
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 3,
            text: sep_line,
            fg: Some(t.text_muted),
            bg: Some(t.background_panel),
            bold: false,
        });

        // Progress bar (if downloading)
        let has_progress = self.download_progress.is_some();
        if let Some((downloaded, total)) = self.download_progress {
            let bar_w = inner_w.saturating_sub(6).max(10);
            let pct = if total > 0 {
                (downloaded as f64 / total as f64).min(1.0)
            } else {
                0.0
            };
            let filled = (pct * bar_w as f64).round() as usize;
            let empty = bar_w.saturating_sub(filled);
            let pct_display = (pct * 100.0) as u32;
            let bar = format!(
                "[{}{}] {:3}%",
                "=".repeat(filled),
                " ".repeat(empty),
                pct_display
            );
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 4,
                text: bar,
                fg: Some(t.accent),
                bg: Some(t.background_panel),
                bold: false,
            });
        }

        // Plugin rows
        if let Some(ref reg) = self.registry {
            if reg.available.is_empty() {
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: 5,
                    text: "No plugins available".into(),
                    fg: self.fg(t.text_muted),
                    bg: self.bg(),
                    bold: false,
                });
                return;
            }

            let list_top = if has_progress { 6u16 } else { 5u16 };
            let list_h = max_list_h(ah) as usize;
            for i in 0..list_h.min(reg.available.len()) {
                let idx = self.scroll as usize + i;
                if idx >= reg.available.len() {
                    break;
                }
                let plugin = &reg.available[idx];
                let is_installed = reg.installed.iter().any(|p| {
                    p.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim_end_matches(".exe"))
                        == Some(&plugin.id)
                });
                let is_enabled = reg.installed.iter().any(|p| {
                    p.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim_end_matches(".exe"))
                        == Some(&plugin.id)
                        && p.enabled
                });
                let hovered = idx == self.cursor;

                let status = if is_enabled {
                    "ON"
                } else if is_installed {
                    "OFF"
                } else {
                    "--"
                };

                let name_s = if plugin.name.len() > name_w {
                    format!("{}…", &plugin.name[..name_w.saturating_sub(1)])
                } else {
                    format!("{:<nw$}", plugin.name, nw = name_w)
                };
                let desc_s = if plugin.description.len() > desc_w {
                    format!("{}…", &plugin.description[..desc_w.saturating_sub(1)])
                } else {
                    format!("{:<dw$}", plugin.description, dw = desc_w)
                };
                let ver_s = if plugin.version.len() > ver_w {
                    format!("{}…", &plugin.version[..ver_w.saturating_sub(1)])
                } else {
                    format!("{:>vw$}", plugin.version, vw = ver_w)
                };

                let action = if !is_installed {
                    "Install"
                } else if reg.installed.iter().any(|p| {
                    p.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim_end_matches(".exe"))
                        == Some(&plugin.id)
                        && p.version != plugin.version
                }) {
                    "Update"
                } else {
                    ""
                };
                let act_s = if action.len() > act_w {
                    format!("{}…", &action[..act_w.saturating_sub(1)])
                } else {
                    format!("{:<aw$}", action, aw = act_w)
                };

                let row_y = list_top + i as u16;
                let row_fg = if hovered { t.inverted_text } else { t.text };
                let line = format!(
                    "{:<sw$}{sep}{:<nw$}{sep}{:<dw$}{sep}{:>vw$}{sep}{:<aw$}",
                    status,
                    name_s,
                    desc_s,
                    ver_s,
                    act_s,
                    sw = status_w,
                    nw = name_w,
                    dw = desc_w,
                    vw = ver_w,
                    aw = act_w,
                    sep = sep
                );
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: row_y,
                    text: line,
                    fg: self.fg(row_fg),
                    bg: if hovered {
                        Some(t.highlight)
                    } else {
                        self.bg()
                    },
                    bold: hovered,
                });
            }
        }
    }

    fn render_detail(&self, detail_idx: usize, cmds: &mut Vec<RenderCmd>) {
        let t = &self.theme;
        let aw = self.area.w;
        let ah = self.area.h;
        if aw < 10 || ah < 3 {
            return;
        }

        santui_ipc::ui::draw_panel(cmds, t, 0, 0, aw, ah, "Plugin Details");

        if let Some(ref reg) = self.registry {
            if let Some(plugin) = reg.available.get(detail_idx) {
                let is_installed = reg.installed.iter().any(|p| {
                    p.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim_end_matches(".exe"))
                        == Some(&plugin.id)
                });
                let is_enabled = reg.installed.iter().any(|p| {
                    p.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim_end_matches(".exe"))
                        == Some(&plugin.id)
                        && p.enabled
                });

                let status_str = if is_enabled {
                    "Enabled"
                } else if is_installed {
                    "Disabled"
                } else {
                    "Not installed"
                };

                let y_base = 2u16;
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base,
                    text: format!(" {}", &plugin.name),
                    fg: Some(t.text),
                    bg: Some(t.background_panel),
                    bold: true,
                });

                let desc_w = aw.saturating_sub(4) as usize;
                let desc = if plugin.description.len() > desc_w {
                    format!("{}…", &plugin.description[..desc_w.saturating_sub(1)])
                } else {
                    plugin.description.clone()
                };
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 2,
                    text: desc,
                    fg: Some(t.text),
                    bg: Some(t.background_panel),
                    bold: false,
                });

                let field_fg = Some(t.text_muted);
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 4,
                    text: format!(" ID:      {}", &plugin.id),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 5,
                    text: format!(" Version: {}", &plugin.version),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 6,
                    text: format!(" Size:    {} bytes", &plugin.size),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 7,
                    text: format!(" Status:  {}", status_str),
                    fg: if is_enabled {
                        Some(t.success)
                    } else {
                        field_fg
                    },
                    bg: Some(t.background_panel),
                    bold: false,
                });

                let hint_y = y_base + 9;
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: hint_y,
                    text: " [Enter] Install / Toggle".into(),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: hint_y + 1,
                    text: " [Esc]   Back to list".into(),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
            }
        }
    }
}

fn max_list_h(content_h: u16) -> u16 {
    content_h.saturating_sub(8).max(3)
}
