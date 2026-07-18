use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{Area, HostMsg, RenderCmd, ThemeData, BORDER_ALL};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            headers: Vec::new(),
            rows: Vec::new(),
            status: "Press 'l' to load sample table, or send TSV/CSV via plugin message.".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: santui_ipc::protocol::IpcKey) -> bool {
        match key {
            santui_ipc::protocol::IpcKey::Char('l') => {
                self.load_sample();
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn load_sample(&mut self) {
        self.load(
            "Name\tAge\tCity\tOccupation\n\
             Alice\t30\tNew York\tEngineer\n\
             Bob\t25\tLondon\tDesigner\n\
             Charlie\t35\tTokyo\tDoctor\n\
             Diana\t28\tParis\tTeacher\n\
             Eve\t32\tSydney\tArchitect",
        );
    }

    fn load(&mut self, content: &str) {
        let delimiter = if content.contains('\t') { '\t' } else { ',' };
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            self.status = "Empty input".into();
            return;
        }
        self.headers = lines[0]
            .split(delimiter)
            .map(|s| s.trim().to_string())
            .collect();
        self.rows = lines[1..]
            .iter()
            .map(|l| l.split(delimiter).map(|s| s.trim().to_string()).collect())
            .collect();
        self.status = format!(
            "Table: {} cols, {} rows",
            self.headers.len(),
            self.rows.len()
        );
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
    let w = app.area.w.max(40);
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
        title: Some(" ASCII Table ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    if app.headers.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 2,
            text: app.status.clone(),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        return cmds;
    }

    let n = app.headers.len().max(1);
    let avail = (w.saturating_sub(4)) as usize;
    let col_w = (avail / n).clamp(6, 30);
    let total_w = col_w * n;
    let x0 = 2 + ((w.saturating_sub(4) as usize).saturating_sub(total_w) / 2) as u16;

    let hbar: String = "\u{2500}".repeat(total_w + n + 1);
    let top: String = "\u{2550}".repeat(total_w + n + 1);

    cmds.push(RenderCmd::Text {
        x: x0,
        y: 2,
        text: top.clone(),
        fg: Some(t.border),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let hdr: String = app
        .headers
        .iter()
        .map(|hd| cell(hd, col_w))
        .collect::<Vec<_>>()
        .join("|");
    cmds.push(RenderCmd::Text {
        x: x0,
        y: 3,
        text: format!("|{hdr}|"),
        fg: Some(t.text),
        bg: Some(t.background_panel),
        bold: true,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: x0,
        y: 4,
        text: hbar.clone(),
        fg: Some(t.border),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let max_rows = h.saturating_sub(6) as usize;
    for (i, row) in app.rows.iter().take(max_rows).enumerate() {
        let r: String = row
            .iter()
            .map(|c| cell(c, col_w))
            .collect::<Vec<_>>()
            .join("|");
        cmds.push(RenderCmd::Text {
            x: x0,
            y: 5 + i as u16,
            text: format!("|{r}|"),
            fg: Some(t.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds.push(RenderCmd::Text {
        x: x0,
        y: 5 + max_rows as u16,
        text: hbar,
        fg: Some(t.border),
        bg: None,
        bold: false,
        modifiers: 0,
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

fn cell(s: &str, w: usize) -> String {
    if s.chars().count() > w.saturating_sub(2) {
        let trunc: String = s.chars().take(w.saturating_sub(3)).collect();
        format!(" {trunc}\u{2026} ")
    } else {
        format!(" {s:<width$} ", width = w.saturating_sub(2))
    }
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
    vec![("Utilities".into(), "Open ASCII table".into())]
}

fn key_hints() -> Vec<(String, String)> {
    vec![("l".into(), "load sample table".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: key_hints(),
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
            Ok(HostMsg::PluginMessage { data, .. }) => {
                app.load(&data);
                app.dirty = true;
                true
            }
            Ok(HostMsg::Key { key, .. }) => app.handle_key(key),
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
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[ascii-table] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
