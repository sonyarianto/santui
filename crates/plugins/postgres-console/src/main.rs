use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, TextStyle, ThemeData, BORDER_ALL,
};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct MockRow {
    cells: Vec<String>,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    input: String,
    history: Vec<String>,
    header: Vec<String>,
    rows: Vec<MockRow>,
    has_result: bool,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            input: String::new(),
            history: Vec::new(),
            header: Vec::new(),
            rows: Vec::new(),
            has_result: false,
            status: String::from("Type SQL query and press Enter to execute (mock)"),
        }
    }
}

impl App {
    fn execute_query(&mut self) {
        let query = self.input.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.history.push(query.clone());
        self.status = format!("Executed: {query}");

        let upper = query.to_uppercase();
        if upper.starts_with("SELECT") || upper.starts_with("select") {
            self.header = vec![
                String::from("id"),
                String::from("name"),
                String::from("value"),
                String::from("created_at"),
            ];
            self.rows = vec![
                MockRow {
                    cells: vec![
                        String::from("1"),
                        String::from("Alpha"),
                        String::from("100"),
                        String::from("2024-01-01"),
                    ],
                },
                MockRow {
                    cells: vec![
                        String::from("2"),
                        String::from("Beta"),
                        String::from("200"),
                        String::from("2024-01-02"),
                    ],
                },
                MockRow {
                    cells: vec![
                        String::from("3"),
                        String::from("Gamma"),
                        String::from("300"),
                        String::from("2024-01-03"),
                    ],
                },
                MockRow {
                    cells: vec![
                        String::from("4"),
                        String::from("Delta"),
                        String::from("400"),
                        String::from("2024-01-04"),
                    ],
                },
                MockRow {
                    cells: vec![
                        String::from("5"),
                        String::from("Epsilon"),
                        String::from("500"),
                        String::from("2024-01-05"),
                    ],
                },
            ];
            self.status = format!("Query returned {} rows", self.rows.len());
        } else if upper.starts_with("INSERT") {
            self.header = vec![String::from("command")];
            self.rows = vec![MockRow {
                cells: vec![String::from("INSERT 0 1")],
            }];
            self.status = String::from("INSERT executed successfully");
        } else if upper.starts_with("UPDATE") {
            self.header = vec![String::from("command")];
            self.rows = vec![MockRow {
                cells: vec![String::from("UPDATE 1")],
            }];
            self.status = String::from("UPDATE executed successfully");
        } else if upper.starts_with("DELETE") {
            self.header = vec![String::from("command")];
            self.rows = vec![MockRow {
                cells: vec![String::from("DELETE 1")],
            }];
            self.status = String::from("DELETE executed successfully");
        } else {
            self.header = vec![String::from("result")];
            self.rows = vec![MockRow {
                cells: vec![String::from("Query executed (mock)")],
            }];
        }
        self.has_result = true;
        self.input.clear();
        self.dirty = true;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Enter if !modifiers.ctrl => {
                self.execute_query();
                true
            }
            IpcKey::Backspace => {
                self.input.pop();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.input.push(c);
                true
            }
            _ => true,
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(40);
        let h = self.area.h.max(12);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": " PostgreSQL Console ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": 1,
            "text": String::from("SQL> "),
            "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
        }}));

        let display = if self.input.is_empty() {
            String::from("(type query)")
        } else {
            self.input.clone()
        };
        cmds.push(json!({"Text": {
            "x": 7, "y": 1,
            "text": display,
            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
        }}));

        let result_y = 3u16;
        if self.has_result && !self.header.is_empty() {
            let table_h = h.saturating_sub(result_y + 3) as usize;
            let col_w = (w.saturating_sub(4) / self.header.len().max(1) as u16).max(10);
            let col_widths: Vec<u16> = self.header.iter().map(|_| col_w).collect();

            let rows_str: Vec<Vec<String>> = self
                .rows
                .iter()
                .take(table_h)
                .map(|r| r.cells.clone())
                .collect();

            cmds.push(json!({"Table": {
                "x": 1, "y": result_y, "w": w.saturating_sub(2),
                "h": table_h.min(rows_str.len().max(1)) as u16,
                "header": self.header,
                "header_style": {"fg": t.text, "bg": null, "bold": true, "modifiers": 0},
                "rows": rows_str,
                "column_widths": col_widths,
                "selected": None::<usize>,
                "style": {"fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0},
                "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
                "current_row": None::<usize>,
                "current_style": None::<TextStyle>,
                "cell_styles": None::<Vec<Vec<Option<TextStyle>>>>,
            }}));
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
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

fn palette_commands() -> Value {
    json!([["Plugins", "Postgres Console"]])
}

fn key_hints() -> Value {
    json!([["esc", "close"], ["enter", "execute query"],])
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = json!({
        "commands": commands_val, "hints": key_hints(), "palette_commands": palette_commands(),
        "request": null, "plugin_message": null, "consumed": consumed,
    });
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json}");
    let _ = out.flush();
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = App::default();
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
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
                log::error!("[postgres-console] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
