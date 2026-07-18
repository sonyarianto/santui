use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct Service {
    name: String,
    description: String,
    status: String,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    services: Vec<Service>,
    cursor: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            services: vec![
                Service {
                    name: String::from("nginx"),
                    description: String::from("High-performance web server and reverse proxy"),
                    status: String::from("running"),
                },
                Service {
                    name: String::from("postgresql"),
                    description: String::from("Advanced relational database system"),
                    status: String::from("running"),
                },
                Service {
                    name: String::from("redis-server"),
                    description: String::from("In-memory data structure store, used as cache"),
                    status: String::from("running"),
                },
                Service {
                    name: String::from("docker"),
                    description: String::from("Container runtime and orchestration engine"),
                    status: String::from("stopped"),
                },
                Service {
                    name: String::from("sshd"),
                    description: String::from("OpenSSH secure shell daemon"),
                    status: String::from("running"),
                },
                Service {
                    name: String::from("cron"),
                    description: String::from("Job scheduler daemon"),
                    status: String::from("failed"),
                },
                Service {
                    name: String::from("ufw"),
                    description: String::from("Uncomplicated firewall manager"),
                    status: String::from("stopped"),
                },
                Service {
                    name: String::from("prometheus"),
                    description: String::from("Monitoring system and time series database"),
                    status: String::from("running"),
                },
            ],
            cursor: 0,
            status: String::from("8 services loaded"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.services.len().saturating_sub(1);
                self.cursor = self.cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Char('s') | IpcKey::Char('S') => {
                if let Some(svc) = self.services.get_mut(self.cursor) {
                    svc.status = String::from("running");
                    self.status = format!("Started {}", svc.name);
                }
                true
            }
            IpcKey::Char('t') | IpcKey::Char('T') => {
                if let Some(svc) = self.services.get_mut(self.cursor) {
                    svc.status = String::from("stopped");
                    self.status = format!("Stopped {}", svc.name);
                }
                true
            }
            IpcKey::Char('r') | IpcKey::Char('R') => {
                if let Some(svc) = self.services.get_mut(self.cursor) {
                    svc.status = String::from("running");
                    self.status = format!("Restarted {}", svc.name);
                }
                true
            }
            IpcKey::Esc => false,
            _ => true,
        }
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(42);
    let h = app.area.h.max(12);

    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: t.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: t.border,
        borders: BORDER_ALL,
        bg: Some(t.background_panel),
        title: Some(String::from(" Service Manager ")),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let list_h = (h.saturating_sub(6)) as usize;
    let start_idx = if app.cursor >= list_h {
        app.cursor - list_h + 1
    } else {
        0
    };
    for i in 0..list_h {
        let idx = start_idx + i;
        if idx >= app.services.len() {
            break;
        }
        let svc = &app.services[idx];
        let selected = idx == app.cursor;
        let status_text = format!("[{}]", svc.status);
        let line = format!(" {:<20} {}", svc.name, status_text);
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1 + i as u16,
            text: line,
            fg: Some(if selected { t.highlight } else { t.text }),
            bg: if selected {
                Some(t.background_overlay)
            } else {
                None
            },
            bold: selected,
            modifiers: 0,
        });
    }

    if let Some(svc) = app.services.get(app.cursor) {
        let detail_y = h.saturating_sub(4);
        cmds.push(RenderCmd::Text {
            x: 2,
            y: detail_y,
            text: format!("{}: {}", svc.name, svc.description),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: app.status.clone(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds
}

fn hints() -> Vec<(String, String)> {
    vec![
        ("up/down".into(), "navigate".into()),
        ("s".into(), "start".into()),
        ("t".into(), "stop".into()),
        ("r".into(), "restart".into()),
        ("esc".into(), "back".into()),
    ]
}

fn default_theme() -> ThemeData {
    ThemeData {
        text: [220; 3],
        text_muted: [140; 3],
        accent: [180; 3],
        highlight: [220; 3],
        logo: [255; 3],
        background: [0; 3],
        background_panel: [20; 3],
        background_overlay: [10; 3],
        border: [150; 3],
        success: [127, 216, 143],
        error: [224, 108, 117],
        inverted_text: [20; 3],
    }
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("Services".to_string(), "Open Service Manager".to_string())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: hints(),
        palette_commands: palette_commands(),
        request: None,
        plugin_message: None,
        consumed,
    };
    let mut out = std::io::stdout().lock();
    let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &msg);
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = App::default();
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        let trimmed = line.trim_end();
        let msg = serde_json::from_str::<HostMsg>(trimmed);
        let consumed = match msg {
            Ok(HostMsg::Init { theme, area, .. }) => {
                app.theme = theme;
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Resize { area }) => {
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::ThemeChange { theme }) => {
                app.theme = theme;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Key { key, modifiers }) => app.handle_key(key, modifiers),
            Ok(HostMsg::PaletteCommand { .. }) => {
                app.dirty = true;
                true
            }
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Tick
                | HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[service-manager] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
