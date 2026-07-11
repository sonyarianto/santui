use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct MockRow {
    cells: Vec<String>,
}

#[derive(Clone)]
struct MockTable {
    name: String,
    columns: Vec<String>,
    rows: Vec<MockRow>,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    input: String,
    history: Vec<String>,
    tables: Vec<MockTable>,
    selected_table: usize,
    view: View,
    status: String,
}

enum View {
    TableList,
    QueryResult,
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
            tables: vec![
                MockTable {
                    name: String::from("users"),
                    columns: vec![
                        String::from("id"),
                        String::from("username"),
                        String::from("email"),
                        String::from("created_at"),
                    ],
                    rows: vec![
                        MockRow {
                            cells: vec![
                                String::from("1"),
                                String::from("alice"),
                                String::from("alice@example.com"),
                                String::from("2024-01-15 10:30:00"),
                            ],
                        },
                        MockRow {
                            cells: vec![
                                String::from("2"),
                                String::from("bob"),
                                String::from("bob@example.com"),
                                String::from("2024-02-20 14:00:00"),
                            ],
                        },
                        MockRow {
                            cells: vec![
                                String::from("3"),
                                String::from("carol"),
                                String::from("carol@example.com"),
                                String::from("2024-03-10 09:15:00"),
                            ],
                        },
                    ],
                },
                MockTable {
                    name: String::from("products"),
                    columns: vec![
                        String::from("id"),
                        String::from("name"),
                        String::from("price"),
                        String::from("stock"),
                    ],
                    rows: vec![
                        MockRow {
                            cells: vec![
                                String::from("1"),
                                String::from("Widget A"),
                                String::from("19.99"),
                                String::from("42"),
                            ],
                        },
                        MockRow {
                            cells: vec![
                                String::from("2"),
                                String::from("Gadget B"),
                                String::from("34.99"),
                                String::from("18"),
                            ],
                        },
                        MockRow {
                            cells: vec![
                                String::from("3"),
                                String::from("Doohickey C"),
                                String::from("9.99"),
                                String::from("150"),
                            ],
                        },
                    ],
                },
                MockTable {
                    name: String::from("orders"),
                    columns: vec![
                        String::from("id"),
                        String::from("user_id"),
                        String::from("total"),
                        String::from("status"),
                    ],
                    rows: vec![
                        MockRow {
                            cells: vec![
                                String::from("1"),
                                String::from("1"),
                                String::from("54.98"),
                                String::from("shipped"),
                            ],
                        },
                        MockRow {
                            cells: vec![
                                String::from("2"),
                                String::from("2"),
                                String::from("34.99"),
                                String::from("pending"),
                            ],
                        },
                    ],
                },
            ],
            selected_table: 0,
            view: View::TableList,
            status: String::from(
                "Connected to mock MySQL. Type SQL or press Tab to browse tables.",
            ),
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

        let upper = query.to_uppercase();
        if upper.starts_with("SHOW TABLES") {
            self.view = View::QueryResult;
            let header = vec![String::from("Tables_in_mock_db")];
            let rows: Vec<MockRow> = self
                .tables
                .iter()
                .map(|t| MockRow {
                    cells: vec![t.name.clone()],
                })
                .collect();
            self.status = format!("Showing {} tables", rows.len());
            let mut table = self.tables[0].clone();
            table.columns = header;
            table.rows = rows;
            self.tables[0] = table;
        } else if upper.starts_with("SELECT") {
            self.view = View::QueryResult;
            let table = &self.tables[self.selected_table];
            self.status = format!("SELECT returned {} rows", table.rows.len());
        } else if upper.starts_with("DESCRIBE") || upper.starts_with("SHOW COLUMNS") {
            self.view = View::QueryResult;
            let table = &self.tables[self.selected_table];
            let header = vec![
                String::from("Field"),
                String::from("Type"),
                String::from("Null"),
                String::from("Key"),
            ];
            let rows: Vec<MockRow> = table
                .columns
                .iter()
                .map(|c| MockRow {
                    cells: vec![
                        c.clone(),
                        String::from("varchar(255)"),
                        String::from("YES"),
                        String::from(""),
                    ],
                })
                .collect();
            self.status = format!("Described table: {}", table.name);
            let mut cloned = table.clone();
            cloned.columns = header;
            cloned.rows = rows;
            self.tables[0] = cloned;
            self.selected_table = 0;
        } else if upper.starts_with("INSERT") {
            self.view = View::QueryResult;
            let header = vec![String::from("result")];
            let rows = vec![MockRow {
                cells: vec![String::from("Query OK, 1 row affected")],
            }];
            let mut cloned = self.tables[0].clone();
            cloned.columns = header;
            cloned.rows = rows;
            self.tables[0] = cloned;
            self.selected_table = 0;
            self.status = String::from("INSERT executed successfully");
        } else if upper.starts_with("UPDATE") || upper.starts_with("DELETE") {
            self.view = View::QueryResult;
            let header = vec![String::from("result")];
            let rows = vec![MockRow {
                cells: vec![String::from("Query OK, 1 row affected")],
            }];
            let mut cloned = self.tables[0].clone();
            cloned.columns = header;
            cloned.rows = rows;
            self.tables[0] = cloned;
            self.selected_table = 0;
            self.status = String::from("Query executed successfully");
        } else if upper.starts_with("CREATE TABLE") {
            let parts: Vec<&str> = query.split_whitespace().collect();
            let table_name = parts.get(2).map(|s| s.to_string()).unwrap_or_default();
            self.tables.push(MockTable {
                name: table_name.clone(),
                columns: vec![String::from("id"), String::from("data")],
                rows: vec![MockRow {
                    cells: vec![String::from("(empty)"), String::from("(empty)")],
                }],
            });
            self.view = View::QueryResult;
            let header = vec![String::from("result")];
            let rows = vec![MockRow {
                cells: vec![format!("Table `{table_name}` created")],
            }];
            let mut cloned = self.tables[0].clone();
            cloned.columns = header;
            cloned.rows = rows;
            self.tables[0] = cloned;
            self.selected_table = 0;
            self.status = String::from("Table created");
        } else if upper.starts_with("DROP TABLE") {
            let parts: Vec<&str> = query.split_whitespace().collect();
            let table_name = parts.get(2).map(|s| s.to_string()).unwrap_or_default();
            self.tables.retain(|t| t.name != table_name);
            self.view = View::QueryResult;
            let header = vec![String::from("result")];
            let rows = vec![MockRow {
                cells: vec![format!("Table `{table_name}` dropped")],
            }];
            let mut cloned = self.tables[0].clone();
            cloned.columns = header;
            cloned.rows = rows;
            self.tables[0] = cloned;
            self.selected_table = 0;
            self.status = String::from("Table dropped");
        } else {
            self.view = View::QueryResult;
            let header = vec![String::from("result")];
            let rows = vec![MockRow {
                cells: vec![String::from("Query executed (mock mode)")],
            }];
            let mut cloned = self.tables[0].clone();
            cloned.columns = header;
            cloned.rows = rows;
            self.tables[0] = cloned;
            self.selected_table = 0;
            self.status = String::from("Query executed");
        }
        self.input.clear();
        self.dirty = true;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Tab => {
                match self.view {
                    View::TableList => {
                        self.view = View::QueryResult;
                        let header = vec![String::from("result")];
                        let rows = vec![MockRow {
                            cells: vec![String::from("Type a query or press Tab for tables")],
                        }];
                        let mut cloned = self.tables[0].clone();
                        cloned.columns = header;
                        cloned.rows = rows;
                        self.tables[0] = cloned;
                        self.selected_table = 0;
                        self.status = String::from("Query mode — type SQL and press Enter");
                    }
                    View::QueryResult => {
                        self.view = View::TableList;
                        self.status = String::from("Browse tables. Press Tab for query mode.");
                    }
                }
                true
            }
            IpcKey::Up | IpcKey::Char('k') if matches!(self.view, View::TableList) => {
                self.selected_table = self.selected_table.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') if matches!(self.view, View::TableList) => {
                let max = self.tables.len().saturating_sub(1);
                self.selected_table = self.selected_table.saturating_add(1).min(max);
                true
            }
            IpcKey::Enter if matches!(self.view, View::TableList) => {
                let table = &self.tables[self.selected_table];
                self.status = format!("Showing data from `{}`", table.name);
                true
            }
            IpcKey::Enter if matches!(self.view, View::QueryResult) && !modifiers.ctrl => {
                self.execute_query();
                true
            }
            IpcKey::Backspace if matches!(self.view, View::QueryResult) => {
                self.input.pop();
                true
            }
            IpcKey::Char(c) if !c.is_control() && matches!(self.view, View::QueryResult) => {
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
            "bg": t.background_panel, "title": " MySQL Browser ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        match self.view {
            View::TableList => {
                cmds.push(json!({"Text": {
                    "x": 2, "y": 1,
                    "text": String::from("Tables in mock_db"),
                    "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
                }}));
                for (i, table) in self.tables.iter().enumerate() {
                    let y = 2 + i as u16;
                    if y >= h.saturating_sub(2) {
                        break;
                    }
                    let highlight = i == self.selected_table;
                    cmds.push(json!({"Text": {
                        "x": 4, "y": y,
                        "text": table.name.clone(),
                        "fg": if highlight { t.inverted_text } else { t.text },
                        "bg": if highlight { Some(t.highlight) } else { Option::<[u8;3]>::None },
                        "bold": highlight, "modifiers": 0,
                    }}));
                    if highlight {
                        let info = format!(
                            "  {} columns x {} rows",
                            table.columns.len(),
                            table.rows.len()
                        );
                        cmds.push(json!({"Text": {
                            "x": (4 + table.name.len() as u16 + 1), "y": y,
                            "text": info,
                            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
                        }}));
                    }
                }
            }
            View::QueryResult => {
                let input_y = 1u16;
                cmds.push(json!({"Text": {
                    "x": 2, "y": input_y,
                    "text": String::from("mysql> "),
                    "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
                }}));
                let display = if self.input.is_empty() {
                    String::from("(type SQL query)")
                } else {
                    self.input.clone()
                };
                cmds.push(json!({"Text": {
                    "x": 9, "y": input_y,
                    "text": display,
                    "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
                }}));

                let result_y = 3u16;
                let table = &self.tables[self.selected_table];
                if !table.columns.is_empty() {
                    let table_h = h.saturating_sub(result_y + 3) as usize;
                    let col_w = (w.saturating_sub(4) / table.columns.len().max(1) as u16).max(10);
                    let col_widths: Vec<u16> = table.columns.iter().map(|_| col_w).collect();
                    let rows_str: Vec<Vec<String>> = table
                        .rows
                        .iter()
                        .take(table_h)
                        .map(|r| r.cells.clone())
                        .collect();

                    cmds.push(json!({"Table": {
                        "x": 1, "y": result_y, "w": w.saturating_sub(2),
                        "h": table_h.min(rows_str.len().max(1)) as u16,
                        "header": table.columns,
                        "header_style": {"fg": t.text, "bg": null, "bold": true, "modifiers": 0},
                        "rows": rows_str,
                        "column_widths": col_widths,
                        "selected": None::<usize>,
                        "style": {"fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0},
                        "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
                        "current_row": None::<usize>,
                        "current_style": None::<santui_ipc::protocol::TextStyle>,
                        "cell_styles": None::<Vec<Vec<Option<santui_ipc::protocol::TextStyle>>>>,
                    }}));
                }
            }
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": String::from("tab switch mode \u{b7} enter execute \u{b7} \u{2191}\u{2193} browse \u{b7} esc close"),
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
    json!([["Plugins", "Mysql Browser"]])
}

fn key_hints() -> Value {
    json!([
        ["tab", "switch mode"],
        ["esc", "close"],
        ["enter", "execute query"],
    ])
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
                log::error!("[mysql-browser] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
