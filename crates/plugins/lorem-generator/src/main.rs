use std::io::{BufRead, BufReader, Write};

use rand::Rng;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

const WORDS: &[&str] = &[
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "ut",
    "labore",
    "et",
    "dolore",
    "magna",
    "aliqua",
    "enim",
    "ad",
    "minim",
    "veniam",
    "quis",
    "nostrud",
    "exercitation",
    "ullamco",
    "laboris",
    "nisi",
    "aliquip",
    "ex",
    "ea",
    "commodo",
    "consequat",
    "duis",
    "aute",
    "irure",
    "reprehenderit",
    "voluptate",
    "velit",
    "esse",
    "cillum",
    "fugiat",
    "nulla",
    "pariatur",
    "excepteur",
    "sint",
    "occaecat",
    "cupidatat",
    "non",
    "proident",
    "sunt",
    "culpa",
    "qui",
    "officia",
    "deserunt",
    "mollit",
    "anim",
    "id",
    "est",
    "laborum",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Paragraphs,
    Sentences,
    Words,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Paragraphs => "Paragraphs",
            Self::Sentences => "Sentences",
            Self::Words => "Words",
        }
    }
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    mode: Mode,
    count: usize,
    text: String,
    status: String,
    history: Vec<String>,
    cursor: usize,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            mode: Mode::Paragraphs,
            count: 3,
            text: String::new(),
            status: "Press g to generate \u{b7} m mode \u{b7} +/- count".into(),
            history: Vec::new(),
            cursor: 0,
        };
        app.generate();
        app.cursor = 0;
        app
    }
}

impl App {
    fn generate(&mut self) {
        let mut rng = rand::thread_rng();
        self.text = match self.mode {
            Mode::Paragraphs => {
                let mut paras = Vec::new();
                for _ in 0..self.count {
                    let sentences: Vec<String> = (0..rng.gen_range(4..8))
                        .map(|_| {
                            let word_count = rng.gen_range(8..18);
                            let words: Vec<&str> = (0..word_count)
                                .map(|_| WORDS[rng.gen_range(0..WORDS.len())])
                                .collect();
                            let mut sentence = words.join(" ");
                            sentence.push('.');
                            let first = sentence[..1].to_uppercase();
                            sentence.replace_range(..1, &first);
                            sentence
                        })
                        .collect();
                    paras.push(sentences.join(" "));
                }
                paras.join("\n\n")
            }
            Mode::Sentences => {
                let sentences: Vec<String> = (0..self.count)
                    .map(|_| {
                        let word_count = rng.gen_range(6..16);
                        let words: Vec<&str> = (0..word_count)
                            .map(|_| WORDS[rng.gen_range(0..WORDS.len())])
                            .collect();
                        let mut sentence = words.join(" ");
                        sentence.push('.');
                        let first = sentence[..1].to_uppercase();
                        sentence.replace_range(..1, &first);
                        sentence
                    })
                    .collect();
                sentences.join(" ")
            }
            Mode::Words => {
                let words: Vec<&str> = (0..self.count)
                    .map(|_| WORDS[rng.gen_range(0..WORDS.len())])
                    .collect();
                words.join(" ")
            }
        };
        self.history.insert(0, self.text.clone());
        self.history.truncate(100);
        self.cursor = 0;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char('g') if !modifiers.ctrl => {
                self.generate();
                self.status = format!(
                    "Generated {} {}",
                    self.count,
                    self.mode.label().to_lowercase()
                );
                true
            }
            IpcKey::Char('m') if !modifiers.ctrl => {
                self.mode = match self.mode {
                    Mode::Paragraphs => Mode::Sentences,
                    Mode::Sentences => Mode::Words,
                    Mode::Words => Mode::Paragraphs,
                };
                self.generate();
                self.status = format!("Mode: {}", self.mode.label());
                true
            }
            IpcKey::Tab => {
                self.mode = match self.mode {
                    Mode::Paragraphs => Mode::Sentences,
                    Mode::Sentences => Mode::Words,
                    Mode::Words => Mode::Paragraphs,
                };
                self.generate();
                self.status = format!("Mode: {}", self.mode.label());
                true
            }
            IpcKey::Char('+') | IpcKey::Char('=') => {
                self.count = (self.count + 1).min(100);
                self.generate();
                self.status = format!("Count: {}", self.count);
                true
            }
            IpcKey::Char('-') => {
                self.count = (self.count.saturating_sub(1)).max(1);
                self.generate();
                self.status = format!("Count: {}", self.count);
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_current();
                true
            }
            IpcKey::Enter => {
                self.copy_current();
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

    fn current_text(&self) -> Option<&str> {
        if self.cursor == 0 {
            Some(&self.text)
        } else {
            self.history.get(self.cursor).map(|s| s.as_str())
        }
    }

    fn copy_current(&mut self) {
        let Some(text) = self.current_text() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        match copy_to_clipboard(text) {
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
        title: Some(" Lorem Generator ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let mode_label = match app.mode {
        Mode::Paragraphs => "[Paragraphs]  Sentences  Words",
        Mode::Sentences => "Paragraphs  [Sentences]  Words",
        Mode::Words => "Paragraphs  Sentences  [Words]",
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: format!("Mode   {mode_label}"),
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: format!("Count  {} (+/-)", app.count),
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let out_box_y = 5;
    let out_box_w = w.saturating_sub(4);
    let out_box_h = 9;
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

    if let Some(text) = app.current_text() {
        for (i, line) in text.lines().enumerate() {
            if i as u16 >= out_box_h.saturating_sub(2) {
                break;
            }
            cmds.push(RenderCmd::Text {
                x: 4,
                y: out_box_y + 1 + i as u16,
                text: line.into(),
                fg: Some(t.text),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    }

    let hint_y = h.saturating_sub(1);
    cmds.push(RenderCmd::Text {
        x: 2,
        y: hint_y,
        text: "g generate \u{b7} m/tab mode \u{b7} +/- count \u{b7} c/\u{23ce} copy \u{b7} \u{2191}\u{2193} history \u{b7} esc".into(),
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
    vec![("Utilities".into(), "Open lorem generator".into())]
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
                app.generate();
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
                log::error!("[lorem-generator] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
