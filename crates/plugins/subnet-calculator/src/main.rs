use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Ip,
    Prefix,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    ip: String,
    mask: String,
    focus: Focus,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            ip: "192.168.1.10".into(),
            mask: "24".into(),
            focus: Focus::Ip,
            status: "Type IP and prefix \u{b7} tab focus \u{b7} c copy \u{b7} esc".into(),
        }
    }
}

impl App {
    fn parse_ip(s: &str) -> Option<[u8; 4]> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 4 {
            return None;
        }
        let mut octets = [0u8; 4];
        for (i, p) in parts.iter().enumerate() {
            octets[i] = p.trim().parse().ok()?;
        }
        Some(octets)
    }

    fn parse_prefix(s: &str) -> Option<u32> {
        let p: u32 = s.trim().parse().ok()?;
        if p <= 32 {
            Some(p)
        } else {
            None
        }
    }

    fn mask_from_prefix(prefix: u32) -> [u8; 4] {
        let bits = prefix;
        let full = if bits == 0 {
            0
        } else {
            0xFFFF_FFFFu32 << (32 - bits)
        };
        [
            ((full >> 24) & 0xFF) as u8,
            ((full >> 16) & 0xFF) as u8,
            ((full >> 8) & 0xFF) as u8,
            (full & 0xFF) as u8,
        ]
    }

    fn calc(&self) -> Option<Subnet> {
        let ip = Self::parse_ip(&self.ip)?;
        let prefix = Self::parse_prefix(&self.mask)?;
        let mask = Self::mask_from_prefix(prefix);
        let ip_u = u32::from_be_bytes(ip);
        let mask_u = u32::from_be_bytes(mask);
        let net_u = ip_u & mask_u;
        let wildcard = !mask_u;
        let bcast_u = net_u | wildcard;
        let network = net_u.to_be_bytes();
        let broadcast = bcast_u.to_be_bytes();
        let hosts = if prefix >= 31 {
            0
        } else {
            (bcast_u - net_u).saturating_sub(1)
        };
        let first = if prefix >= 31 {
            None
        } else {
            Some((net_u + 1).to_be_bytes())
        };
        let last = if prefix >= 31 {
            None
        } else {
            Some((bcast_u - 1).to_be_bytes())
        };
        Some(Subnet {
            network,
            broadcast,
            wildcard: wildcard.to_be_bytes(),
            first,
            last,
            hosts,
            prefix,
        })
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::Ip => Focus::Prefix,
                    Focus::Prefix => Focus::Ip,
                };
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                if let Some(s) = self.calc() {
                    let text = format!(
                        "{}.{}.{}.{}/{}",
                        s.network[0], s.network[1], s.network[2], s.network[3], s.prefix
                    );
                    match copy_to_clipboard(&text) {
                        Ok(()) => self.status = "Copied network CIDR".into(),
                        Err(e) => self.status = format!("Clipboard error: {e}"),
                    }
                }
                true
            }
            IpcKey::Backspace => {
                match self.focus {
                    Focus::Ip => {
                        self.ip.pop();
                    }
                    Focus::Prefix => {
                        self.mask.pop();
                    }
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                match self.focus {
                    Focus::Ip => {
                        if c.is_ascii_digit() || c == '.' {
                            self.ip.push(c);
                        }
                    }
                    Focus::Prefix => {
                        if c.is_ascii_digit() {
                            self.mask.push(c);
                        }
                    }
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

struct Subnet {
    network: [u8; 4],
    broadcast: [u8; 4],
    wildcard: [u8; 4],
    first: Option<[u8; 4]>,
    last: Option<[u8; 4]>,
    hosts: u32,
    prefix: u32,
}

fn fmt_ip(o: [u8; 4]) -> String {
    format!("{}.{}.{}.{}", o[0], o[1], o[2], o[3])
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(44);
    let h = app.area.h.max(16);

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
        title: Some(" Subnet Calculator ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: format!(
            "IP:   {}{}",
            if app.focus == Focus::Ip { "> " } else { "  " },
            app.ip
        ),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Ip,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: format!(
            "Prefix: {}{}",
            if app.focus == Focus::Prefix {
                "> "
            } else {
                "  "
            },
            app.mask
        ),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Prefix,
        modifiers: 0,
    });

    let box_y = 4;
    let box_w = w.saturating_sub(4);
    let box_h = 9;
    cmds.push(RenderCmd::Border {
        x: 2,
        y: box_y,
        w: box_w,
        h: box_h,
        fg: t.accent,
        borders: BORDER_ALL,
        bg: Some(t.background),
        title: None,
        title_fg: None,
        title_dash_fg: None,
        border_type: None,
    });

    if let Some(s) = app.calc() {
        let rows = vec![
            format!("Network:      {}/{}", fmt_ip(s.network), s.prefix),
            format!("Netmask:      {}", fmt_ip(app_mask(s.prefix))),
            format!("Wildcard:     {}", fmt_ip(s.wildcard)),
            format!("Broadcast:    {}", fmt_ip(s.broadcast)),
            format!(
                "Host range:   {}",
                match (s.first, s.last) {
                    (Some(f), Some(l)) => format!("{} - {}", fmt_ip(f), fmt_ip(l)),
                    _ => "n/a".into(),
                }
            ),
            format!("Hosts:        {}", s.hosts),
        ];
        for (i, r) in rows.into_iter().enumerate() {
            cmds.push(RenderCmd::Text {
                x: 4,
                y: box_y + 1 + i as u16,
                text: r,
                fg: Some(t.text),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    } else {
        cmds.push(RenderCmd::Text {
            x: 4,
            y: box_y + 2,
            text: "Enter a valid IPv4 address and prefix (0-32)".into(),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
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

fn app_mask(prefix: u32) -> [u8; 4] {
    App::mask_from_prefix(prefix)
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
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
        ("c".into(), "copy CIDR".into()),
        ("esc".into(), "back".into()),
    ]
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("Utilities".into(), "Open subnet calculator".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: hints(),
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
                log::error!("[subnet-calculator] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
