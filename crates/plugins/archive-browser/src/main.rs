use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone)]
struct ArchiveEntry {
    name: String,
    size: u64,
    is_dir: bool,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    path_input: String,
    entries: Vec<ArchiveEntry>,
    loaded: bool,
    selected: usize,
    editing: bool,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            path_input: String::new(),
            entries: Vec::new(),
            loaded: false,
            selected: 0,
            editing: false,
            status: String::from(
                "Enter path to .zip file · Enter: load · o: open path · Esc: close",
            ),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.editing {
            match key {
                IpcKey::Esc => {
                    self.editing = false;
                }
                IpcKey::Enter => {
                    self.editing = false;
                    self.load_archive();
                }
                IpcKey::Backspace => {
                    self.path_input.pop();
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.path_input.push(c);
                }
                _ => {}
            }
            return true;
        }
        match key {
            IpcKey::Esc => false,
            IpcKey::Up | IpcKey::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.entries.len().saturating_sub(1);
                self.selected = (self.selected + 1).min(max);
                true
            }
            IpcKey::Enter => {
                self.load_archive();
                true
            }
            IpcKey::Char('o') => {
                self.editing = true;
                self.status = String::from("Enter archive path:");
                true
            }
            IpcKey::Char('r') => {
                self.editing = true;
                self.path_input.clear();
                self.loaded = false;
                self.entries.clear();
                self.selected = 0;
                self.status = String::from("Enter archive path:");
                true
            }
            _ => true,
        }
    }

    fn load_archive(&mut self) {
        let path = self.path_input.trim();
        if path.is_empty() {
            self.status = String::from("Please enter a file path");
            return;
        }
        if !path.ends_with(".zip") {
            self.status = String::from("Only .zip files are supported");
            return;
        }
        match std::fs::File::open(path) {
            Ok(file) => match zip::ZipArchive::new(file) {
                Ok(mut archive) => {
                    self.entries.clear();
                    for i in 0..archive.len() {
                        if let Ok(entry) = archive.by_index(i) {
                            self.entries.push(ArchiveEntry {
                                name: entry.name().to_string(),
                                size: entry.size(),
                                is_dir: entry.is_dir(),
                            });
                        }
                    }
                    if self.entries.is_empty() {
                        self.status = String::from("Archive is empty or could not be read");
                    } else {
                        self.status =
                            format!("Loaded {} entries from {}", self.entries.len(), path);
                    }
                    self.loaded = true;
                    self.selected = 0;
                }
                Err(e) => {
                    self.status = format!("Error reading zip: {e}");
                }
            },
            Err(e) => {
                self.status = format!("Cannot open file: {e}");
            }
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
            "bg": t.background_panel, "title": String::from(" Archive Browser "),
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": if self.editing {
                format!("> Path: {}", self.path_input)
            } else {
                format!("  Path: {}", self.path_input)
            },
            "fg": if self.editing { t.accent } else { t.text },
            "bg": null, "bold": self.editing, "modifiers": 0,
        }}));

        if self.loaded {
            let header = format!("{:<60} {:>12}  {}", "Name", "Size", "Type");
            cmds.push(json!({"Text": {
                "x": 2, "y": 3, "text": header,
                "fg": t.text_muted, "bg": null, "bold": true, "modifiers": 0,
            }}));

            let list_y = 4;
            let list_h = h.saturating_sub(7).max(1);
            let items: Vec<String> = self
                .entries
                .iter()
                .map(|e| {
                    let size_str = if e.is_dir {
                        String::from("<DIR>")
                    } else if e.size >= 1_000_000 {
                        format!("{:.1}MB", e.size as f64 / 1_000_000.0)
                    } else if e.size >= 1_000 {
                        format!("{:.1}KB", e.size as f64 / 1_000.0)
                    } else {
                        format!("{}B", e.size)
                    };
                    format!(
                        "{:<60} {:>12}  {}",
                        e.name,
                        size_str,
                        if e.is_dir { "dir" } else { "file" }
                    )
                })
                .collect();
            cmds.push(json!({"List": {
                "x": 2, "y": list_y, "w": w.saturating_sub(4), "h": list_h,
                "items": items,
                "selected": if self.selected < self.entries.len() { Some(self.selected) } else { None },
                "style": {"fg": t.text, "bg": null, "bold": false, "modifiers": 0},
                "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
            }}));
        } else {
            cmds.push(json!({"Text": {
                "x": 2, "y": 3,
                "text": String::from("Press o to enter a .zip file path, then Enter to load"),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
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
    json!([["Tools", "Archive Browser"]])
}

fn key_hints() -> Value {
    json!([
        ["o", "open path"],
        ["Enter", "load archive"],
        ["r", "reset"],
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
                log::error!("[archive-browser] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
