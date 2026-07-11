use std::io::{BufRead, BufReader, Write};

use md5::Md5;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Algorithm {
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

impl Algorithm {
    fn label(self) -> &'static str {
        match self {
            Self::Md5 => "MD5",
            Self::Sha1 => "SHA-1",
            Self::Sha256 => "SHA-256",
            Self::Sha512 => "SHA-512",
        }
    }

    fn hash(self, input: &[u8]) -> String {
        match self {
            Self::Md5 => {
                let mut h = Md5::new();
                h.update(input);
                h.finalize()
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            }
            Self::Sha1 => {
                let mut h = Sha1::new();
                h.update(input);
                h.finalize()
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            }
            Self::Sha256 => {
                let mut h = Sha256::new();
                h.update(input);
                h.finalize()
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            }
            Self::Sha512 => {
                let mut h = Sha512::new();
                h.update(input);
                h.finalize()
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            }
        }
    }
}

const ALGOS: &[Algorithm] = &[
    Algorithm::Md5,
    Algorithm::Sha1,
    Algorithm::Sha256,
    Algorithm::Sha512,
];

#[derive(Debug, Clone)]
struct Entry {
    input: String,
    hash: String,
    algo: Algorithm,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    input: String,
    algo_idx: usize,
    status: String,
    history: Vec<Entry>,
    cursor: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            input: String::new(),
            algo_idx: 2, // SHA-256 default
            status: "Type to hash \u{b7} tab algo \u{b7} c copy \u{b7} enter copy+save".into(),
            history: Vec::new(),
            cursor: 0,
        }
    }
}

impl App {
    fn current_algo(&self) -> Algorithm {
        ALGOS[self.algo_idx]
    }

    fn current_hash(&self) -> String {
        if self.input.is_empty() {
            String::new()
        } else {
            self.current_algo().hash(self.input.as_bytes())
        }
    }

    fn add_history(&mut self) {
        let hash = self.current_hash();
        if hash.is_empty() {
            return;
        }
        self.history.insert(
            0,
            Entry {
                input: self.input.clone(),
                hash,
                algo: self.current_algo(),
            },
        );
        self.history.truncate(100);
        self.cursor = 0;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.algo_idx = (self.algo_idx + 1) % ALGOS.len();
                self.status = format!("Algorithm: {}", self.current_algo().label());
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_current();
                true
            }
            IpcKey::Enter => {
                if !self.input.is_empty() {
                    self.add_history();
                    self.copy_current();
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

    fn copy_current(&mut self) {
        let hash = if self.cursor == 0 && !self.history.is_empty() {
            self.current_hash()
        } else {
            self.history
                .get(self.cursor)
                .map(|e| e.hash.clone())
                .unwrap_or_default()
        };
        if hash.is_empty() {
            return;
        }
        match copy_to_clipboard(&hash) {
            Ok(()) => self.status = "Copied to clipboard".into(),
            Err(e) => self.status = format!("Clipboard error: {e}"),
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
        title: Some(" Hash Calculator ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let algo_line: String = ALGOS
        .iter()
        .enumerate()
        .map(|(i, a)| {
            if i == app.algo_idx {
                format!("[{}]", a.label())
            } else {
                format!(" {} ", a.label())
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: format!("Algo  {algo_line}"),
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

    let out_y = 6;
    cmds.push(RenderCmd::Text {
        x: 2,
        y: out_y,
        text: format!("{} Hash", app.current_algo().label()),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let out_box_y = out_y + 1;
    let out_box_w = w.saturating_sub(4);
    let out_box_h = 3;
    cmds.push(RenderCmd::Border {
        x: 2,
        y: out_box_y,
        w: out_box_w,
        h: out_box_h,
        fg: t.accent,
        borders: BORDER_ALL,
        bg: Some(t.background),
        title: None,
        title_fg: None,
        title_dash_fg: None,
        border_type: None,
    });

    let hash_display = if app.input.is_empty() {
        "(empty)".into()
    } else {
        app.current_hash()
    };
    cmds.push(RenderCmd::Text {
        x: 4,
        y: out_box_y + 1,
        text: hash_display,
        fg: Some(t.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let hist_y = out_box_y + out_box_h + 1;
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
    let bottom_space = 3;
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
        .map(|e| format!("{:<8}  {:<30}  {}", e.algo.label(), e.input, e.hash))
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
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text: "type to hash \u{b7} tab algo \u{b7} enter copy+save \u{b7} c copy \u{b7} \u{2191}\u{2193} history \u{b7} esc".into(),
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

fn palette_commands() -> Vec<(String, String)> {
    vec![("Utilities".into(), "Open hash calculator".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": [],
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
                log::error!("[hash-calculator] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
