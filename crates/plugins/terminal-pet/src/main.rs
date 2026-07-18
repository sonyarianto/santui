use rand::RngExt;
use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};

const CAT_FRAMES: &[&[&str]] = &[
    &["  /\\_/\\  ", " ( o.o ) ", "  > ^ <  "],
    &["  /\\_/\\  ", " ( -.- ) ", "  > ^ <  "],
    &["  /\\_/\\  ", " ( @.@ ) ", "  > ^ <  "],
];

const DOG_FRAMES: &[&[&str]] = &[
    &[
        "  / \\__",
        " (    @\\___",
        "  /         O",
        " /   (_____/",
        "/_____/   U",
    ],
    &[
        "  / \\__",
        " (    @\\___",
        "  /         O",
        " /   (_____/",
        "/_____/   U",
    ],
];

const BUNNY_FRAMES: &[&[&str]] = &[
    &["  (\\_/)", "  (x_x)", "  (\")(\")"],
    &["  (\\_/)", "  (\"_\"))", "  (\")(\")"],
];

struct Pet {
    art: &'static [&'static str],
    name: &'static str,
}

const PETS: &[Pet] = &[
    Pet {
        art: CAT_FRAMES[0],
        name: "Cat",
    },
    Pet {
        art: DOG_FRAMES[0],
        name: "Dog",
    },
    Pet {
        art: BUNNY_FRAMES[0],
        name: "Bunny",
    },
];

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    pet_idx: usize,
    mood: &'static str,
    tick_count: u32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            pet_idx: 0,
            mood: "happy",
            tick_count: 0,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Char(' ') => {
                self.pet_idx = (self.pet_idx + 1) % PETS.len();
                self.mood = "happy";
                true
            }
            IpcKey::Char('p') if !modifiers.ctrl => {
                self.mood = if self.mood == "happy" {
                    "sleepy"
                } else {
                    "happy"
                };
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
        let w = self.area.w.max(36);
        let h = self.area.h.max(14);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        let pet = &PETS[self.pet_idx];
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": format!(" Terminal Pet - {} ", pet.name),
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let frame = pet.art;
        let start_y = 2u16;
        let start_x = 4u16;

        for (i, line) in frame.iter().enumerate() {
            cmds.push(json!({"Text": {
                "x": start_x, "y": start_y + i as u16,
                "text": line.to_string(),
                "fg": t.accent, "bg": null, "bold": false, "modifiers": 0,
            }}));
        }

        let info_y = start_y + frame.len() as u16 + 1;

        cmds.push(json!({"Text": {
            "x": 2, "y": info_y,
            "text": format!("Mood: {}", self.mood),
            "fg": t.text, "bg": null, "bold": true, "modifiers": 0,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": info_y + 1,
            "text": format!("Pets: {}/{}", self.pet_idx + 1, PETS.len()),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        if self.mood == "sleepy" {
            cmds.push(json!({"Text": {
                "x": 2, "y": info_y + 3,
                "text": String::from("Zzz..."),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
            }}));
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": format!("Count: {}", self.tick_count),
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
    vec![("Plugins".into(), "Terminal Pet".into())]
}

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("esc".into(), "close".into()),
        ("space".into(), "switch pet".into()),
        ("p".into(), "toggle mood".into()),
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
            Ok(HostMsg::Tick) => {
                app.tick_count += 1;
                if app.tick_count % 10 == 0 {
                    app.dirty = true;
                    let mut rng = rand::rng();
                    if rng.random_bool(0.1) {
                        app.pet_idx = rng.random_range(0..PETS.len());
                    }
                }
                app.dirty = true;
                false
            }
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[terminal-pet] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
