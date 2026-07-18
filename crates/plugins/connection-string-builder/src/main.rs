use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};

const DB_TYPES: &[&str] = &["PostgreSQL", "MySQL", "SQLite", "MongoDB"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    DbType,
    Host,
    Port,
    Database,
    User,
    Password,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    db_type: usize,
    host: String,
    port: String,
    database: String,
    user: String,
    password: String,
    focus: Focus,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            db_type: 0,
            host: String::from("localhost"),
            port: String::from("5432"),
            database: String::from("mydb"),
            user: String::from("user"),
            password: String::from("pass"),
            focus: Focus::DbType,
            status: String::from(
                "Tab: cycle fields · \u{2190}\u{2192}: change DB type · Esc: close",
            ),
        }
    }
}

impl App {
    fn connection_string(&self) -> String {
        match self.db_type {
            0 => format!(
                "postgresql://{}:{}@{}:{}/{}",
                self.user, self.password, self.host, self.port, self.database
            ),
            1 => format!(
                "mysql://{}:{}@{}:{}/{}",
                self.user, self.password, self.host, self.port, self.database
            ),
            2 => {
                if self.database.is_empty() {
                    String::from("sqlite://:memory:")
                } else {
                    format!("sqlite:///{}", self.database)
                }
            }
            3 => format!(
                "mongodb://{}:{}@{}:{}/{}",
                self.user, self.password, self.host, self.port, self.database
            ),
            _ => String::new(),
        }
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::DbType => Focus::Host,
                    Focus::Host => Focus::Port,
                    Focus::Port => Focus::Database,
                    Focus::Database => Focus::User,
                    Focus::User => Focus::Password,
                    Focus::Password => Focus::DbType,
                };
                true
            }
            IpcKey::Left => {
                if self.focus == Focus::DbType {
                    self.db_type = self.db_type.saturating_sub(1);
                    self.update_default_port();
                }
                true
            }
            IpcKey::Right => {
                if self.focus == Focus::DbType {
                    self.db_type = (self.db_type + 1).min(DB_TYPES.len() - 1);
                    self.update_default_port();
                }
                true
            }
            IpcKey::Backspace => {
                match self.focus {
                    Focus::Host => {
                        self.host.pop();
                    }
                    Focus::Port => {
                        self.port.pop();
                    }
                    Focus::Database => {
                        self.database.pop();
                    }
                    Focus::User => {
                        self.user.pop();
                    }
                    Focus::Password => {
                        self.password.pop();
                    }
                    _ => {}
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                match self.focus {
                    Focus::Host => {
                        self.host.push(c);
                    }
                    Focus::Port => {
                        if c.is_ascii_digit() {
                            self.port.push(c);
                        }
                    }
                    Focus::Database => {
                        self.database.push(c);
                    }
                    Focus::User => {
                        self.user.push(c);
                    }
                    Focus::Password => {
                        self.password.push(c);
                    }
                    _ => {}
                }
                true
            }
            _ => true,
        }
    }

    fn update_default_port(&mut self) {
        self.port = match self.db_type {
            0 => String::from("5432"),
            1 => String::from("3306"),
            2 => String::new(),
            3 => String::from("27017"),
            _ => String::new(),
        };
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(50);
        let h = self.area.h.max(14);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": String::from(" Connection String Builder "),
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let db_str = DB_TYPES
            .iter()
            .enumerate()
            .map(|(i, d)| {
                if i == self.db_type {
                    format!("[{}]", d)
                } else {
                    format!(" {} ", d)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": format!("DB Type: {}", db_str),
            "fg": if self.focus == Focus::DbType { t.accent } else { t.text },
            "bg": null, "bold": self.focus == Focus::DbType, "modifiers": 0,
        }}));

        let fields = [
            ("Host", &self.host, Focus::Host),
            ("Port", &self.port, Focus::Port),
            ("Database", &self.database, Focus::Database),
            ("User", &self.user, Focus::User),
            ("Password", &self.password, Focus::Password),
        ];
        for (i, (label, value, focus)) in fields.iter().enumerate() {
            let active = *focus == self.focus;
            cmds.push(json!({"Text": {
                "x": 2, "y": 3 + i as u16, "text": format!(
                    "{} {}: {}",
                    if active { ">" } else { " " },
                    label,
                    value
                ),
                "fg": if active { t.accent } else { t.text },
                "bg": null, "bold": active, "modifiers": 0,
            }}));
        }

        let conn_str = self.connection_string();
        cmds.push(json!({"Border": {
            "x": 2, "y": 9, "w": w.saturating_sub(4), "h": 3,
            "fg": t.border, "borders": BORDER_ALL, "bg": t.background,
            "title": String::from(" Connection String "),
            "title_fg": t.text_muted, "title_dash_fg": t.border, "border_type": null,
        }}));
        cmds.push(json!({"Text": {
            "x": 4, "y": 10, "text": conn_str,
            "fg": t.success, "bg": null, "bold": true, "modifiers": 0,
        }}));

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

fn palette_commands() -> Vec<(String, String)> {
    vec![]
}

fn key_hints() -> Vec<(String, String)> {
    vec![("Tab".into(), "cycle fields".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app
            .render()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect(),
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
                log::error!("[connection-string-builder] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
