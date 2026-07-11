use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, LogEntry, ThemeData, BORDER_ALL,
};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

const DEFAULT_PATH: &str = "/var/log/syslog";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogSource {
    File,
    Runtime,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    path: String,
    lines: Vec<String>,
    scroll: usize,
    filter: String,
    mode: Mode,
    status: String,
    source: LogSource,
    runtime_logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    View,
    Path,
    Filter,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            path: DEFAULT_PATH.into(),
            lines: Vec::new(),
            scroll: 0,
            filter: String::new(),
            mode: Mode::View,
            status: "p path · f filter · ↑↓ scroll · g goto · r reload · l runtime · esc".into(),
            source: LogSource::File,
            runtime_logs: Vec::new(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match self.mode {
            Mode::Path => {
                match key {
                    IpcKey::Esc => {
                        self.mode = Mode::View;
                        self.status =
                            "p path · f filter · ↑↓ scroll · g goto · r reload · l runtime · esc"
                                .into();
                    }
                    IpcKey::Char('\n') | IpcKey::Char('\r') => {
                        self.mode = Mode::View;
                        self.load_file();
                    }
                    IpcKey::Char(c) if !modifiers.ctrl => {
                        if c == '\u{7f}' || c == '\x08' {
                            self.path.pop();
                        } else {
                            self.path.push(c);
                        }
                    }
                    IpcKey::Backspace => {
                        self.path.pop();
                    }
                    _ => {}
                }
                return true;
            }
            Mode::Filter => {
                match key {
                    IpcKey::Esc => {
                        self.filter.clear();
                        self.mode = Mode::View;
                        self.status =
                            "p path · f filter · ↑↓ scroll · g goto · r reload · l runtime · esc"
                                .into();
                    }
                    IpcKey::Char('\n') | IpcKey::Char('\r') => {
                        self.mode = Mode::View;
                        self.scroll = 0;
                    }
                    IpcKey::Char(c) if !modifiers.ctrl => {
                        if c == '\u{7f}' || c == '\x08' {
                            self.filter.pop();
                        } else {
                            self.filter.push(c);
                        }
                    }
                    IpcKey::Backspace => {
                        self.filter.pop();
                    }
                    _ => {}
                }
                return true;
            }
            Mode::View => {}
        }
        match key {
            IpcKey::Esc => false,
            IpcKey::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                true
            }
            IpcKey::Down => {
                let max_scroll = self.visible_count().saturating_sub(1);
                self.scroll = self
                    .scroll
                    .min(max_scroll)
                    .saturating_add(1)
                    .min(max_scroll);
                true
            }
            IpcKey::PageUp => {
                let page = self.area.h.saturating_sub(8) as usize;
                self.scroll = self.scroll.saturating_sub(page);
                true
            }
            IpcKey::PageDown => {
                let page = self.area.h.saturating_sub(8) as usize;
                let max_scroll = self.visible_count().saturating_sub(1);
                self.scroll = (self.scroll + page).min(max_scroll);
                true
            }
            IpcKey::Home => {
                self.scroll = 0;
                true
            }
            IpcKey::End => {
                self.scroll = self.visible_count().saturating_sub(1);
                true
            }
            IpcKey::Char('p') if !modifiers.ctrl => {
                self.mode = Mode::Path;
                self.status = "Enter log file path:".into();
                true
            }
            IpcKey::Char('f') if !modifiers.ctrl => {
                self.mode = Mode::Filter;
                self.status = "Enter filter text:".into();
                true
            }
            IpcKey::Char('r') if !modifiers.ctrl => {
                self.load_file();
                self.status = format!("Reloaded: {} lines", self.lines.len());
                true
            }
            IpcKey::Char('g') if !modifiers.ctrl => {
                self.scroll = 0;
                true
            }
            IpcKey::Char('l') if !modifiers.ctrl => {
                self.source = match self.source {
                    LogSource::File => LogSource::Runtime,
                    LogSource::Runtime => LogSource::File,
                };
                self.scroll = if matches!(self.source, LogSource::Runtime) {
                    self.runtime_logs.len().saturating_sub(1)
                } else {
                    0
                };
                let label = match self.source {
                    LogSource::File => "file",
                    LogSource::Runtime => "runtime",
                };
                self.status = format!(
                    "Switched to {label} logs ({} entries)",
                    self.visible_count()
                );
                true
            }
            _ => true,
        }
    }

    fn load_file(&mut self) {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => {
                self.lines = content.lines().map(|l| l.to_string()).collect();
                if self.lines.len() > 10000 {
                    self.lines = self.lines[self.lines.len() - 10000..].to_vec();
                }
                self.status = format!("Loaded: {} lines from {}", self.lines.len(), self.path);
                self.scroll = self.lines.len().saturating_sub(1);
            }
            Err(e) => {
                self.lines = vec![format!("Error reading {}: {e}", self.path)];
                self.status = "Failed to load file".into();
                self.scroll = 0;
            }
        }
    }

    fn visible_count(&self) -> usize {
        match self.source {
            LogSource::File => {
                if self.filter.is_empty() {
                    self.lines.len()
                } else {
                    self.lines
                        .iter()
                        .filter(|l| l.contains(&self.filter))
                        .count()
                }
            }
            LogSource::Runtime => {
                if self.filter.is_empty() {
                    self.runtime_logs.len()
                } else {
                    self.runtime_logs
                        .iter()
                        .filter(|e| {
                            e.message.contains(&self.filter) || e.target.contains(&self.filter)
                        })
                        .count()
                }
            }
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
        let source_label = match self.source {
            LogSource::File => "File",
            LogSource::Runtime => "Runtime",
        };
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": format!(" Log Viewer [{source_label}] "),
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let mode_text = match self.mode {
            Mode::View => format!("[VIEW] {}", self.path),
            Mode::Path => format!("[PATH] {}", self.path),
            Mode::Filter => {
                if self.filter.is_empty() {
                    "[FILTER] (type filter)".into()
                } else {
                    format!("[FILTER] {}", self.filter)
                }
            }
        };
        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": mode_text, "fg": t.text_muted, "bg": null,
            "bold": true, "modifiers": 0,
        }}));

        let content_y = 3u16;
        let content_h = h.saturating_sub(6) as usize;
        let total = self.visible_count();

        match self.source {
            LogSource::File => {
                let filtered: Vec<&String> = if self.filter.is_empty() {
                    self.lines.iter().collect()
                } else {
                    self.lines
                        .iter()
                        .filter(|l| l.contains(&self.filter))
                        .collect()
                };

                if filtered.is_empty() {
                    cmds.push(json!({"Text": {
                        "x": 2, "y": content_y,
                        "text": if self.lines.is_empty() {
                            "Press p to enter a log file path, then Enter".to_string()
                        } else {
                            "No matching lines (press f to change filter)".to_string()
                        },
                        "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
                    }}));
                } else {
                    for (i, line) in filtered
                        .iter()
                        .skip(self.scroll)
                        .take(content_h)
                        .enumerate()
                    {
                        cmds.push(json!({"Text": {
                            "x": 2, "y": content_y + i as u16,
                            "text": line.to_string(),
                            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
                        }}));
                    }
                }
            }
            LogSource::Runtime => {
                let filtered: Vec<&LogEntry> = if self.filter.is_empty() {
                    self.runtime_logs.iter().collect()
                } else {
                    self.runtime_logs
                        .iter()
                        .filter(|e| {
                            e.message.contains(&self.filter) || e.target.contains(&self.filter)
                        })
                        .collect()
                };

                if filtered.is_empty() {
                    cmds.push(json!({"Text": {
                        "x": 2, "y": content_y,
                        "text": if self.runtime_logs.is_empty() {
                            "Waiting for log entries...".to_string()
                        } else {
                            "No matching entries (press f to change filter)".to_string()
                        },
                        "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
                    }}));
                } else {
                    for (i, entry) in filtered
                        .iter()
                        .skip(self.scroll)
                        .take(content_h)
                        .enumerate()
                    {
                        let level_fg = match entry.level.as_str() {
                            "ERROR" => t.error,
                            "WARN" => [255, 200, 0],
                            _ => t.text,
                        };
                        let line = format!("[{}] {}: {}", entry.level, entry.target, entry.message);
                        cmds.push(json!({"Text": {
                            "x": 2, "y": content_y + i as u16,
                            "text": line,
                            "fg": level_fg, "bg": null, "bold": entry.level == "ERROR",
                            "modifiers": 0,
                        }}));
                    }
                }
            }
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": format!("{} visible [{}..{}]  {} total in buffer",
                total, self.scroll, self.scroll + content_h, self.entries_total()),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
    }

    fn entries_total(&self) -> usize {
        match self.source {
            LogSource::File => self.lines.len(),
            LogSource::Runtime => self.runtime_logs.len(),
        }
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
        {"key": "p", "hint": "path"},
        {"key": "f", "hint": "filter"},
        {"key": "r", "hint": "reload"},
        {"key": "l", "hint": "log source"},
        {"key": "g", "hint": "top"},
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
            Ok(HostMsg::LogEntries { entries }) => {
                for entry in entries {
                    if app.runtime_logs.len() >= 10000 {
                        app.runtime_logs.remove(0);
                    }
                    app.runtime_logs.push(entry);
                }
                app.dirty = true;
                if matches!(app.source, LogSource::Runtime) {
                    app.scroll = app.runtime_logs.len().saturating_sub(1);
                }
                false
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
                log::error!("[log-viewer] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
