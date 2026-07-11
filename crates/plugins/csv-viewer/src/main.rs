use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    row_scroll: usize,
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
            row_scroll: 0,
            status: "No CSV loaded. Send CSV content via IPC to view.".into(),
        }
    }
}

impl App {
    fn load(&mut self, content: &str) {
        self.headers.clear();
        self.rows.clear();
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(content.as_bytes());
        if let Ok(hdrs) = rdr.headers() {
            self.headers = hdrs.iter().map(|s| s.to_string()).collect();
        } else {
            self.status = "No headers found".into();
            return;
        }
        for record in rdr.records().flatten() {
            self.rows
                .push(record.iter().map(|s| s.to_string()).collect());
            if self.rows.len() >= 1000 {
                break;
            }
        }
        self.row_scroll = 0;
        self.status = format!(
            "Loaded {} rows, {} columns",
            self.rows.len(),
            self.headers.len()
        );
    }

    fn visible_rows(&self) -> usize {
        let h = self.area.h.max(14);
        h.saturating_sub(1 + 1 + 2) as usize // border + header + hints
    }

    fn rows_info(&self) -> String {
        if self.rows.is_empty() {
            "0 rows".into()
        } else {
            let total = self.rows.len();
            let vis = self.visible_rows();
            let end = (self.row_scroll + vis).min(total);
            format!("{}-{} of {}", self.row_scroll + 1, end, total)
        }
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        let total = self.rows.len();
        let vis = self.visible_rows();
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.row_scroll = self.row_scroll.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                if total > vis {
                    self.row_scroll = (self.row_scroll + 1).min(total - vis);
                }
                true
            }
            IpcKey::PageUp => {
                self.row_scroll = self.row_scroll.saturating_sub(vis.max(1));
                true
            }
            IpcKey::PageDown => {
                if total > vis {
                    self.row_scroll = (self.row_scroll + vis.max(1)).min(total - vis);
                }
                true
            }
            IpcKey::Home => {
                self.row_scroll = 0;
                true
            }
            IpcKey::End => {
                if total > vis {
                    self.row_scroll = total - vis;
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
        title: Some(" CSV Viewer ".into()),
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
    } else {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: format!("  {}", app.rows_info()),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });

        if app.rows.is_empty() {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 3,
                text: "No data rows".into(),
                fg: Some(t.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        } else {
            let vis = app.visible_rows();

            let n_cols = app.headers.len().max(1);
            let avail_w = (w.saturating_sub(4)) as usize;
            let col_w = (avail_w / n_cols).clamp(5, 20);
            let table_w = col_w * n_cols;
            let table_x = 2 + ((w.saturating_sub(4) as usize).saturating_sub(table_w) / 2) as u16;

            // Header
            let header_str: String = app
                .headers
                .iter()
                .map(|h| {
                    let s = if h.len() > col_w.saturating_sub(2) {
                        format!("{}{}", &h[..col_w.saturating_sub(3)], "\u{2026}")
                    } else {
                        format!("{:width$}", h, width = col_w.saturating_sub(1))
                    };
                    s
                })
                .collect::<Vec<_>>()
                .join(" ");
            cmds.push(RenderCmd::Text {
                x: table_x,
                y: 2,
                text: header_str,
                fg: Some(t.text),
                bg: Some(t.background_panel),
                bold: true,
                modifiers: 0,
            });

            let separator: String = (0..table_w).map(|_| "\u{2500}").collect();
            cmds.push(RenderCmd::Text {
                x: table_x,
                y: 3,
                text: separator,
                fg: Some(t.border),
                bg: None,
                bold: false,
                modifiers: 0,
            });

            let bottom_space: u16 = 2;
            let max_table_h = h.saturating_sub(4 + bottom_space) as usize;
            let visible_rows = max_table_h
                .min(vis)
                .min(app.rows.len().saturating_sub(app.row_scroll));

            for i in 0..visible_rows {
                let row_idx = app.row_scroll + i;
                if let Some(row) = app.rows.get(row_idx) {
                    let row_str: String = row
                        .iter()
                        .map(|cell| {
                            if cell.len() > col_w.saturating_sub(2) {
                                format!("{}{}", &cell[..col_w.saturating_sub(3)], "\u{2026}")
                            } else {
                                format!("{:width$}", cell, width = col_w.saturating_sub(1))
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    cmds.push(RenderCmd::Text {
                        x: table_x,
                        y: 4 + i as u16,
                        text: row_str,
                        fg: Some(t.text),
                        bg: None,
                        bold: false,
                        modifiers: 0,
                    });
                }
            }
        }
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
        ("↑↓/jk".into(), "scroll".into()),
        ("pgup/pgdn".into(), "page".into()),
        ("home/end".into(), "jump".into()),
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
    vec![("Utilities".into(), "Open CSV viewer".into())]
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
            Ok(HostMsg::PluginMessage { data, .. }) => {
                app.load(&data);
                app.dirty = true;
                true
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
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[csv-viewer] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
