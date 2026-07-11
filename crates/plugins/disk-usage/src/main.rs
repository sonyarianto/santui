use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct DirEntry {
    name: String,
    total_gb: u64,
    used_gb: u64,
}

fn mock_dirs() -> Vec<DirEntry> {
    vec![
        DirEntry {
            name: "/home".into(),
            total_gb: 500,
            used_gb: 320,
        },
        DirEntry {
            name: "/var".into(),
            total_gb: 100,
            used_gb: 72,
        },
        DirEntry {
            name: "/etc".into(),
            total_gb: 50,
            used_gb: 12,
        },
        DirEntry {
            name: "/usr".into(),
            total_gb: 200,
            used_gb: 145,
        },
        DirEntry {
            name: "/opt".into(),
            total_gb: 150,
            used_gb: 48,
        },
        DirEntry {
            name: "/tmp".into(),
            total_gb: 50,
            used_gb: 6,
        },
        DirEntry {
            name: "/boot".into(),
            total_gb: 2,
            used_gb: 1,
        },
    ]
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    dirs: Vec<DirEntry>,
    cursor: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            dirs: mock_dirs(),
            cursor: 0,
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
                let max = self.dirs.len().saturating_sub(1);
                self.cursor = self.cursor.min(max).saturating_add(1).min(max);
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
    let w = app.area.w.max(48);
    let h = app.area.h.max(12);
    let gauge_w = w.saturating_sub(34).max(10);

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
        title: Some(" Disk Usage Analyzer ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let style = TextStyle {
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    };
    let hl_style = TextStyle {
        fg: Some(t.inverted_text),
        bg: Some(t.highlight),
        bold: true,
        modifiers: 0,
    };
    let gauge_style = TextStyle {
        fg: None,
        bg: Some(t.accent),
        bold: false,
        modifiers: 0,
    };

    let rows: Vec<Vec<String>> = app
        .dirs
        .iter()
        .map(|d| {
            let pct = d
                .used_gb
                .checked_mul(100)
                .and_then(|v| v.checked_div(d.total_gb.max(1)))
                .unwrap_or(0);
            vec![
                d.name.clone(),
                format!("{}/{} GB", d.used_gb, d.total_gb),
                format!("{}%", pct),
            ]
        })
        .collect();

    cmds.push(RenderCmd::Table {
        x: 2,
        y: 1,
        w: gauge_w.saturating_add(30),
        h: app.dirs.len() as u16 + 2,
        header: vec!["Directory".into(), "Used".into(), "%".into()],
        header_style: TextStyle {
            fg: Some(t.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![14, 16, 6],
        selected: Some(app.cursor),
        style,
        highlight_style: hl_style,
        current_row: None,
        current_style: None,
        cell_styles: None,
    });

    for (i, dir) in app.dirs.iter().enumerate() {
        let y = 2 + i as u16;
        let ratio = if dir.total_gb > 0 {
            dir.used_gb as f64 / dir.total_gb as f64
        } else {
            0.0
        };
        let pct = (ratio * 100.0) as u64;
        let label = format!("[{}]", "=".repeat((pct / 10) as usize));
        cmds.push(RenderCmd::Gauge {
            x: 2 + 2 + 14 + 16 + 2,
            y,
            w: gauge_w,
            h: 1,
            ratio,
            label: Some(label),
            style,
            gauge_style,
        });
    }

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: format!(
            "{} directories, {} selected",
            app.dirs.len(),
            app.cursor + 1
        ),
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
    vec![(
        "System Monitoring".into(),
        "Open disk usage analyzer".into(),
    )]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": hints(),
        "palette_commands": palette_commands(),
        "request": null,
        "plugin_message": null,
        "consumed": consumed,
    });
    if let Ok(json_str) = serde_json::to_string(&json) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{json_str}");
        let _ = out.flush();
    }
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
                log::error!("[disk-usage] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
