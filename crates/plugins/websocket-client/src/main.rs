use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::thread;
use tungstenite::Message;

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    url: String,
    cursor_pos: usize,
    connected: bool,
    messages: Vec<String>,
    input: String,
    input_cursor: usize,
    rx: Option<mpsc::Receiver<String>>,
    tx: Option<mpsc::Sender<String>>,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            url: String::new(),
            cursor_pos: 0,
            connected: false,
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            rx: None,
            tx: None,
            status: "Enter WebSocket URL and press Enter to connect".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Char('\n') | IpcKey::Char('\r') => {
                if self.connected {
                    self.send_message();
                } else {
                    self.connect();
                }
                true
            }
            IpcKey::Char(c) if !modifiers.ctrl => {
                if self.connected {
                    if c == '\u{7f}' || c == '\x08' {
                        if self.input_cursor > 0 {
                            self.input.remove(self.input_cursor - 1);
                            self.input_cursor -= 1;
                        }
                    } else {
                        self.input.insert(self.input_cursor, c);
                        self.input_cursor += 1;
                    }
                } else {
                    if c == '\u{7f}' || c == '\x08' {
                        if self.cursor_pos > 0 {
                            self.url.remove(self.cursor_pos - 1);
                            self.cursor_pos -= 1;
                        }
                    } else {
                        self.url.insert(self.cursor_pos, c);
                        self.cursor_pos += 1;
                    }
                }
                true
            }
            IpcKey::Left => {
                if self.connected {
                    self.input_cursor = self.input_cursor.saturating_sub(1);
                } else {
                    self.cursor_pos = self.cursor_pos.saturating_sub(1);
                }
                true
            }
            IpcKey::Right => {
                if self.connected {
                    if self.input_cursor < self.input.len() {
                        self.input_cursor += 1;
                    }
                } else {
                    if self.cursor_pos < self.url.len() {
                        self.cursor_pos += 1;
                    }
                }
                true
            }
            IpcKey::Home => {
                self.cursor_pos = 0;
                self.input_cursor = 0;
                true
            }
            IpcKey::End => {
                self.cursor_pos = self.url.len();
                self.input_cursor = self.input.len();
                true
            }
            IpcKey::Backspace => {
                if self.connected && self.input_cursor > 0 {
                    self.input.remove(self.input_cursor - 1);
                    self.input_cursor -= 1;
                } else if !self.connected && self.cursor_pos > 0 {
                    self.url.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
                true
            }
            IpcKey::Delete => {
                if self.connected && self.input_cursor < self.input.len() {
                    self.input.remove(self.input_cursor);
                } else if !self.connected && self.cursor_pos < self.url.len() {
                    self.url.remove(self.cursor_pos);
                }
                true
            }
            _ => true,
        }
    }

    fn connect(&mut self) {
        let url = self.url.trim().to_string();
        if url.is_empty() {
            return;
        }
        self.status = format!("Connecting to {url}...");
        let (msg_tx, msg_rx) = mpsc::channel();
        let (send_tx, send_rx) = mpsc::channel();
        thread::spawn(move || match tungstenite::connect(&url) {
            Ok((mut stream, _)) => {
                let _ = msg_tx.send("Connected!".into());
                loop {
                    if let Ok(cmd) = send_rx.try_recv() {
                        if cmd == "__close__" {
                            break;
                        }
                        let _ = stream.send(Message::Text(cmd));
                    }
                    match stream.read() {
                        Ok(Message::Text(t)) => {
                            let _ = msg_tx.send(format!("<< {t}"));
                        }
                        Ok(Message::Close(_)) => {
                            let _ = msg_tx.send("Connection closed".into());
                            break;
                        }
                        Ok(_) => {}
                        Err(_) => {
                            let _ = msg_tx.send("Connection error".into());
                            break;
                        }
                    }
                    thread::sleep(std::time::Duration::from_millis(50));
                }
            }
            Err(e) => {
                let _ = msg_tx.send(format!("Connection failed: {e}"));
            }
        });
        self.rx = Some(msg_rx);
        self.tx = Some(send_tx);
        self.connected = true;
    }

    fn send_message(&mut self) {
        let msg = self.input.trim().to_string();
        if msg.is_empty() || self.tx.is_none() {
            return;
        }
        self.messages.push(format!(">> {msg}"));
        let _ = self.tx.as_ref().unwrap().send(msg);
        self.input.clear();
        self.input_cursor = 0;
        if self.messages.len() > 200 {
            self.messages.remove(0);
        }
    }

    fn poll(&mut self) {
        if let Some(ref rx) = self.rx {
            while let Ok(msg) = rx.try_recv() {
                self.messages.push(msg);
                if self.messages.len() > 200 {
                    self.messages.remove(0);
                }
                self.dirty = true;
            }
        }
    }

    fn render(&mut self) -> Vec<Value> {
        self.poll();
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
            "bg": t.background_panel, "title": " WebSocket Client ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let status_color = if self.connected {
            t.success
        } else {
            t.text_muted
        };
        cmds.push(json!({"Text": {
            "x": 2, "y": 1,
            "text": if self.connected { format!("Connected to {}", self.url) } else { self.url.clone() },
            "fg": status_color, "bg": null, "bold": true, "modifiers": 0,
        }}));

        let content_y = 3u16;
        let content_h = h.saturating_sub(7) as usize;

        cmds.push(json!({"Border": {
            "x": 2, "y": content_y, "w": w.saturating_sub(4), "h": content_h as u16 + 1,
            "fg": t.accent, "borders": BORDER_ALL, "bg": t.background,
            "title": " Messages ", "title_fg": t.accent,
            "title_dash_fg": t.border, "border_type": null,
        }}));

        if self.messages.is_empty() {
            cmds.push(json!({"Text": {
                "x": 4, "y": content_y + 1,
                "text": if self.connected { String::from("Connected. Type a message and press Enter.") } else { String::from("Enter a ws:// or wss:// URL and press Enter") },
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
            }}));
        } else {
            let start = self
                .messages
                .len()
                .saturating_sub(content_h.saturating_sub(2));
            for (i, msg) in self.messages[start..].iter().enumerate() {
                cmds.push(json!({"Text": {
                    "x": 4, "y": content_y + 1 + i as u16,
                    "text": msg.clone(),
                    "fg": if msg.starts_with(">>") { t.success } else { t.text },
                    "bg": null, "bold": false, "modifiers": 0,
                }}));
            }
        }

        let input_y = content_y + content_h as u16 + 2;
        if self.connected && input_y + 1 < h {
            cmds.push(json!({"Text": {
                "x": 2, "y": input_y, "text": format!("> {}", self.input),
                "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
            }}));
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": String::from("enter connect/send \u{b7} esc"),
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
        {"key": "enter", "hint": "connect/send"},
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
                log::error!("[websocket-client] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
