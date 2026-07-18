use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct MongoDoc {
    id: String,
    fields: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct MongoCollection {
    name: String,
    doc_count: usize,
    docs: Vec<MongoDoc>,
}

#[derive(Debug, Clone)]
enum View {
    Collections,
    Documents(usize),
    DocumentDetail(usize, usize),
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    collections: Vec<MongoCollection>,
    selected: usize,
    view: View,
    status: String,
}

fn mock_collections() -> Vec<MongoCollection> {
    vec![
        MongoCollection {
            name: String::from("users"),
            doc_count: 3,
            docs: vec![
                MongoDoc {
                    id: String::from("661f...a1"),
                    fields: vec![
                        (String::from("name"), String::from("Alice")),
                        (String::from("email"), String::from("alice@test.com")),
                        (String::from("age"), String::from("28")),
                        (String::from("role"), String::from("admin")),
                    ],
                },
                MongoDoc {
                    id: String::from("661f...b2"),
                    fields: vec![
                        (String::from("name"), String::from("Bob")),
                        (String::from("email"), String::from("bob@test.com")),
                        (String::from("age"), String::from("35")),
                        (String::from("role"), String::from("editor")),
                    ],
                },
                MongoDoc {
                    id: String::from("661f...c3"),
                    fields: vec![
                        (String::from("name"), String::from("Charlie")),
                        (String::from("email"), String::from("charlie@test.com")),
                        (String::from("age"), String::from("22")),
                        (String::from("role"), String::from("viewer")),
                    ],
                },
            ],
        },
        MongoCollection {
            name: String::from("products"),
            doc_count: 2,
            docs: vec![
                MongoDoc {
                    id: String::from("662a...d4"),
                    fields: vec![
                        (String::from("name"), String::from("Widget")),
                        (String::from("price"), String::from("9.99")),
                        (String::from("stock"), String::from("150")),
                    ],
                },
                MongoDoc {
                    id: String::from("662a...e5"),
                    fields: vec![
                        (String::from("name"), String::from("Gadget")),
                        (String::from("price"), String::from("24.99")),
                        (String::from("stock"), String::from("75")),
                    ],
                },
            ],
        },
        MongoCollection {
            name: String::from("orders"),
            doc_count: 2,
            docs: vec![
                MongoDoc {
                    id: String::from("662b...f6"),
                    fields: vec![
                        (String::from("userId"), String::from("661f...a1")),
                        (String::from("product"), String::from("Widget")),
                        (String::from("quantity"), String::from("2")),
                        (String::from("total"), String::from("19.98")),
                    ],
                },
                MongoDoc {
                    id: String::from("662b...g7"),
                    fields: vec![
                        (String::from("userId"), String::from("661f...b2")),
                        (String::from("product"), String::from("Gadget")),
                        (String::from("quantity"), String::from("1")),
                        (String::from("total"), String::from("24.99")),
                    ],
                },
            ],
        },
    ]
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            collections: mock_collections(),
            selected: 0,
            view: View::Collections,
            status: String::from("Select a collection to browse documents"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match (&self.view.clone(), key) {
            (View::Collections, IpcKey::Up) => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                true
            }
            (View::Collections, IpcKey::Down) => {
                if self.selected + 1 < self.collections.len() {
                    self.selected += 1;
                }
                true
            }
            (View::Collections, IpcKey::Enter) => {
                if self.selected < self.collections.len() {
                    self.view = View::Documents(self.selected);
                    self.selected = 0;
                    self.status = format!("Documents in: {}", self.collections[self.selected].name);
                }
                true
            }
            (View::Collections, IpcKey::Esc) => false,
            (View::Documents(_idx), IpcKey::Up) => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                true
            }
            (View::Documents(idx), IpcKey::Down) => {
                if let Some(col) = self.collections.get(*idx) {
                    if self.selected + 1 < col.docs.len() {
                        self.selected += 1;
                    }
                }
                true
            }
            (View::Documents(idx), IpcKey::Enter) => {
                if let Some(col) = self.collections.get(*idx) {
                    if self.selected < col.docs.len() {
                        self.view = View::DocumentDetail(*idx, self.selected);
                        self.status = String::from("Document details");
                    }
                }
                true
            }
            (View::Documents(_), IpcKey::Esc) | (View::Documents(_), IpcKey::Char('h')) => {
                self.view = View::Collections;
                self.selected = 0;
                self.status = String::from("Select a collection to browse documents");
                true
            }
            (View::DocumentDetail(idx, _), IpcKey::Esc)
            | (View::DocumentDetail(idx, _), IpcKey::Char('h')) => {
                self.view = View::Documents(*idx);
                self.selected = 0;
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
            "bg": t.background_panel, "title": " MongoDB Explorer ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let list_y = 1u16;
        let list_h = h.saturating_sub(4) as usize;

        match &self.view {
            View::Collections => {
                let items: Vec<String> = self
                    .collections
                    .iter()
                    .map(|c| format!("\u{1f4cb} {}  ({} docs)", c.name, c.doc_count))
                    .collect();
                let vis_items: Vec<String> = items.iter().take(list_h).cloned().collect();
                let vis_sel = if self.selected < list_h {
                    Some(self.selected)
                } else {
                    None
                };
                cmds.push(json!({"List": {
                    "x": 1, "y": list_y, "w": w.saturating_sub(2), "h": list_h as u16,
                    "items": vis_items, "selected": vis_sel,
                    "style": {"fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0},
                    "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
                }}));
            }
            View::Documents(idx) => {
                if let Some(col) = self.collections.get(*idx) {
                    let items: Vec<String> = col
                        .docs
                        .iter()
                        .map(|d| format!("\u{1f4dc} {}  ({} fields)", d.id, d.fields.len()))
                        .collect();
                    let vis_items: Vec<String> = items.iter().take(list_h).cloned().collect();
                    let vis_sel = if self.selected < list_h {
                        Some(self.selected)
                    } else {
                        None
                    };
                    cmds.push(json!({"List": {
                        "x": 1, "y": list_y, "w": w.saturating_sub(2), "h": list_h as u16,
                        "items": vis_items, "selected": vis_sel,
                        "style": {"fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0},
                        "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
                    }}));
                }
            }
            View::DocumentDetail(idx, doc_idx) => {
                if let Some(col) = self.collections.get(*idx) {
                    if let Some(doc) = col.docs.get(*doc_idx) {
                        cmds.push(json!({"Text": {
                            "x": 2, "y": list_y,
                            "text": format!("_id: {}", doc.id),
                            "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
                        }}));
                        for (i, (field, value)) in doc.fields.iter().enumerate() {
                            let y = list_y + 1 + i as u16;
                            if y + 1 >= h {
                                break;
                            }
                            cmds.push(json!({"Text": {
                                "x": 2, "y": y,
                                "text": format!("  {field}: {value}"),
                                "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
                            }}));
                        }
                    }
                }
            }
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

fn palette_commands() -> Vec<(String, String)> {
    vec![]
}

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("esc".into(), "close".into()),
        ("\u{2191}\u{2193}".into(), "navigate".into()),
        ("enter".into(), "select".into()),
    ]
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
                log::error!("[mongo-explorer] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
