use std::io::{BufRead, BufReader};

use rusqlite::Connection;
use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct TableInfo {
    name: String,
    columns: Vec<String>,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    file_path: String,
    tables: Vec<TableInfo>,
    selected_table: usize,
    rows: Vec<Vec<String>>,
    status: String,
    mode: Mode,
    path_buffer: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    PathInput,
    Browse,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            file_path: String::new(),
            tables: Vec::new(),
            selected_table: 0,
            rows: Vec::new(),
            status: String::from("Enter .db file path and press Enter"),
            mode: Mode::PathInput,
            path_buffer: String::new(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match self.mode {
            Mode::PathInput => match key {
                IpcKey::Esc => false,
                IpcKey::Enter => {
                    let path = self.path_buffer.trim().to_string();
                    if !path.is_empty() {
                        self.file_path = path.clone();
                        self.open_database(&path);
                    }
                    true
                }
                IpcKey::Backspace => {
                    self.path_buffer.pop();
                    true
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.path_buffer.push(c);
                    true
                }
                _ => true,
            },
            Mode::Browse => match key {
                IpcKey::Esc => {
                    self.mode = Mode::PathInput;
                    self.dirty = true;
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    self.selected_table = self.selected_table.saturating_sub(1);
                    self.load_table_data();
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    let max = self.tables.len().saturating_sub(1);
                    self.selected_table = self.selected_table.saturating_add(1).min(max);
                    self.load_table_data();
                    true
                }
                IpcKey::Enter => {
                    self.load_table_data();
                    true
                }
                _ => true,
            },
        }
    }

    fn open_database(&mut self, path: &str) {
        match Connection::open(path) {
            Ok(conn) => {
                let sql = "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name";
                let mut stmt = match conn.prepare(sql) {
                    Ok(s) => s,
                    Err(e) => {
                        self.status = format!("Error: {e}");
                        return;
                    }
                };
                let names: Vec<String> = match stmt.query_map([], |row| row.get::<_, String>(0)) {
                    Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                    Err(e) => {
                        self.status = format!("Error: {e}");
                        return;
                    }
                };
                self.tables = names
                    .into_iter()
                    .map(|name| {
                        let cols = Self::get_columns(&conn, &name);
                        TableInfo {
                            name,
                            columns: cols,
                        }
                    })
                    .collect();
                self.selected_table = 0;
                self.mode = Mode::Browse;
                self.status = format!("Opened {path}");
                if !self.tables.is_empty() {
                    self.load_table_data();
                }
            }
            Err(e) => {
                self.status = format!("Error: {e}");
            }
        }
    }

    fn get_columns(conn: &Connection, table: &str) -> Vec<String> {
        let sql = format!("PRAGMA table_info({table})");
        match conn.prepare(&sql) {
            Ok(mut stmt) => match stmt.query_map([], |row| row.get::<_, String>(1)) {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    }

    fn load_table_data(&mut self) {
        if self.tables.is_empty() {
            self.rows.clear();
            return;
        }
        let table = &self.tables[self.selected_table].name;
        match Connection::open(&self.file_path) {
            Ok(conn) => {
                let sql = format!("SELECT * FROM \"{table}\" LIMIT 50");
                match conn.prepare(&sql) {
                    Ok(mut stmt) => {
                        let col_count = stmt.column_count();
                        match stmt.query_map([], |row| {
                            let mut vals = Vec::new();
                            for i in 0..col_count {
                                let val: String = match row.get::<_, String>(i) {
                                    Ok(v) => v,
                                    Err(_) => match row.get::<_, i64>(i) {
                                        Ok(n) => n.to_string(),
                                        Err(_) => match row.get::<_, f64>(i) {
                                            Ok(f) => format!("{f:.2}"),
                                            Err(_) => match row.get::<_, Option<String>>(i) {
                                                Ok(Some(s)) => s,
                                                _ => String::from("NULL"),
                                            },
                                        },
                                    },
                                };
                                vals.push(val);
                            }
                            Ok(vals)
                        }) {
                            Ok(rows) => {
                                self.rows = rows.filter_map(|r| r.ok()).collect();
                                self.status = format!("Table: {table}  Rows: {}", self.rows.len());
                            }
                            Err(e) => {
                                self.rows.clear();
                                self.status = format!("Error: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        self.rows.clear();
                        self.status = format!("Error: {e}");
                    }
                }
            }
            Err(e) => {
                self.rows.clear();
                self.status = format!("Error: {e}");
            }
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        self.cached_commands.clone()
    }
}

fn render_ui(app: &App) -> Vec<Value> {
    let t = &app.theme;
    let w = app.area.w.max(48);
    let h = app.area.h.max(14);
    let mut cmds: Vec<Value> = Vec::new();

    cmds.push(json!({
        String::from("type"): String::from("Rect"),
        String::from("x"): 0, String::from("y"): 0,
        String::from("w"): w, String::from("h"): h,
        String::from("bg"): t.background,
    }));
    cmds.push(json!({
        String::from("type"): String::from("Border"),
        String::from("x"): 0, String::from("y"): 0,
        String::from("w"): w, String::from("h"): h,
        String::from("fg"): t.border,
        String::from("borders"): BORDER_ALL,
        String::from("bg"): t.background_panel,
        String::from("title"): String::from(" SQLite Browser "),
        String::from("title_fg"): t.text,
        String::from("title_dash_fg"): t.border,
    }));

    match app.mode {
        Mode::PathInput => {
            cmds.push(json!({
                String::from("type"): String::from("Text"),
                String::from("x"): 2, String::from("y"): 2,
                String::from("text"): String::from("Enter .db file path:"),
                String::from("fg"): t.text,
                String::from("bold"): false,
                String::from("modifiers"): 0,
            }));
            cmds.push(json!({
                String::from("type"): String::from("Border"),
                String::from("x"): 2, String::from("y"): 4,
                String::from("w"): w.saturating_sub(4), String::from("h"): 3,
                String::from("fg"): t.accent,
                String::from("borders"): BORDER_ALL,
                String::from("bg"): t.background,
            }));
            cmds.push(json!({
                String::from("type"): String::from("Text"),
                String::from("x"): 4, String::from("y"): 5,
                String::from("text"): app.path_buffer.clone(),
                String::from("fg"): t.text,
                String::from("bold"): false,
                String::from("modifiers"): 0,
            }));
        }
        Mode::Browse => {
            let left_w = (w / 3).max(16);
            cmds.push(json!({
                String::from("type"): String::from("Border"),
                String::from("x"): 1, String::from("y"): 1,
                String::from("w"): left_w, String::from("h"): h.saturating_sub(4),
                String::from("fg"): t.border,
                String::from("borders"): BORDER_ALL,
                String::from("bg"): t.background,
                String::from("title"): String::from(" Tables "),
                String::from("title_fg"): t.text,
                String::from("title_dash_fg"): t.border,
            }));

            for (i, table) in app.tables.iter().enumerate() {
                let y = 2 + i as u16;
                if y >= h.saturating_sub(5) {
                    break;
                }
                let is_sel = i == app.selected_table;
                cmds.push(json!({
                    String::from("type"): String::from("Text"),
                    String::from("x"): 3, String::from("y"): y,
                    String::from("text"): if is_sel {
                        format!("▸ {}", table.name)
                    } else {
                        format!("  {}", table.name)
                    },
                    String::from("fg"): if is_sel { t.highlight } else { t.text },
                    String::from("bold"): is_sel,
                    String::from("modifiers"): 0,
                }));
            }

            let data_x = left_w + 2;
            let data_w = w.saturating_sub(data_x + 1);

            if let Some(table) = app.tables.get(app.selected_table) {
                cmds.push(json!({
                    String::from("type"): String::from("Border"),
                    String::from("x"): data_x, String::from("y"): 1,
                    String::from("w"): data_w, String::from("h"): h.saturating_sub(4),
                    String::from("fg"): t.border,
                    String::from("borders"): BORDER_ALL,
                    String::from("bg"): t.background,
                    String::from("title"): format!(" {} ", table.name),
                    String::from("title_fg"): t.text,
                    String::from("title_dash_fg"): t.border,
                }));

                for (ci, col) in table.columns.iter().enumerate() {
                    let x = data_x + 2 + ci as u16 * 20;
                    if x + 18 > w {
                        break;
                    }
                    cmds.push(json!({
                        String::from("type"): String::from("Text"),
                        String::from("x"): x, String::from("y"): 2,
                        String::from("text"): col.clone(),
                        String::from("fg"): t.accent,
                        String::from("bold"): true,
                        String::from("modifiers"): 0,
                    }));
                }

                for (ri, row) in app.rows.iter().enumerate() {
                    let y = 3 + ri as u16;
                    if y >= h.saturating_sub(5) {
                        break;
                    }
                    for (ci, val) in row.iter().enumerate() {
                        let x = data_x + 2 + ci as u16 * 20;
                        if x + 18 > w {
                            break;
                        }
                        cmds.push(json!({
                            String::from("type"): String::from("Text"),
                            String::from("x"): x, String::from("y"): y,
                            String::from("text"): val.clone(),
                            String::from("fg"): t.text,
                            String::from("bold"): false,
                            String::from("modifiers"): 0,
                        }));
                    }
                }
            }
        }
    }

    let status_y = h.saturating_sub(2);
    cmds.push(json!({
        String::from("type"): String::from("Text"),
        String::from("x"): 2, String::from("y"): status_y,
        String::from("text"): app.status.clone(),
        String::from("fg"): t.text_muted,
        String::from("bold"): false,
        String::from("modifiers"): 0,
    }));

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

fn hints() -> Vec<(String, String)> {
    vec![
        ("enter".into(), "confirm".into()),
        ("esc".into(), "back".into()),
    ]
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("Plugins".into(), "Open SQLite Browser".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app
            .render()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect(),
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
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: Result<HostMsg, _> = serde_json::from_str(&line);
                match msg {
                    Ok(HostMsg::Init { theme, area, .. }) => {
                        app.theme = theme;
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::Resize { area }) => {
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::ThemeChange { theme }) => {
                        app.theme = theme;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::Key { key, modifiers }) => {
                        let consumed = app.handle_key(key, modifiers);
                        respond(&mut app, consumed);
                    }
                    Ok(HostMsg::Tick) => {
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::PaletteCommand { .. }) => {
                        app.dirty = true;
                        respond(&mut app, true);
                    }
                    Ok(HostMsg::Shutdown) => break,
                    Ok(_) => {
                        respond(&mut app, false);
                    }
                    Err(e) => {
                        log::error!("[sqlite-browser] parse error: {e}: {line}");
                        respond(&mut app, false);
                    }
                }
            }
        }
    }
}
