use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContainerStatus {
    Running,
    Stopped,
}

#[derive(Debug, Clone)]
struct Container {
    name: String,
    image: String,
    status: ContainerStatus,
    ports: String,
}

impl Container {
    fn toggle(&mut self) {
        self.status = match self.status {
            ContainerStatus::Running => ContainerStatus::Stopped,
            ContainerStatus::Stopped => ContainerStatus::Running,
        };
    }
}

fn mock_containers() -> Vec<Container> {
    vec![
        Container {
            name: "nginx-web".into(),
            image: "nginx:1.25".into(),
            status: ContainerStatus::Running,
            ports: "0.0.0.0:80->80/tcp".into(),
        },
        Container {
            name: "redis-cache".into(),
            image: "redis:7-alpine".into(),
            status: ContainerStatus::Running,
            ports: "0.0.0.0:6379->6379/tcp".into(),
        },
        Container {
            name: "postgres-db".into(),
            image: "postgres:16".into(),
            status: ContainerStatus::Running,
            ports: "0.0.0.0:5432->5432/tcp".into(),
        },
        Container {
            name: "node-api".into(),
            image: "node:20-slim".into(),
            status: ContainerStatus::Stopped,
            ports: "3000/tcp".into(),
        },
        Container {
            name: "mongo-db".into(),
            image: "mongo:7".into(),
            status: ContainerStatus::Stopped,
            ports: "27017/tcp".into(),
        },
        Container {
            name: "grafana".into(),
            image: "grafana/grafana:10".into(),
            status: ContainerStatus::Running,
            ports: "0.0.0.0:3000->3000/tcp".into(),
        },
        Container {
            name: "prometheus".into(),
            image: "prom/prometheus:v2".into(),
            status: ContainerStatus::Running,
            ports: "0.0.0.0:9090->9090/tcp".into(),
        },
    ]
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    containers: Vec<Container>,
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
            containers: mock_containers(),
            cursor: 0,
            status: "Enter/Space toggle start/stop \u{b7} \u{2191}\u{2193}/jk navigate".into(),
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
                let max = self.containers.len().saturating_sub(1);
                self.cursor = self.cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Enter | IpcKey::Char(' ') => {
                if let Some(c) = self.containers.get_mut(self.cursor) {
                    let action = match c.status {
                        ContainerStatus::Running => "Stopped",
                        ContainerStatus::Stopped => "Started",
                    };
                    c.toggle();
                    self.status = format!("Container {} {} (mock)", c.name, action);
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
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
    let w = app.area.w.max(60);
    let h = app.area.h.max(14);

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
        title: Some(" Docker Dashboard ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let rows: Vec<Vec<String>> = app
        .containers
        .iter()
        .map(|c| {
            let (status_text, _status_fg) = match c.status {
                ContainerStatus::Running => ("Running".into(), t.success),
                ContainerStatus::Stopped => ("Stopped".into(), t.error),
            };
            vec![
                c.name.clone(),
                c.image.clone(),
                status_text,
                c.ports.clone(),
            ]
        })
        .collect();

    let cell_styles: Vec<Vec<Option<TextStyle>>> = app
        .containers
        .iter()
        .map(|c| {
            let status_style = match c.status {
                ContainerStatus::Running => TextStyle {
                    fg: Some(t.success),
                    bg: None,
                    bold: true,
                    modifiers: 0,
                },
                ContainerStatus::Stopped => TextStyle {
                    fg: Some(t.error),
                    bg: None,
                    bold: true,
                    modifiers: 0,
                },
            };
            vec![None, None, Some(status_style), None]
        })
        .collect();

    cmds.push(RenderCmd::Table {
        x: 2,
        y: 1,
        w: w.saturating_sub(4),
        h: app.containers.len() as u16 + 2,
        header: vec![
            "Name".into(),
            "Image".into(),
            "Status".into(),
            "Ports".into(),
        ],
        header_style: TextStyle {
            fg: Some(t.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![18, 20, 10, 28],
        selected: Some(app.cursor),
        style: TextStyle {
            fg: Some(t.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(t.inverted_text),
            bg: Some(t.highlight),
            bold: true,
            modifiers: 0,
        },
        current_row: None,
        current_style: None,
        cell_styles: Some(cell_styles),
    });

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
        ("↑↓/jk".into(), "navigate".into()),
        ("enter/space".into(), "toggle".into()),
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
    vec![("DevOps".into(), "Open Docker dashboard".into())]
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
            Ok(HostMsg::Tick) => false,
            Ok(HostMsg::PaletteCommand { .. }) => {
                app.dirty = true;
                true
            }
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[docker-dashboard] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
