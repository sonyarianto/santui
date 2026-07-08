use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct ColumnInfo {
    name: String,
}

#[derive(Debug, Clone)]
struct QueryResult {
    columns: Vec<ColumnInfo>,
    rows: Vec<Vec<String>>,
    message: String,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    query: String,
    query_cursor: usize,
    result: Option<QueryResult>,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            query: String::new(),
            query_cursor: 0,
            result: None,
            status: String::from("Type a SQL query and press Enter to execute"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Enter => {
                if !self.query.is_empty() {
                    self.execute_query();
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                let pos = self.query_cursor;
                if pos <= self.query.len() {
                    self.query.insert(pos, c);
                    self.query_cursor += 1;
                }
                true
            }
            IpcKey::Backspace => {
                if self.query_cursor > 0 {
                    let pos = self.query_cursor - 1;
                    if pos < self.query.len() {
                        self.query.remove(pos);
                    }
                    self.query_cursor -= 1;
                }
                true
            }
            IpcKey::Delete => {
                if self.query_cursor < self.query.len() {
                    self.query.remove(self.query_cursor);
                }
                true
            }
            IpcKey::Left => {
                self.query_cursor = self.query_cursor.saturating_sub(1);
                true
            }
            IpcKey::Right => {
                if self.query_cursor < self.query.len() {
                    self.query_cursor += 1;
                }
                true
            }
            IpcKey::Home => {
                self.query_cursor = 0;
                true
            }
            IpcKey::End => {
                self.query_cursor = self.query.len();
                true
            }
            IpcKey::Char('c') | IpcKey::Char('C') => {
                self.query.clear();
                self.query_cursor = 0;
                self.result = None;
                self.status = String::from("Query cleared");
                true
            }
            IpcKey::Esc => false,
            _ => true,
        }
    }

    fn execute_query(&mut self) {
        let q = self.query.trim().to_uppercase();
        if q.starts_with("SELECT") || q.starts_with("select") {
            self.result = Some(parse_select(&self.query));
            self.status = String::from("Query executed — mock results shown");
        } else {
            self.result = Some(QueryResult {
                columns: Vec::new(),
                rows: Vec::new(),
                message: String::from("Query executed successfully"),
            });
            self.status = String::from("Non-SELECT query executed successfully");
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

fn parse_select(query: &str) -> QueryResult {
    let upper = query.to_uppercase();
    let after_select = if let Some(pos) = upper.find("SELECT ") {
        &upper[pos + 7..]
    } else {
        return QueryResult {
            columns: vec![ColumnInfo {
                name: String::from("result"),
            }],
            rows: vec![vec![String::from("mock data")]],
            message: String::new(),
        };
    };

    let columns_part = if let Some(pos) = after_select.find("FROM") {
        &after_select[..pos]
    } else if let Some(pos) = after_select.find("FROM") {
        &after_select[..pos]
    } else {
        after_select
    };

    let col_names: Vec<String> = columns_part
        .split(',')
        .map(|s| {
            let trimmed = s.trim().trim_end_matches(',');
            let clean = trimmed
                .split_whitespace()
                .next()
                .unwrap_or(trimmed)
                .trim_matches('"')
                .trim_matches('\'')
                .trim_matches('`')
                .to_string();
            if clean.is_empty() {
                String::from("col")
            } else {
                clean
            }
        })
        .filter(|s| !s.is_empty())
        .collect();

    let columns: Vec<ColumnInfo> = if col_names.is_empty() {
        vec![ColumnInfo {
            name: String::from("result"),
        }]
    } else {
        col_names
            .into_iter()
            .map(|name| ColumnInfo { name })
            .collect()
    };

    let mut rows = Vec::new();
    for i in 0..3.min(columns.len().max(1)) {
        let row: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(j, col)| {
                format!(
                    "{}_{}_{}",
                    col.name,
                    i + 1,
                    match j % 3 {
                        0 => "value",
                        1 => "data",
                        _ => "result",
                    }
                )
            })
            .collect();
        rows.push(row);
    }

    QueryResult {
        columns,
        rows,
        message: String::new(),
    }
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(42);
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
        title: Some(String::from(" SQL Playground ")),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!("SQL> {}", app.query),
        fg: Some(t.highlight),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    if let Some(ref result) = app.result {
        if !result.columns.is_empty() {
            let header: String = result
                .columns
                .iter()
                .map(|c| format!("{:<20}", c.name))
                .collect::<Vec<_>>()
                .join(" ");
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 3,
                text: header,
                fg: Some(t.text),
                bg: None,
                bold: true,
                modifiers: 0,
            });
            let sep: String = (0..w.saturating_sub(4) as usize)
                .map(|_| '\u{2500}')
                .collect();
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 4,
                text: sep,
                fg: Some(t.border),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            let max_vis = (h.saturating_sub(7)) as usize;
            for (i, row) in result.rows.iter().enumerate().take(max_vis) {
                let row_str: String = row
                    .iter()
                    .map(|c| format!("{:<20}", c))
                    .collect::<Vec<_>>()
                    .join(" ");
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: 5 + i as u16,
                    text: row_str,
                    fg: Some(t.text),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
            }
        }
        if !result.message.is_empty() {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 5,
                text: result.message.clone(),
                fg: Some(t.success),
                bg: None,
                bold: false,
                modifiers: 0,
            });
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
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text: String::from("enter execute \u{b7} c clear \u{b7} esc"),
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

fn palette_commands() -> serde_json::Value {
    serde_json::json!([("SQL".to_string(), "Open SQL Playground".to_string())])
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
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[sql-playground] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
