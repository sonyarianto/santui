use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    entries: Vec<String>,
    selected: usize,
    scroll: usize,
    status: String,
    edit_buffer: String,
    editing: bool,
}

impl Default for App {
    fn default() -> Self {
        let path = std::env::var("PATH").unwrap_or_default();
        let entries: Vec<String> = std::env::split_paths(&path)
            .map(|p| p.to_string_lossy().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            status: format!(
                "{} entries \u{b7} d delete \u{b7} e edit \u{b7} a add \u{b7} esc",
                entries.len()
            ),
            entries,
            selected: 0,
            scroll: 0,
            edit_buffer: String::new(),
            editing: false,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.editing {
            match key {
                IpcKey::Esc => {
                    self.editing = false;
                    self.status = format!(
                        "{} entries \u{b7} d delete \u{b7} e edit \u{b7} a add \u{b7} esc",
                        self.entries.len()
                    );
                }
                IpcKey::Char('\n') | IpcKey::Char('\r') => {
                    let trimmed = self.edit_buffer.trim().to_string();
                    if !trimmed.is_empty() {
                        if self.selected < self.entries.len() {
                            self.entries[self.selected] = trimmed;
                        } else {
                            self.entries.push(trimmed);
                        }
                    }
                    self.editing = false;
                    self.status = format!("{} entries", self.entries.len());
                }
                IpcKey::Char(c) if !modifiers.ctrl => {
                    if c == '\u{7f}' || c == '\x08' {
                        self.edit_buffer.pop();
                    } else {
                        self.edit_buffer.push(c);
                    }
                }
                IpcKey::Backspace => {
                    self.edit_buffer.pop();
                }
                _ => {}
            }
            return true;
        }
        match key {
            IpcKey::Esc => false,
            IpcKey::Up => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            IpcKey::Down => {
                self.selected = (self.selected + 1).min(self.entries.len().saturating_sub(1));
                true
            }
            IpcKey::Home => {
                self.selected = 0;
                self.scroll = 0;
                true
            }
            IpcKey::End => {
                self.selected = self.entries.len().saturating_sub(1);
                true
            }
            IpcKey::PageUp => {
                let page = self.area.h.saturating_sub(6) as usize;
                self.selected = self.selected.saturating_sub(page);
                true
            }
            IpcKey::PageDown => {
                let page = self.area.h.saturating_sub(6) as usize;
                self.selected = (self.selected + page).min(self.entries.len().saturating_sub(1));
                true
            }
            IpcKey::Char('d') if !modifiers.ctrl => {
                if !self.entries.is_empty() && self.selected < self.entries.len() {
                    self.entries.remove(self.selected);
                    if self.selected >= self.entries.len() && !self.entries.is_empty() {
                        self.selected = self.entries.len() - 1;
                    }
                    self.status = format!("Deleted \u{b7} {} entries", self.entries.len());
                }
                true
            }
            IpcKey::Char('e') if !modifiers.ctrl => {
                if self.selected < self.entries.len() {
                    self.edit_buffer = self.entries[self.selected].clone();
                    self.editing = true;
                    self.status = "Edit path entry, Enter to confirm:".into();
                }
                true
            }
            IpcKey::Char('a') if !modifiers.ctrl => {
                self.edit_buffer.clear();
                self.editing = true;
                self.selected = self.entries.len();
                self.status = "Add new path entry, Enter to confirm:".into();
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
        let w = self.area.w.max(52);
        let h = self.area.h.max(12);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": " PATH Editor ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        if self.editing {
            cmds.push(json!({"Text": {
                "x": 2, "y": 1, "text": self.status.clone(),
                "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
            }}));
            cmds.push(json!({"Text": {
                "x": 2, "y": 2,
                "text": if self.edit_buffer.is_empty() { String::from("/new/path") } else { self.edit_buffer.clone() },
                "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
            }}));
        } else {
            cmds.push(json!({"Text": {
                "x": 2, "y": 1,
                "text": format!("{} entries in PATH", self.entries.len()),
                "fg": t.text_muted, "bg": null, "bold": true, "modifiers": 0,
            }}));

            let content_y = 3u16;
            let max_rows = h.saturating_sub(5) as usize;

            if self.entries.is_empty() {
                cmds.push(json!({"Text": {
                    "x": 2, "y": content_y,
                    "text": String::from("PATH is empty"),
                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
                }}));
            } else {
                let visible_end = (self.scroll + max_rows).min(self.entries.len());
                for i in self.scroll..visible_end {
                    let is_selected = i == self.selected;
                    cmds.push(json!({"Text": {
                        "x": 2, "y": content_y + (i - self.scroll) as u16,
                        "text": self.entries[i].clone(),
                        "fg": t.text, "bg": if is_selected { json!(t.highlight) } else { json!(null) },
                        "bold": is_selected, "modifiers": 0,
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
            "text": String::from("\u{2191}\u{2193} select \u{b7} d delete \u{b7} e edit \u{b7} a add \u{b7} esc"),
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
    json!([
        {"key": "esc", "hint": "close"},
        {"key": "d", "hint": "delete entry"},
        {"key": "e", "hint": "edit entry"},
        {"key": "a", "hint": "add entry"},
    ])
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = json!({
        "commands": commands_val, "hints": [], "palette_commands": palette_commands(),
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
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[path-editor] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
