use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

const GRID_W: usize = 16;
const GRID_H: usize = 16;

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    grid: [[bool; GRID_W]; GRID_H],
    cursor_x: usize,
    cursor_y: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            grid: [[false; GRID_W]; GRID_H],
            cursor_x: 0,
            cursor_y: 0,
            status: String::from(
                "Arrow keys move \u{b7} space toggle \u{b7} c clear \u{b7} esc close",
            ),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Up => {
                self.cursor_y = self.cursor_y.wrapping_sub(1) % GRID_H;
                true
            }
            IpcKey::Down => {
                self.cursor_y = (self.cursor_y + 1) % GRID_H;
                true
            }
            IpcKey::Left => {
                self.cursor_x = self.cursor_x.wrapping_sub(1) % GRID_W;
                true
            }
            IpcKey::Right => {
                self.cursor_x = (self.cursor_x + 1) % GRID_W;
                true
            }
            IpcKey::Char(' ') => {
                self.grid[self.cursor_y][self.cursor_x] = !self.grid[self.cursor_y][self.cursor_x];
                true
            }
            IpcKey::Char('c') => {
                self.grid = [[false; GRID_W]; GRID_H];
                true
            }
            IpcKey::Esc => false,
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
            "bg": t.background_panel, "title": " Pixel Art ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let grid_offset_x = 2u16;
        let grid_offset_y = 1u16;

        for y in 0..GRID_H {
            for x in 0..GRID_W {
                let on = self.grid[y][x];
                let cursor = x == self.cursor_x && y == self.cursor_y;
                let ch = if on {
                    if cursor {
                        "\u{2593}"
                    } else {
                        "\u{2588}"
                    }
                } else {
                    if cursor {
                        "\u{2592}"
                    } else {
                        "\u{2591}"
                    }
                };
                cmds.push(json!({"Text": {
                    "x": grid_offset_x + x as u16 * 2,
                    "y": grid_offset_y + y as u16,
                    "text": ch,
                    "fg": if on { t.accent } else { t.text_muted },
                    "bg": None::<[u8; 3]>, "bold": false, "modifiers": 0,
                }}));
            }
        }

        let info_y = grid_offset_y + GRID_H as u16 + 1;
        cmds.push(json!({"Text": {
            "x": 2, "y": info_y,
            "text": format!("Cursor: ({}, {})  Pixels on: {}", self.cursor_x, self.cursor_y,
                self.grid.iter().flatten().filter(|&&v| v).count()),
            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
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
    vec![("Plugins".to_string(), "Pixel Art".to_string())]
}

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("esc".to_string(), "close".to_string()),
        ("arrows".to_string(), "move cursor".to_string()),
        ("space".to_string(), "toggle pixel".to_string()),
        ("c".to_string(), "clear canvas".to_string()),
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
                log::error!("[pixel-art] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
