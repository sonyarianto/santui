use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::Read;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    port: u16,
    listening: bool,
    requests: Vec<String>,
    server_rx: Option<mpsc::Receiver<String>>,
    status: String,
    scroll: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            port: 9999,
            listening: false,
            requests: Vec::new(),
            server_rx: None,
            status: "Press s to start webhook server".into(),
            scroll: 0,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Char('s') if !modifiers.ctrl => {
                if self.listening {
                    self.status = "Server already running".into();
                } else {
                    self.start_server();
                }
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.requests.clear();
                self.scroll = 0;
                self.status = "Cleared".into();
                true
            }
            IpcKey::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                true
            }
            IpcKey::Down => {
                let max = self.requests.len().saturating_sub(1);
                self.scroll = self.scroll.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Home => {
                self.scroll = 0;
                true
            }
            IpcKey::End => {
                self.scroll = self.requests.len().saturating_sub(1);
                true
            }
            _ => true,
        }
    }

    fn start_server(&mut self) {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = match TcpListener::bind(&addr) {
            Ok(l) => l,
            Err(e) => {
                self.status = format!("Failed to bind {addr}: {e}");
                return;
            }
        };
        listener.set_nonblocking(true).unwrap_or(());
        let (tx, rx) = mpsc::channel::<String>();
        self.server_rx = Some(rx);
        self.listening = true;
        self.status = format!("Listening on http://localhost:{}/", self.port);
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut s) => {
                        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut buf = vec![0u8; 4096];
                        match s.read(&mut buf) {
                            Ok(n) if n > 0 => {
                                let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                                let summary = format!(
                                    "[{}] {} bytes\n{}",
                                    chrono_now(),
                                    n,
                                    raw.lines().next().unwrap_or("")
                                );
                                let _ = tx.send(summary);
                                let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                                let _ = s.write_all(response.as_bytes());
                            }
                            _ => {}
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => break,
                }
            }
        });
    }

    fn poll_requests(&mut self) {
        if let Some(ref rx) = self.server_rx {
            while let Ok(req) = rx.try_recv() {
                self.requests.push(req);
                if self.requests.len() > 500 {
                    self.requests.remove(0);
                }
                self.scroll = self.requests.len().saturating_sub(1);
                self.dirty = true;
            }
        }
    }

    fn render(&mut self) -> Vec<Value> {
        self.poll_requests();
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
            "bg": t.background_panel, "title": " Webhook Tester ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let status_color = if self.listening {
            t.success
        } else {
            t.text_muted
        };
        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": format!("Port: {}  {}", self.port,
                if self.listening { "[LISTENING]" } else { "[STOPPED]" }),
            "fg": status_color, "bg": null, "bold": true, "modifiers": 0,
        }}));

        let content_y = 3u16;
        let max_rows = h.saturating_sub(5) as usize;

        if self.requests.is_empty() {
            cmds.push(json!({"Text": {
                "x": 2, "y": content_y,
                "text": if self.listening {
                    String::from("Waiting for incoming webhooks...")
                } else {
                    String::from("Press s to start the server")
                },
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
            }}));
        } else {
            for (i, req) in self
                .requests
                .iter()
                .skip(self.scroll)
                .take(max_rows)
                .enumerate()
            {
                cmds.push(json!({"Text": {
                    "x": 2, "y": content_y + i as u16,
                    "text": req.clone(),
                    "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
                }}));
            }
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": format!("{} requests received", self.requests.len()),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
    }
}

fn chrono_now() -> String {
    "now".into()
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
    json!([["Plugins", "Webhook Tester"]])
}

fn key_hints() -> Value {
    json!([["esc", "close"], ["s", "start server"], ["c", "clear"],])
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
                log::error!("[webhook-tester] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
