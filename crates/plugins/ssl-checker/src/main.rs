use std::io::{BufRead, BufReader, Write};

use rand::RngExt;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct CertInfo {
    domain: String,
    subject: String,
    issuer: String,
    expiry_days: u32,
    valid: bool,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    input: String,
    input_cursor: usize,
    result: Option<CertInfo>,
    checking: bool,
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
            input_cursor: 0,
            result: None,
            checking: false,
            status: String::from("Enter a domain:port (e.g. example.com:443) and press Enter"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.checking {
            return true;
        }
        match key {
            IpcKey::Enter => {
                if !self.input.is_empty() {
                    self.check_cert();
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                let pos = self.input_cursor;
                if pos <= self.input.len() {
                    self.input.insert(pos, c);
                    self.input_cursor += 1;
                }
                true
            }
            IpcKey::Backspace => {
                if self.input_cursor > 0 {
                    let pos = self.input_cursor - 1;
                    if pos < self.input.len() {
                        self.input.remove(pos);
                    }
                    self.input_cursor -= 1;
                }
                true
            }
            IpcKey::Delete => {
                if self.input_cursor < self.input.len() {
                    self.input.remove(self.input_cursor);
                }
                true
            }
            IpcKey::Left => {
                self.input_cursor = self.input_cursor.saturating_sub(1);
                true
            }
            IpcKey::Right => {
                if self.input_cursor < self.input.len() {
                    self.input_cursor += 1;
                }
                true
            }
            IpcKey::Home => {
                self.input_cursor = 0;
                true
            }
            IpcKey::End => {
                self.input_cursor = self.input.len();
                true
            }
            IpcKey::Esc => false,
            _ => true,
        }
    }

    fn check_cert(&mut self) {
        let domain = self.input.trim().to_string();
        if domain.is_empty() {
            self.status = String::from("Please enter a domain");
            return;
        }
        let (host, _port) = if let Some(pos) = domain.rfind(':') {
            let h = domain[..pos].to_string();
            let p: u16 = domain[pos + 1..].parse().unwrap_or(443);
            (h, p)
        } else {
            (domain.clone(), 443)
        };

        let mut rng = rand::rng();
        let expiry_days = rng.random_range(30..400);
        let valid = expiry_days > 30;
        let suffixes = [".com", ".org", ".net", ".io"];
        let issuer_suffix = suffixes[rng.random_range(0..suffixes.len())];

        self.result = Some(CertInfo {
            domain: domain.clone(),
            subject: format!("CN={}", host),
            issuer: format!("CN=Santui Mock CA{}", issuer_suffix),
            expiry_days,
            valid,
        });
        self.status = format!("Certificate check complete for {}", domain);
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(42);
    let h = app.area.h.max(14);

    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: t.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: t.border,
        borders: BORDER_ALL,
        bg: Some(t.background_panel),
        title: Some(String::from(" SSL/TLS Checker ")),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!("Domain: {}", app.input),
        fg: Some(t.highlight),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    if let Some(ref cert) = app.result {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 3,
            text: String::from("Certificate Information:"),
            fg: Some(t.text),
            bg: None,
            bold: true,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 4,
            text: format!("  Domain:  {}", cert.domain),
            fg: Some(t.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 5,
            text: format!("  Subject: {}", cert.subject),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 6,
            text: format!("  Issuer:  {}", cert.issuer),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });

        let expiry_color = if cert.expiry_days > 180 {
            t.success
        } else if cert.expiry_days > 30 {
            t.text
        } else {
            t.error
        };
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 7,
            text: format!("  Expires: {} days", cert.expiry_days),
            fg: Some(expiry_color),
            bg: None,
            bold: true,
            modifiers: 0,
        });

        let valid_text = if cert.valid { "VALID" } else { "EXPIRED" };
        let valid_color = if cert.valid { t.success } else { t.error };
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 9,
            text: format!("  Status:  {}", valid_text),
            fg: Some(valid_color),
            bg: None,
            bold: true,
            modifiers: 0,
        });
    }

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: app.status.clone(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds
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

fn hints() -> Vec<(String, String)> {
    vec![
        ("enter".into(), "check".into()),
        ("esc".into(), "back".into()),
    ]
}

fn palette_commands() -> serde_json::Value {
    serde_json::json!([("SSL".to_string(), "Open SSL/TLS Checker".to_string())])
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": hints(),
        "palette_commands": palette_commands(),
        "request": null,
        "plugin_message": null,
        "consumed": consumed,
    });
    if let Ok(json_str) = serde_json::to_string(&json) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{json_str}");
        let _ = out.flush();
    }
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = App::default();
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        let trimmed = line.trim_end();
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
                log::error!("[ssl-checker] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
