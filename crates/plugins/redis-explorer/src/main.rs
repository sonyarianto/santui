use std::io::{BufRead, BufReader, Write};

use rand::RngExt;
use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct RedisKey {
    name: String,
    value: String,
    kind: String,
}

fn generate_mock_keys(count: usize) -> Vec<RedisKey> {
    let prefixes = &[
        "user",
        "session",
        "cache",
        "config",
        "queue",
        "rate_limit",
        "flag",
    ];
    let mut rng = rand::rng();
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        let prefix = prefixes[rng.random_range(0..prefixes.len())];
        let suffix: u32 = rng.random_range(1..1000);
        let name = format!("{prefix}:{suffix}");
        let kind = match rng.random_range(0..3) {
            0 => String::from("string"),
            1 => String::from("list"),
            _ => String::from("set"),
        };
        let value = match kind.as_str() {
            "string" => {
                let len = rng.random_range(5..30);
                let chars: String = (0..len)
                    .map(|_| rng.random_range(b'a'..=b'z') as char)
                    .collect();
                format!("\"{chars}\"")
            }
            "list" => {
                let len = rng.random_range(1..6);
                let items: Vec<String> = (0..len).map(|i| format!("item_{i}")).collect();
                format!("[{}]", items.join(", "))
            }
            _ => {
                let len = rng.random_range(2..5);
                let items: Vec<String> = (0..len).map(|i| format!("val_{i}")).collect();
                format!("{{{}}}", items.join(", "))
            }
        };
        keys.push(RedisKey { name, value, kind });
    }
    keys.sort_by(|a, b| a.name.cmp(&b.name));
    keys
}

#[derive(Debug, Clone)]
enum View {
    List,
    Detail(usize),
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    keys: Vec<RedisKey>,
    selected: usize,
    view: View,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let keys = generate_mock_keys(30);
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            keys,
            selected: 0,
            view: View::List,
            status: String::from("Select a key to view its value"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match (&self.view.clone(), key) {
            (View::List, IpcKey::Up) => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                true
            }
            (View::List, IpcKey::Down) => {
                if self.selected + 1 < self.keys.len() {
                    self.selected += 1;
                }
                true
            }
            (View::List, IpcKey::Enter) => {
                if self.selected < self.keys.len() {
                    self.view = View::Detail(self.selected);
                    self.status = format!("Key: {}", self.keys[self.selected].name);
                }
                true
            }
            (View::List, IpcKey::Esc) => false,
            (View::Detail(_), IpcKey::Esc) | (View::Detail(_), IpcKey::Char('h')) => {
                self.view = View::List;
                self.status = String::from("Select a key to view its value");
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
            "bg": t.background_panel, "title": " Redis Explorer ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let list_y = 1u16;
        let list_h = h.saturating_sub(4) as usize;

        match &self.view {
            View::List => {
                let items: Vec<String> = self
                    .keys
                    .iter()
                    .take(list_h)
                    .map(|k| format!("\u{1f511} {}  [{}]", k.name, k.kind))
                    .collect();
                let vis_sel = if self.selected < list_h {
                    Some(self.selected)
                } else {
                    None
                };
                cmds.push(json!({"List": {
                    "x": 1, "y": list_y, "w": w.saturating_sub(2),
                    "h": items.len().min(list_h) as u16,
                    "items": items,
                    "selected": vis_sel,
                    "style": {"fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0},
                    "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
                }}));
            }
            View::Detail(idx) => {
                if let Some(key) = self.keys.get(*idx) {
                    cmds.push(json!({"Text": {
                        "x": 2, "y": list_y,
                        "text": format!("Key:   {}", key.name),
                        "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
                    }}));
                    cmds.push(json!({"Text": {
                        "x": 2, "y": list_y + 1,
                        "text": format!("Type:  {}", key.kind),
                        "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
                    }}));
                    cmds.push(json!({"Text": {
                        "x": 2, "y": list_y + 2,
                        "text": String::from("Value:"),
                        "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
                    }}));

                    let max_val_w = w.saturating_sub(4) as usize;
                    let value_display = if key.value.len() > max_val_w {
                        let truncated: String = key
                            .value
                            .chars()
                            .take(max_val_w.saturating_sub(3))
                            .collect();
                        format!("{truncated}...")
                    } else {
                        key.value.clone()
                    };

                    cmds.push(json!({"Border": {
                        "x": 2, "y": list_y + 3, "w": w.saturating_sub(4),
                        "h": 3, "fg": t.border, "borders": BORDER_ALL,
                        "bg": t.background, "title": None::<String>,
                        "title_fg": None::<[u8; 3]>, "title_dash_fg": None::<[u8; 3]>,
                        "border_type": None::<u8>,
                    }}));
                    cmds.push(json!({"Text": {
                        "x": 4, "y": list_y + 4,
                        "text": value_display,
                        "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
                    }}));
                }
            }
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        let hint = match self.view {
            View::List => {
                String::from("\u{2191}\u{2193} navigate \u{b7} enter view \u{b7} esc close")
            }
            View::Detail(_) => String::from("h/esc back to key list"),
        };
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": hint,
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
    json!([["Plugins", "Redis Explorer"]])
}

fn key_hints() -> Value {
    json!([
        ["esc", "close"],
        ["\u{2191}\u{2193}", "navigate"],
        ["enter", "view value"],
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
                log::error!("[redis-explorer] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
