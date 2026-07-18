use std::io::{BufRead, BufReader};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct ScanResult {
    port: u16,
    open: bool,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    host: String,
    port_start: u16,
    port_end: u16,
    input_mode: InputMode,
    results: Vec<ScanResult>,
    scanning: bool,
    scanned: bool,
    status: String,
}

#[derive(Debug, Clone)]
enum InputMode {
    Host,
    PortStart,
    PortEnd,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            host: String::from("127.0.0.1"),
            port_start: 1,
            port_end: 100,
            input_mode: InputMode::Host,
            results: Vec::new(),
            scanning: false,
            scanned: false,
            status: String::from("Enter host and port range, then press s to scan"),
        }
    }
}

impl App {
    fn start_scan(&mut self) {
        self.scanning = true;
        self.scanned = false;
        self.results.clear();
        self.status = String::from("Scanning...");
        self.dirty = true;

        let host = self.host.clone();
        let start = self.port_start;
        let end = self.port_end;

        let mut results = Vec::new();
        for port in start..=end {
            let addr = format!("{host}:{port}");
            let open = if let Ok(mut addrs) = addr.to_socket_addrs() {
                if let Some(sockaddr) = addrs.next() {
                    TcpStream::connect_timeout(&sockaddr, Duration::from_millis(500)).is_ok()
                } else {
                    false
                }
            } else {
                false
            };
            results.push(ScanResult { port, open });
        }

        self.results = results;
        self.scanning = false;
        self.scanned = true;
        let open_count = self.results.iter().filter(|r| r.open).count();
        self.status = format!(
            "Scan complete: {open_count}/{count} ports open",
            count = self.results.len()
        );
        self.dirty = true;
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Tab => {
                self.input_mode = match self.input_mode {
                    InputMode::Host => InputMode::PortStart,
                    InputMode::PortStart => InputMode::PortEnd,
                    InputMode::PortEnd => InputMode::Host,
                };
                true
            }
            IpcKey::Char('s') => {
                if !self.scanning {
                    self.start_scan();
                }
                true
            }
            IpcKey::Backspace => {
                match self.input_mode {
                    InputMode::Host => {
                        self.host.pop();
                    }
                    InputMode::PortStart => {
                        let val = self.port_start / 10;
                        self.port_start = val;
                    }
                    InputMode::PortEnd => {
                        let val = self.port_end / 10;
                        self.port_end = val;
                    }
                }
                true
            }
            IpcKey::Char(c) if c.is_ascii_digit() => {
                match self.input_mode {
                    InputMode::Host => self.host.push(c),
                    InputMode::PortStart => {
                        self.port_start = self.port_start * 10 + c.to_digit(10).unwrap() as u16;
                    }
                    InputMode::PortEnd => {
                        self.port_end = self.port_end * 10 + c.to_digit(10).unwrap() as u16;
                    }
                }
                true
            }
            IpcKey::Char('.') => {
                if matches!(self.input_mode, InputMode::Host) {
                    self.host.push('.');
                }
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
            "bg": t.background_panel, "title": " Port Scanner ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let host_label = if matches!(self.input_mode, InputMode::Host) {
            "\u{25b6} Host:"
        } else {
            "  Host:"
        };
        let port_start_label = if matches!(self.input_mode, InputMode::PortStart) {
            "\u{25b6} Start:"
        } else {
            "  Start:"
        };
        let port_end_label = if matches!(self.input_mode, InputMode::PortEnd) {
            "\u{25b6} End:"
        } else {
            "  End:"
        };

        cmds.push(json!({"Text": {
            "x": 2, "y": 1,
            "text": format!("{host_label} {}", self.host),
            "fg": t.text, "bg": null, "bold": matches!(self.input_mode, InputMode::Host),
            "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 2,
            "text": format!("{port_start_label} {}", self.port_start),
            "fg": t.text, "bg": null, "bold": matches!(self.input_mode, InputMode::PortStart),
            "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 3,
            "text": format!("{port_end_label} {}", self.port_end),
            "fg": t.text, "bg": null, "bold": matches!(self.input_mode, InputMode::PortEnd),
            "modifiers": 0,
        }}));

        let result_y = 5u16;
        let max_results = h.saturating_sub(result_y + 3) as usize;

        if self.scanning {
            cmds.push(json!({"Text": {
                "x": 2, "y": result_y,
                "text": String::from("\u{23f3} Scanning..."),
                "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
            }}));
        } else if self.scanned {
            let items: Vec<String> = self
                .results
                .iter()
                .take(max_results)
                .map(|r| {
                    if r.open {
                        format!("\u{2705} Port {}  OPEN", r.port)
                    } else {
                        format!("\u{274c} Port {}", r.port)
                    }
                })
                .collect();
            if items.is_empty() {
                cmds.push(json!({"Text": {
                    "x": 2, "y": result_y,
                    "text": String::from("No results"),
                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
                }}));
            } else {
                cmds.push(json!({"List": {
                    "x": 1, "y": result_y, "w": w.saturating_sub(2),
                    "h": max_results.min(items.len()) as u16,
                    "items": items, "selected": None::<usize>,
                    "style": {"fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0},
                    "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
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
    vec![]
}

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("esc".into(), "close".into()),
        ("tab".into(), "switch field".into()),
        ("s".into(), "start scan".into()),
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
                log::error!("[port-scanner] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
