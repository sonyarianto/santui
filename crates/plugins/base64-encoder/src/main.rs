use std::io::{BufRead, BufReader};

use base64::{engine::general_purpose, Engine as _};
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Encode,
    Decode,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Encode => "Encode",
            Self::Decode => "Decode",
        }
    }
}

#[derive(Debug, Clone)]
struct Entry {
    input: String,
    output: String,
    mode: Mode,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    mode: Mode,
    input: String,
    output: String,
    status: String,
    history: Vec<Entry>,
    cursor: usize,
    clipboard: Option<arboard::Clipboard>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            mode: Mode::Encode,
            input: String::new(),
            output: String::new(),
            status: String::new(),
            history: Vec::new(),
            cursor: 0,
            clipboard: arboard::Clipboard::new().ok(),
        }
    }
}

impl App {
    fn process(&mut self) {
        self.output = match self.mode {
            Mode::Encode => general_purpose::STANDARD.encode(self.input.as_bytes()),
            Mode::Decode => general_purpose::STANDARD
                .decode(self.input.as_bytes())
                .map(|v| String::from_utf8_lossy(&v).to_string())
                .unwrap_or_else(|_| "(invalid base64)".to_string()),
        };
    }

    fn add_to_history(&mut self) {
        if self.input.is_empty() {
            return;
        }
        self.history.insert(
            0,
            Entry {
                input: self.input.clone(),
                output: self.output.clone(),
                mode: self.mode,
            },
        );
        self.history.truncate(100);
        self.cursor = 0;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char('m') if !modifiers.ctrl => {
                self.mode = match self.mode {
                    Mode::Encode => Mode::Decode,
                    Mode::Decode => Mode::Encode,
                };
                self.process();
                self.status = format!("Mode: {}", self.mode.label());
                true
            }
            IpcKey::Tab => {
                self.mode = match self.mode {
                    Mode::Encode => Mode::Decode,
                    Mode::Decode => Mode::Encode,
                };
                self.process();
                self.status = format!("Mode: {}", self.mode.label());
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_output();
                true
            }
            IpcKey::Enter => {
                if !self.input.is_empty() {
                    self.add_to_history();
                    self.copy_output();
                }
                true
            }
            IpcKey::Backspace => {
                self.input.pop();
                self.process();
                self.status.clear();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.input.push(c);
                self.process();
                self.status.clear();
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

    fn copy_output(&mut self) {
        if self.output.is_empty() || self.output == "(invalid base64)" {
            return;
        }
        match self.clipboard.as_mut() {
            Some(clip) => match clip.set_text(self.output.clone()) {
                Ok(()) => self.status = "Copied to clipboard".into(),
                Err(e) => self.status = format!("Clipboard error: {e}"),
            },
            None => self.status = "Clipboard error: unable to open clipboard".into(),
        }
    }

    fn hints(&self) -> Vec<(String, String)> {
        vec![
            ("type".into(), "input".into()),
            ("m/tab".into(), "mode".into()),
            ("enter".into(), "copy + save".into()),
            ("c".into(), "copy".into()),
            ("↑↓".into(), "history".into()),
            ("esc".into(), "back".into()),
        ]
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
    let w = app.area.w.max(50);
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
        title: Some(" Base64 Encoder ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let mode_label = if app.mode == Mode::Encode {
        "[Encode]  Decode"
    } else {
        "Encode  [Decode]"
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: format!("Mode  {mode_label}"),
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: "Input".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    let input_display = if app.input.is_empty() {
        "(type here)".into()
    } else {
        app.input.clone()
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 4,
        text: input_display,
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let out_header_y = 6;
    cmds.push(RenderCmd::Text {
        x: 2,
        y: out_header_y,
        text: format!("Output ({})", app.mode.label()),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let out_text_y = out_header_y + 1;
    let out_display = if app.output.is_empty() {
        "(empty)".into()
    } else {
        app.output.clone()
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: out_text_y,
        text: out_display,
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let hist_y = out_text_y + 2;
    cmds.push(RenderCmd::Text {
        x: 2,
        y: hist_y,
        text: "History".into(),
        fg: Some(t.text),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let list_start = hist_y + 1;
    let bottom_space = 2;
    let list_h = h.saturating_sub(list_start + bottom_space).max(1);
    let list_w = w.saturating_sub(4);
    let visible = list_h as usize;
    let total = app.history.len();
    let start = if total <= visible {
        0
    } else {
        app.cursor
            .saturating_sub(visible / 2)
            .min(total.saturating_sub(visible))
    };

    let items: Vec<String> = app
        .history
        .iter()
        .skip(start)
        .take(visible)
        .map(|e| {
            let mode_str = if e.mode == Mode::Encode { "enc" } else { "dec" };
            format!("{mode_str}  {:<20}  \u{2192}  {}", e.input, e.output)
        })
        .collect();

    let vis_sel = if app.cursor >= start && app.cursor < start + visible {
        Some(app.cursor - start)
    } else {
        None
    };

    cmds.push(RenderCmd::List {
        x: 2,
        y: list_start,
        w: list_w,
        h: list_h,
        items,
        selected: vis_sel,
        style: TextStyle {
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(t.inverted_text),
            bg: Some(t.highlight),
            bold: true,
            modifiers: 0,
        },
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

fn respond(app: &mut App, hints: Vec<(String, String)>, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints,
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
                log::error!("[base64-encoder] parse error: {e}: {trimmed}");
                false
            }
        };
        let hints = app.hints();
        respond(&mut app, hints, consumed);
        line.clear();
    }
}
