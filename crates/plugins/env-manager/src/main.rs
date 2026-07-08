use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    env_vars: Vec<(String, String)>,
    filtered: Vec<usize>,
    cursor: usize,
    filter: String,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let mut env_vars: Vec<(String, String)> = std::env::vars().collect();
        env_vars.sort_by(|a, b| a.0.cmp(&b.0));
        let count = env_vars.len();
        let filtered: Vec<usize> = (0..count).collect();
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            env_vars,
            filtered,
            cursor: 0,
            filter: String::new(),
            status: format!("{} env vars", count),
        }
    }
}

impl App {
    fn apply_filter(&mut self) {
        let query = self.filter.to_lowercase();
        if query.is_empty() {
            self.filtered = (0..self.env_vars.len()).collect();
        } else {
            self.filtered = self
                .env_vars
                .iter()
                .enumerate()
                .filter(|(_, (k, v))| {
                    k.to_lowercase().contains(&query) || v.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }
        self.cursor = self.cursor.min(self.filtered.len().saturating_sub(1));
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.filtered.len().saturating_sub(1);
                self.cursor = self.cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Char('/') => {
                self.filter.clear();
                self.status = "Filter: ".into();
                true
            }
            IpcKey::Backspace => {
                if !self.filter.is_empty() {
                    self.filter.pop();
                    self.apply_filter();
                    self.status =
                        format!("Filter: {}  ({} matches)", self.filter, self.filtered.len());
                }
                true
            }
            IpcKey::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.apply_filter();
                self.status = format!("{} env vars", self.env_vars.len());
                true
            }
            IpcKey::Esc => false,
            IpcKey::Char(ch) if !ch.is_control() && !self.status.starts_with("Filter:") => {
                self.filter.push(ch);
                self.apply_filter();
                self.status = format!("Filter: {}  ({} matches)", self.filter, self.filtered.len());
                true
            }
            _ => {
                if self.status.starts_with("Filter:") {
                    match key {
                        IpcKey::Enter => {
                            self.status = format!(
                                "Filter: {}  ({} matches)",
                                self.filter,
                                self.filtered.len()
                            );
                            true
                        }
                        IpcKey::Char(ch) if !ch.is_control() => {
                            self.filter.push(ch);
                            self.apply_filter();
                            self.status = format!(
                                "Filter: {}  ({} matches)",
                                self.filter,
                                self.filtered.len()
                            );
                            true
                        }
                        _ => true,
                    }
                } else {
                    false
                }
            }
        }
    }

    fn handle_tick(&mut self) {}

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
        title: Some(" Environment Variables ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let header = format!("{} variables (filter: {})", app.env_vars.len(), app.filter);
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: header,
        fg: Some(t.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let rows: Vec<Vec<String>> = app
        .filtered
        .iter()
        .map(|&i| {
            let (k, v) = &app.env_vars[i];
            let display_val = if v.len() > 60 {
                format!("{}...", &v[..57])
            } else {
                v.clone()
            };
            vec![k.clone(), display_val]
        })
        .collect();

    let list_h = h.saturating_sub(5) as usize;
    let vis_count = rows.len().min(list_h);

    cmds.push(RenderCmd::Table {
        x: 2,
        y: 2,
        w: w.saturating_sub(4),
        h: vis_count as u16 + 1,
        header: vec!["Name".into(), "Value".into()],
        header_style: TextStyle {
            fg: Some(t.highlight),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![28, w.saturating_sub(36)],
        selected: Some(app.cursor.min(app.filtered.len().saturating_sub(1))),
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
        cell_styles: None,
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
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text: "\u{2191}\u{2193}/jk navigate \u{b7} / filter \u{b7} esc close".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
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
        "Developer".into(),
        "Open environment variable manager".into(),
    )]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": [],
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
            Ok(HostMsg::Tick) => {
                app.handle_tick();
                false
            }
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
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[env-manager] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
