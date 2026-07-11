use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordType {
    A,
    Mx,
    Ns,
    Txt,
    Cname,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    domain: String,
    cursor_pos: usize,
    record_type: RecordType,
    results: Vec<String>,
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
            record_type: RecordType::A,
            results: Vec::new(),
            status: "Enter domain and press Enter to resolve DNS".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Char('\n') | IpcKey::Char('\r') => {
                self.lookup();
                true
            }
            IpcKey::Char('t') if !modifiers.ctrl => {
                self.record_type = match self.record_type {
                    RecordType::A => RecordType::Mx,
                    RecordType::Mx => RecordType::Ns,
                    RecordType::Ns => RecordType::Txt,
                    RecordType::Txt => RecordType::Cname,
                    RecordType::Cname => RecordType::A,
                };
                if !self.domain.is_empty() {
                    self.lookup();
                }
                true
            }
            IpcKey::Char(c) if !modifiers.ctrl => {
                if c == '\u{7f}' || c == '\x08' {
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

    fn type_flag(&self) -> &'static str {
        match self.record_type {
            RecordType::A => "-t A",
            RecordType::Mx => "-t MX",
            RecordType::Ns => "-t NS",
            RecordType::Txt => "-t TXT",
            RecordType::Cname => "-t CNAME",
        }
    }

    fn type_label(&self) -> &'static str {
        match self.record_type {
            RecordType::A => "A",
            RecordType::Mx => "MX",
            RecordType::Ns => "NS",
            RecordType::Txt => "TXT",
            RecordType::Cname => "CNAME",
        }
    }

    fn lookup(&mut self) {
        let domain = self.domain.trim().to_lowercase();
        if domain.is_empty() {
            return;
        }
        self.status = format!("Resolving {} records for {domain}...", self.type_label());
        match Command::new("nslookup")
            .args([self.type_flag(), &domain])
            .output()
        {
            Ok(out) => {
                let raw = String::from_utf8_lossy(&out.stdout).to_string()
                    + &String::from_utf8_lossy(&out.stderr);
                let lines: Vec<String> = raw.lines().map(|l| l.to_string()).collect();
                if lines.is_empty() {
                    self.results = vec![format!("No {} records found", self.type_label())];
                } else {
                    self.results = lines;
                }
                self.status = format!("{}: {} lines", self.type_label(), self.results.len());
            }
            Err(e) => {
                self.results = vec![
                    format!("nslookup error: {e}"),
                    "Try installing dnsutils/bind-utils.".into(),
                ];
                self.status = "nslookup failed".into();
            }
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(56);
        let h = self.area.h.max(14);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": " DNS Globe ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": format!("Record: {} (t to toggle)", self.type_label()),
            "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 2, "text": "Domain:", "fg": t.text_muted, "bg": null,
            "bold": true, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 3,
            "text": if self.domain.is_empty() { String::from("example.com") } else { self.domain.clone() },
            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2 + self.cursor_pos as u16 + 2, "y": 3,
            "text": String::from("\u{258c}"), "fg": t.accent, "bg": null,
            "bold": false, "modifiers": 2,
        }}));

        let box_y = 5;
        let box_w = w.saturating_sub(4);
        let box_h = h.saturating_sub(7).max(4);

        cmds.push(json!({"Border": {
            "x": 2, "y": box_y, "w": box_w, "h": box_h, "fg": t.accent,
            "borders": BORDER_ALL, "bg": t.background,
            "title": format!(" {} Records ", self.type_label()),
            "title_fg": t.accent, "title_dash_fg": t.border, "border_type": null,
        }}));

        if self.results.is_empty() {
            cmds.push(json!({"Text": {
                "x": 4, "y": box_y + 1,
                "text": String::from("Enter a domain and press Enter"),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
            }}));
        } else {
            let max_rows = box_h.saturating_sub(2) as usize;
            for (i, line) in self.results.iter().take(max_rows).enumerate() {
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
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": String::from("type domain \u{b7} enter resolve \u{b7} t type \u{b7} esc"),
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
    json!([["Plugins", "Dns Globe"]])
}

fn key_hints() -> Value {
    json!([["esc", "close"], ["enter", "resolve"], ["t", "toggle type"],])
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
                log::error!("[dns-globe] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
