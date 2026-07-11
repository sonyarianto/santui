use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone)]
struct RenameItem {
    original: String,
    renamed: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditMode {
    Prefix,
    Suffix,
    Find,
    Replace,
    None,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    prefix: String,
    suffix: String,
    find: String,
    replace: String,
    files: Vec<String>,
    preview: Vec<RenameItem>,
    edit_mode: EditMode,
    selected: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let files: Vec<String> = (1..=10).map(|i| format!("file{}.txt", i)).collect();
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            prefix: String::new(),
            suffix: String::new(),
            find: String::new(),
            replace: String::new(),
            files,
            preview: Vec::new(),
            edit_mode: EditMode::None,
            selected: 0,
            status: String::from("Tab: cycle fields · Enter: preview · p/s/f/r: edit · Esc: close"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.edit_mode != EditMode::None {
            match key {
                IpcKey::Esc => {
                    self.edit_mode = EditMode::None;
                }
                IpcKey::Enter => {
                    self.edit_mode = EditMode::None;
                    self.update_preview();
                }
                IpcKey::Backspace => match self.edit_mode {
                    EditMode::Prefix => {
                        self.prefix.pop();
                    }
                    EditMode::Suffix => {
                        self.suffix.pop();
                    }
                    EditMode::Find => {
                        self.find.pop();
                    }
                    EditMode::Replace => {
                        self.replace.pop();
                    }
                    EditMode::None => {}
                },
                IpcKey::Char(c) if !c.is_control() => match self.edit_mode {
                    EditMode::Prefix => {
                        self.prefix.push(c);
                    }
                    EditMode::Suffix => {
                        self.suffix.push(c);
                    }
                    EditMode::Find => {
                        self.find.push(c);
                    }
                    EditMode::Replace => {
                        self.replace.push(c);
                    }
                    EditMode::None => {}
                },
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
                let max = self.preview.len().saturating_sub(1);
                self.selected = (self.selected + 1).min(max);
                true
            }
            IpcKey::Enter => {
                self.update_preview();
                true
            }
            IpcKey::Tab => {
                self.edit_mode = match self.edit_mode {
                    EditMode::None => EditMode::Prefix,
                    EditMode::Prefix => EditMode::Suffix,
                    EditMode::Suffix => EditMode::Find,
                    EditMode::Find => EditMode::Replace,
                    EditMode::Replace => EditMode::None,
                };
                self.status = match self.edit_mode {
                    EditMode::None => String::from("Tab: cycle fields · Enter: preview"),
                    EditMode::Prefix => String::from("Editing prefix:"),
                    EditMode::Suffix => String::from("Editing suffix:"),
                    EditMode::Find => String::from("Editing find:"),
                    EditMode::Replace => String::from("Editing replace:"),
                };
                true
            }
            IpcKey::Char('p') => {
                self.edit_mode = EditMode::Prefix;
                self.status = String::from("Editing prefix:");
                true
            }
            IpcKey::Char('s') => {
                self.edit_mode = EditMode::Suffix;
                self.status = String::from("Editing suffix:");
                true
            }
            IpcKey::Char('f') => {
                self.edit_mode = EditMode::Find;
                self.status = String::from("Editing find:");
                true
            }
            IpcKey::Char('r') => {
                self.edit_mode = EditMode::Replace;
                self.status = String::from("Editing replace:");
                true
            }
            IpcKey::Char('a') => {
                self.files
                    .push(format!("new-file-{}.txt", self.files.len() + 1));
                self.update_preview();
                true
            }
            IpcKey::Char('d') => {
                if self.selected < self.preview.len() {
                    let orig = self.preview[self.selected].original.clone();
                    self.files.retain(|f| *f != orig);
                    self.update_preview();
                    self.selected = self.selected.min(self.preview.len().saturating_sub(1));
                }
                true
            }
            _ => true,
        }
    }

    fn update_preview(&mut self) {
        self.preview.clear();
        for file in &self.files {
            let mut name = file.clone();
            if !self.prefix.is_empty() {
                name = format!("{}{}", self.prefix, name);
            }
            if !self.suffix.is_empty() {
                if let Some(dot) = name.rfind('.') {
                    name.insert_str(dot, &self.suffix);
                } else {
                    name.push_str(&self.suffix);
                }
            }
            if !self.find.is_empty() {
                name = name.replace(&self.find, &self.replace);
            }
            self.preview.push(RenameItem {
                original: file.clone(),
                renamed: name,
            });
        }
        self.status = format!("Preview: {} files", self.preview.len());
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
            "bg": t.background_panel, "title": String::from(" Batch Renamer "),
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let fields = [
            ("Prefix", &self.prefix, EditMode::Prefix),
            ("Suffix", &self.suffix, EditMode::Suffix),
            ("Find", &self.find, EditMode::Find),
            ("Replace", &self.replace, EditMode::Replace),
        ];
        for (i, (label, value, mode)) in fields.iter().enumerate() {
            let active = *mode == self.edit_mode;
            cmds.push(json!({"Text": {
                "x": 2, "y": 1 + i as u16, "text": format!(
                    "{} {}: {}",
                    if active { ">" } else { " " },
                    label,
                    value
                ),
                "fg": if active { t.accent } else { t.text },
                "bg": null, "bold": active, "modifiers": 0,
            }}));
        }

        let list_y = 6;
        let list_h = h.saturating_sub(9).max(1);
        let header = if self.preview.is_empty() {
            String::new()
        } else {
            format!("{:<30} -> {}", "Original", "Renamed")
        };
        if !header.is_empty() {
            cmds.push(json!({"Text": {
                "x": 2, "y": list_y - 1, "text": header,
                "fg": t.text_muted, "bg": null, "bold": true, "modifiers": 0,
            }}));
        }

        let items: Vec<String> = self
            .preview
            .iter()
            .map(|p| format!("{:<30} -> {}", p.original, p.renamed))
            .collect();
        cmds.push(json!({"List": {
            "x": 2, "y": list_y, "w": w.saturating_sub(4), "h": list_h,
            "items": items,
            "selected": if self.selected < self.preview.len() { Some(self.selected) } else { None },
            "style": {"fg": t.text, "bg": null, "bold": false, "modifiers": 0},
            "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": String::from("p/s/f/r: edit field · Tab: cycle · Enter: preview · a: add file · d: del · Esc: close"),
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
    json!([["Plugins", "Batch Renamer"]])
}

fn key_hints() -> Value {
    json!([
        ["p", "edit prefix"],
        ["s", "edit suffix"],
        ["f", "edit find"],
        ["r", "edit replace"],
        ["Enter", "preview"],
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
                log::error!("[batch-renamer] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
