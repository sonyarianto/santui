use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::process::Command;

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    domain: String,
    cursor_pos: usize,
    output: Vec<String>,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            domain: String::new(),
            cursor_pos: 0,
            output: Vec::new(),
            status: "Type a domain and press Enter to look up".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Char(c) if !modifiers.ctrl => {
                if c == '\n' || c == '\r' {
                    self.lookup();
                } else if c == '\u{7f}' || c == '\x08' {
                    if self.cursor_pos > 0 {
                        self.domain.remove(self.cursor_pos - 1);
                        self.cursor_pos -= 1;
                    }
                } else {
                    self.domain.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                }
                true
            }
            IpcKey::Left => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
                true
            }
            IpcKey::Right => {
                if self.cursor_pos < self.domain.len() {
                    self.cursor_pos += 1;
                }
                true
            }
            IpcKey::Home => {
                self.cursor_pos = 0;
                true
            }
            IpcKey::End => {
                self.cursor_pos = self.domain.len();
                true
            }
            IpcKey::Backspace => {
                if self.cursor_pos > 0 {
                    self.domain.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
                true
            }
            IpcKey::Delete => {
                if self.cursor_pos < self.domain.len() {
                    self.domain.remove(self.cursor_pos);
                }
                true
            }
            _ => true,
        }
    }

    fn lookup(&mut self) {
        let domain = self.domain.trim().to_lowercase();
        if domain.is_empty() {
            self.output = vec!["Please enter a domain name".into()];
            self.status = "No domain entered".into();
            return;
        }
        self.status = format!("Looking up {domain}...");
        self.output = vec![format!("Running whois for {domain}...")];
        match Command::new("whois").arg(&domain).output() {
            Ok(out) => {
                let raw = if out.status.success() || !out.stderr.is_empty() {
                    String::from_utf8_lossy(&out.stdout).to_string()
                } else {
                    format!("Error: {}", String::from_utf8_lossy(&out.stderr))
                };
                if raw.trim().is_empty() {
                    self.output = vec![format!("No WHOIS data returned for {domain}")];
                    self.status = "No results".into();
                } else {
                    self.output = raw.lines().map(|l| l.to_string()).collect();
                    self.status = format!("{} lines", self.output.len());
                }
            }
            Err(e) => {
                self.output = vec![
                    format!("Failed to run whois: {e}"),
                    "Ensure whois is installed on your system.".into(),
                ];
                self.status = "Error running whois".into();
            }
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(52);
        let h = self.area.h.max(14);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": " WHOIS Lookup ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": "Domain:", "fg": t.text_muted, "bg": null,
            "bold": true, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 2,
            "text": if self.domain.is_empty() { "(type domain)".to_string() } else { self.domain.clone() },
            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2 + self.cursor_pos as u16 + 2, "y": 2,
            "text": String::from("\u{258c}"), "fg": t.accent, "bg": null,
            "bold": false, "modifiers": 2,
        }}));

        let box_y = 4;
        let box_w = w.saturating_sub(4);
        let box_h = h.saturating_sub(6).max(4);

        cmds.push(json!({"Border": {
            "x": 2, "y": box_y, "w": box_w, "h": box_h, "fg": t.accent,
            "borders": BORDER_ALL, "bg": t.background, "title": " Results ",
            "title_fg": t.accent, "title_dash_fg": t.border, "border_type": null,
        }}));

        if self.output.is_empty() {
            cmds.push(json!({"Text": {
                "x": 4, "y": box_y + 1,
                "text": "Enter a domain and press Enter to look up WHOIS data",
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
            }}));
        } else {
            let max_rows = box_h.saturating_sub(2) as usize;
            let visible = &self.output[..self.output.len().min(max_rows)];
            for (i, line) in visible.iter().enumerate() {
                cmds.push(json!({"Text": {
                    "x": 4, "y": box_y + 1 + i as u16,
                    "text": line.clone(),
                    "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
                }}));
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
    vec![("Plugins".to_string(), "Whois Lookup".to_string())]
}

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("esc".to_string(), "close".to_string()),
        ("enter".to_string(), "lookup".to_string()),
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
                log::error!("[whois-lookup] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
