use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct FileEntry {
    name: String,
    is_dir: bool,
    size: u64,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    current_path: PathBuf,
    entries: Vec<FileEntry>,
    selected: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let current_path = PathBuf::from(".");
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            current_path,
            entries: Vec::new(),
            selected: 0,
            status: String::new(),
        };
        app.read_dir();
        app
    }
}

impl App {
    fn read_dir(&mut self) {
        self.entries.clear();
        match std::fs::read_dir(&self.current_path) {
            Ok(entries) => {
                let mut file_entries: Vec<FileEntry> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                        FileEntry { name, is_dir, size }
                    })
                    .collect();
                file_entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
                self.entries = file_entries;
                self.status = String::from("Path: ") + &self.current_path.to_string_lossy();
            }
            Err(e) => {
                self.status = format!("Error: {e}");
            }
        }
        self.selected = 0;
    }

    fn enter_dir(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            if entry.is_dir {
                self.current_path.push(&entry.name);
                self.read_dir();
                self.dirty = true;
            }
        }
    }

    fn go_up(&mut self) -> bool {
        if !self
            .current_path
            .parent()
            .map(|p| p.as_os_str().is_empty())
            .unwrap_or(true)
            && self.current_path.parent().is_some()
        {
            self.current_path.pop();
            self.read_dir();
            self.dirty = true;
            true
        } else {
            false
        }
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                true
            }
            IpcKey::Down => {
                if self.selected + 1 < self.entries.len() {
                    self.selected += 1;
                }
                true
            }
            IpcKey::Enter => {
                self.enter_dir();
                true
            }
            IpcKey::Esc => self.go_up(),
            IpcKey::Char('h') => {
                self.go_up();
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
            "bg": t.background_panel, "title": " File Explorer ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let list_y = 1u16;
        let list_h = h.saturating_sub(4) as usize;
        let max_name_w = w.saturating_sub(22) as usize;

        let visible_items: Vec<String> = self
            .entries
            .iter()
            .take(list_h)
            .map(|e| {
                let name = if e.name.len() > max_name_w {
                    let truncated: String =
                        e.name.chars().take(max_name_w.saturating_sub(3)).collect();
                    format!("{truncated}...")
                } else {
                    e.name.clone()
                };
                let icon = if e.is_dir { "\u{1f4c1}" } else { "\u{1f4c4}" };
                let size_str = if e.is_dir {
                    String::from("  <DIR>")
                } else if e.size < 1024 {
                    format!("{:>4}B", e.size)
                } else if e.size < 1024 * 1024 {
                    format!("{:>5.1}K", e.size as f64 / 1024.0)
                } else {
                    format!("{:>5.1}M", e.size as f64 / (1024.0 * 1024.0))
                };
                format!("{icon} {name} {size_str}")
            })
            .collect();

        let vis_sel = if self.selected < list_h {
            Some(self.selected)
        } else {
            None
        };

        cmds.push(json!({"List": {
            "x": 1, "y": list_y, "w": w.saturating_sub(2), "h": list_h as u16,
            "items": visible_items,
            "selected": vis_sel,
            "style": {"fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0},
            "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": String::from("\u{2191}\u{2193} navigate \u{b7} enter open \u{b7} h up \u{b7} esc back"),
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
    json!([["Plugins", "File Explorer"]])
}

fn key_hints() -> Value {
    json!([
        ["esc", "close"],
        ["\u{2191}\u{2193}", "navigate"],
        ["enter", "open dir"],
        ["h", "go up"],
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
                log::error!("[file-explorer] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
