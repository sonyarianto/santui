use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    data: Vec<u8>,
    offset: usize,
    input: String,
    mode: Mode,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    View,
    Input,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            data: b"Hello, World! This is a hex viewer. Type some text and press Enter to see its hex dump.".to_vec(),
            offset: 0,
            input: String::new(),
            mode: Mode::View,
            status: "Enter hex text \u{b7} \u{2191}\u{2193} scroll \u{b7} c copy \u{b7} esc".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match self.mode {
            Mode::Input => match key {
                IpcKey::Enter => {
                    if !self.input.is_empty() {
                        self.data = self.input.as_bytes().to_vec();
                        self.offset = 0;
                        self.status = format!("Loaded {} bytes", self.data.len());
                    }
                    self.mode = Mode::View;
                    true
                }
                IpcKey::Esc => {
                    self.mode = Mode::View;
                    if self.data.is_empty() {
                        self.data = b"Hello, World!".to_vec();
                        self.status = "Enter hex text \u{b7} \u{2191}\u{2193} scroll \u{b7} c copy \u{b7} esc".into();
                    }
                    true
                }
                IpcKey::Backspace => {
                    self.input.pop();
                    true
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.input.push(c);
                    true
                }
                _ => false,
            },
            Mode::View => match key {
                IpcKey::Char('i') if !modifiers.ctrl => {
                    self.mode = Mode::Input;
                    self.input.clear();
                    self.status = "Enter text to hex dump:".into();
                    true
                }
                IpcKey::Char('c') if !modifiers.ctrl => {
                    let text = hex_string(&self.data);
                    match copy_to_clipboard(&text) {
                        Ok(()) => self.status = "Copied hex string".into(),
                        Err(e) => self.status = format!("Clipboard error: {e}"),
                    }
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    self.offset = self.offset.saturating_sub(16);
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    self.offset = (self.offset + 16).min(self.data.len().saturating_sub(1));
                    true
                }
                IpcKey::PageUp => {
                    self.offset = self.offset.saturating_sub(16 * 10);
                    true
                }
                IpcKey::PageDown => {
                    self.offset = (self.offset + 16 * 10).min(self.data.len().saturating_sub(1));
                    true
                }
                IpcKey::Home => {
                    self.offset = 0;
                    true
                }
                IpcKey::End => {
                    self.offset = self.data.len().saturating_sub(1);
                    true
                }
                IpcKey::Esc => false,
                _ => false,
            },
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

fn hex_string(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(52);
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
        title: Some(" Hex Viewer ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let info = format!(
        "{:>8}  {}",
        format!("Size: {}B", app.data.len()),
        if app.mode == Mode::Input {
            "[INPUT MODE]"
        } else {
            ""
        }
    );
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: info,
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    if app.mode == Mode::Input {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 2,
            text: if app.input.is_empty() {
                "(type text to hex dump)".into()
            } else {
                format!("> {}", app.input)
            },
            fg: Some(t.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        });
    }

    let header_y = if app.mode == Mode::Input { 4 } else { 3 };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: header_y,
        text: "Offset    00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F  ASCII".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let rows = (h - header_y - 3) as usize;
    let bytes_per_row = 16;
    for row in 0..rows {
        let base = app.offset + row * bytes_per_row;
        if base >= app.data.len() {
            break;
        }
        let offset_str = format!("{:08X}", base);
        let mut hex_parts = Vec::new();
        let mut ascii = String::new();
        for i in 0..bytes_per_row {
            let idx = base + i;
            if idx < app.data.len() {
                hex_parts.push(format!("{:02X}", app.data[idx]));
                let c = app.data[idx];
                ascii.push(if c.is_ascii_graphic() || c == b' ' {
                    c as char
                } else {
                    '.'
                });
            } else {
                hex_parts.push("  ".into());
            }
        }
        let hex_cols = hex_parts[..8].join(" ");
        let hex_cols2 = hex_parts[8..].join(" ");
        cmds.push(RenderCmd::Text {
            x: 2,
            y: header_y + 1 + row as u16,
            text: format!("{offset_str}  {hex_cols}  {hex_cols2}  {ascii}"),
            fg: Some(t.text),
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
        ("↑↓".into(), "scroll".into()),
        ("pgUp/pgDn".into(), "page".into()),
        ("i".into(), "input".into()),
        ("c".into(), "copy hex".into()),
        ("home/end".into(), "jump".into()),
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
    vec![("File Management".into(), "Open hex viewer".into())]
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
                log::error!("[hex-viewer] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
