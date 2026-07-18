use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    use sha1::{Digest, Sha1};
    let block_size = 64;
    let mut k = key.to_vec();
    if k.len() > block_size {
        k = Sha1::digest(&k).to_vec();
    }
    k.resize(block_size, 0);
    let mut ipad = vec![0x36u8; block_size];
    let mut opad = vec![0x5Cu8; block_size];
    for i in 0..k.len() {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha1::new();
    inner.update(&ipad);
    inner.update(data);
    let inner_hash = inner.finalize();
    let mut outer = Sha1::new();
    outer.update(&opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

fn totp(secret: &[u8], time_step: u64) -> u32 {
    let time_bytes = time_step.to_be_bytes();
    let mut msg = [0u8; 8];
    msg.copy_from_slice(&time_bytes);
    let hmac = hmac_sha1(secret, &msg);
    let offset = (hmac[19] & 0x0f) as usize;
    let code = u32::from_be_bytes([
        hmac[offset] & 0x7f,
        hmac[offset + 1],
        hmac[offset + 2],
        hmac[offset + 3],
    ]);
    code % 1_000_000
}

fn base32_decode(input: &str) -> Option<Vec<u8>> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let chars: Vec<char> = cleaned.to_uppercase().chars().collect();
    let mut bits = 0u64;
    let mut bits_count = 0;
    let mut out = Vec::new();
    for &c in &chars {
        let val = match c {
            'A'..='Z' => (c as u8) - b'A',
            '2'..='7' => (c as u8) - b'2' + 26,
            '=' => break,
            _ => return None,
        };
        bits = (bits << 5) | val as u64;
        bits_count += 5;
        if bits_count >= 8 {
            bits_count -= 8;
            out.push((bits >> bits_count) as u8);
            bits &= (1 << bits_count) - 1;
        }
    }
    Some(out)
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    secret: String,
    decoded: Vec<u8>,
    code: String,
    time_remaining: u64,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let secret = "JBSWY3DPEHPK3PXP".to_string();
        let decoded = base32_decode(&secret).unwrap_or_default();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let time_step = now / 30;
        let code_val = totp(&decoded, time_step);
        let remaining = 30 - (now % 30);
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            secret,
            decoded,
            code: format!("{:06}", code_val),
            time_remaining: remaining,
            status: "Enter Base32 secret \u{b7} c copy \u{b7} esc".into(),
        }
    }
}

impl App {
    fn update_code(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let time_step = now / 30;
        let remaining = 30 - (now % 30);
        let code_val = totp(&self.decoded, time_step);
        self.code = format!("{:06}", code_val);
        self.time_remaining = remaining;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char('c') if !modifiers.ctrl => {
                match copy_to_clipboard(&self.code) {
                    Ok(()) => self.status = format!("Copied {}.", self.code),
                    Err(e) => self.status = format!("Clipboard error: {e}"),
                }
                true
            }
            IpcKey::Enter => {
                if !self.secret.trim().is_empty() {
                    if let Some(decoded) = base32_decode(self.secret.trim()) {
                        self.decoded = decoded;
                        self.update_code();
                        self.status = "Secret updated \u{b7} next code in 30s".into();
                    } else {
                        self.status = "Invalid Base32 secret".into();
                    }
                }
                true
            }
            IpcKey::Backspace => {
                self.secret.pop();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.secret.push(c);
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

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(44);
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
        title: Some(" TOTP Viewer ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let bar_fill = (app.time_remaining as f64 / 30.0 * (w.saturating_sub(8)) as f64) as u16;
    let bar = "█".repeat(bar_fill as usize);
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: bar,
        fg: Some(if app.time_remaining < 5 {
            t.error
        } else {
            t.success
        }),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: format!("Code: {}", app.code),
        fg: Some(t.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 4,
        text: format!("Expires in: {}s", app.time_remaining),
        fg: Some(if app.time_remaining < 5 {
            t.error
        } else {
            t.text_muted
        }),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 6,
        text: "Secret (Base32):".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 7,
        text: if app.secret.is_empty() {
            "(enter Base32 key)".into()
        } else {
            app.secret.clone()
        },
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

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
        ("enter".into(), "apply".into()),
        ("c".into(), "copy code".into()),
        ("esc".into(), "back".into()),
    ]
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("Security".into(), "Open TOTP viewer".into())]
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
            Ok(HostMsg::Tick) => {
                app.update_code();
                app.dirty = true;
                false
            }
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[totp-viewer] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
