use std::io::{BufRead, BufReader};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    token: String,
    status: String,
    history: Vec<String>,
    cursor: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            token: String::new(),
            status: "Paste JWT \u{b7} enter decode \u{b7} c copy \u{b7} esc".into(),
            history: Vec::new(),
            cursor: 0,
        }
    }
}

impl App {
    fn decode(token: &str) -> Option<(String, String, String)> {
        let parts: Vec<&str> = token.trim().split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let header = decode_segment(parts[0])?;
        let payload = decode_segment(parts[1])?;
        let _sig = parts[2];
        Some((header, payload, parts[2].to_string()))
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char('c') if !modifiers.ctrl => {
                if let Some((_, payload, _)) = Self::decode(&self.token) {
                    match copy_to_clipboard(&payload) {
                        Ok(()) => self.status = "Copied payload JSON".into(),
                        Err(e) => self.status = format!("Clipboard error: {e}"),
                    }
                }
                true
            }
            IpcKey::Enter => {
                if !self.token.trim().is_empty() {
                    match Self::decode(&self.token) {
                        Some(_) => {
                            self.history.insert(0, self.token.clone());
                            self.history.truncate(100);
                            self.cursor = 0;
                            self.status = "Decoded".into();
                        }
                        None => self.status = "Invalid JWT (need 3 dot-separated parts)".into(),
                    }
                }
                true
            }
            IpcKey::Backspace => {
                self.token.pop();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.token.push(c);
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                if self.cursor < self.history.len().saturating_sub(1) {
                    self.cursor += 1;
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn current_token(&self) -> &str {
        if self.cursor == 0 {
            &self.token
        } else {
            self.history
                .get(self.cursor)
                .map(|s| s.as_str())
                .unwrap_or("")
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

fn decode_segment(seg: &str) -> Option<String> {
    let bytes = URL_SAFE_NO_PAD.decode(seg).ok()?;
    String::from_utf8(bytes).ok()
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(46);
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
        title: Some(" JWT Inspector ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: "Token".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    let token = app.current_token();
    let token_display = if token.is_empty() {
        "(paste here)".into()
    } else {
        token.to_string()
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: token_display,
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let box_y = 5;
    let box_w = w.saturating_sub(4);
    let box_h = h.saturating_sub(7).max(6);
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

    if let Some((header, payload, _)) = App::decode(token) {
        let mut line_no = 0u16;
        for (label, json) in [("HEADER", header), ("PAYLOAD", payload)] {
            cmds.push(RenderCmd::Text {
                x: 4,
                y: box_y + 1 + line_no,
                text: format!("── {label} ──"),
                fg: Some(t.text_muted),
                bg: None,
                bold: true,
                modifiers: 0,
            });
            line_no += 1;
            for l in json
                .lines()
                .take((box_h.saturating_sub(line_no + 1)) as usize)
            {
                cmds.push(RenderCmd::Text {
                    x: 4,
                    y: box_y + 1 + line_no,
                    text: l.to_string(),
                    fg: Some(t.accent),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
                line_no += 1;
            }
            line_no += 1;
        }
    } else {
        cmds.push(RenderCmd::Text {
            x: 4,
            y: box_y + 1,
            text: "(invalid or empty)".into(),
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

fn hints() -> Vec<(String, String)> {
    vec![
        ("paste".into(), "token".into()),
        ("enter".into(), "decode".into()),
        ("c".into(), "copy payload".into()),
        ("↑↓".into(), "history".into()),
        ("esc".into(), "back".into()),
    ]
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

fn palette_commands() -> Vec<(String, String)> {
    vec![("Plugins".into(), "Open JWT inspector".into())]
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
                log::error!("[jwt-inspector] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
