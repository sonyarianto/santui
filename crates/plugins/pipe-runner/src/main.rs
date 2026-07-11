use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    input: String,
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
            input: String::new(),
            cursor_pos: 0,
            output: Vec::new(),
            status: "Type a shell command and press Enter to run".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Char('\n') | IpcKey::Char('\r') => {
                self.run();
                true
            }
            IpcKey::Char(c) if !modifiers.ctrl => {
                if c == '\u{7f}' || c == '\x08' {
                    if self.cursor_pos > 0 {
                        self.input.remove(self.cursor_pos - 1);
                        self.cursor_pos -= 1;
                    }
                } else {
                    self.input.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                }
                true
            }
            IpcKey::Left => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
                true
            }
            IpcKey::Right => {
                if self.cursor_pos < self.input.len() {
                    self.cursor_pos += 1;
                }
                true
            }
            IpcKey::Home => {
                self.cursor_pos = 0;
                true
            }
            IpcKey::End => {
                self.cursor_pos = self.input.len();
                true
            }
            IpcKey::Backspace => {
                if self.cursor_pos > 0 {
                    self.input.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
                true
            }
            IpcKey::Delete => {
                if self.cursor_pos < self.input.len() {
                    self.input.remove(self.cursor_pos);
                }
                true
            }
            _ => true,
        }
    }

    fn run(&mut self) {
        let cmd = self.input.trim();
        if cmd.is_empty() {
            self.status = "No command entered".into();
            return;
        }
        self.status = format!("Running: {cmd}");
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            self.output = vec!["Empty command".into()];
            return;
        }
        let program = parts[0];
        let args = &parts[1..];
        match Command::new(program).args(args).output() {
            Ok(out) => {
                let mut lines: Vec<String> = Vec::new();
                if !out.stdout.is_empty() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    lines.extend(text.lines().map(|l| l.to_string()));
                }
                if !out.stderr.is_empty() {
                    if !lines.is_empty() {
                        lines.push(String::new());
                    }
                    lines.push("--- stderr ---".into());
                    let text = String::from_utf8_lossy(&out.stderr);
                    lines.extend(text.lines().map(|l| l.to_string()));
                }
                if lines.is_empty() {
                    lines.push(format!(
                        "Command succeeded (exit code: {})",
                        out.status.code().unwrap_or(-1)
                    ));
                }
                self.output = lines;
                self.status = format!("Exit: {}", out.status);
            }
            Err(e) => {
                self.output = vec![format!("Error: {e}")];
                self.status = "Command failed".into();
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
            "bg": t.background_panel, "title": " Pipe Runner ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": "$", "fg": t.success, "bg": null,
            "bold": true, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 4, "y": 1,
            "text": if self.input.is_empty() { String::from("ls -la") } else { self.input.clone() },
            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 4 + self.cursor_pos as u16, "y": 1,
            "text": String::from("\u{258c}"), "fg": t.accent, "bg": null,
            "bold": false, "modifiers": 2,
        }}));

        let box_y = 3;
        let box_w = w.saturating_sub(4);
        let box_h = h.saturating_sub(5).max(4);

        cmds.push(json!({"Border": {
            "x": 2, "y": box_y, "w": box_w, "h": box_h, "fg": t.accent,
            "borders": BORDER_ALL, "bg": t.background,
            "title": " Output ", "title_fg": t.accent,
            "title_dash_fg": t.border, "border_type": null,
        }}));

        if self.output.is_empty() {
            cmds.push(json!({"Text": {
                "x": 4, "y": box_y + 1,
                "text": String::from("Run a command to see output here"),
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

fn palette_commands() -> Value {
    json!([["Plugins", "Pipe Runner"]])
}

fn key_hints() -> Value {
    json!([["esc", "close"], ["enter", "run command"],])
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
                log::error!("[pipe-runner] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
