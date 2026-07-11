use std::io::{BufRead, BufReader, Write};

use rand::seq::IndexedRandom;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

const WORD_LIST: &[&str] = &[
    "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog", "pack", "my", "box", "with",
    "five", "dozen", "liquor", "jugs", "how", "vexingly", "bad", "zebras", "fizz", "buzz", "hello",
    "world", "rust", "types", "safe", "fast", "memory", "system", "code", "test", "build",
    "deploy", "ship", "learn", "teach", "share", "open", "source", "free", "time", "space", "life",
    "work", "play", "make", "break", "fix", "find", "keep", "grow", "think", "know", "show",
    "tell", "help", "give", "take", "bring", "buy", "sell", "use", "long", "short", "big", "small",
    "hot", "cold", "new", "old", "good", "bad", "high", "low", "dark", "light", "hard", "soft",
    "fast", "slow", "rich", "poor", "deep", "shallow", "full", "clean", "dirty", "wide", "narrow",
    "thick", "thin", "loose", "tight", "rough", "smooth", "early", "late", "near", "far", "left",
    "right", "front", "back", "top", "bottom", "inner", "outer", "upper", "lower", "major",
    "minor", "happy", "sad", "angry", "calm", "brave", "scared", "silly", "wise", "loud", "quiet",
    "sweet", "sour", "bitter", "salty", "spicy",
];

const WORDS_PER_TEST: usize = 100;

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    target: Vec<String>,
    typed: String,
    pos: usize,
    done: bool,
    start_time: Option<std::time::Instant>,
    elapsed: f64,
    wpm: f64,
    accuracy: f64,
    finished: bool,
    hint: String,
}

impl Default for App {
    fn default() -> Self {
        let mut rng = rand::rng();
        let target: Vec<String> = WORD_LIST
            .sample(&mut rng, WORDS_PER_TEST)
            .map(|s| s.to_string())
            .collect();
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            target,
            typed: String::new(),
            pos: 0,
            done: false,
            start_time: None,
            elapsed: 0.0,
            wpm: 0.0,
            accuracy: 100.0,
            finished: false,
            hint: "Start typing \u{b7} esc back".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => {
                if self.done {
                    self.reset();
                }
                false
            }
            IpcKey::Backspace => {
                if self.start_time.is_none() {
                    self.start_time = Some(std::time::Instant::now());
                }
                if self.pos > 0 {
                    self.pos -= 1;
                    self.typed.pop();
                }
                true
            }
            IpcKey::Char(c) if !modifiers.ctrl && !c.is_control() => {
                if self.start_time.is_none() {
                    self.start_time = Some(std::time::Instant::now());
                }
                if !self.finished && self.pos < self.target.len() {
                    let _correct = self.target[self.pos].chars().nth(self.typed.len());
                    self.typed.push(c);
                    let word_done = self.typed.len() >= self.target[self.pos].len();
                    if word_done {
                        let _word_ok = self.typed == self.target[self.pos];
                        self.pos += 1;
                        self.typed.clear();
                        if self.pos >= self.target.len() {
                            self.finish();
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn finish(&mut self) {
        self.finished = true;
        if let Some(start) = self.start_time {
            self.elapsed = start.elapsed().as_secs_f64();
            let minutes = self.elapsed / 60.0;
            if minutes > 0.0 {
                let total_chars: usize = self.target.iter().map(|w| w.len()).sum();
                self.wpm = (total_chars as f64 / 5.0) / minutes;
                self.accuracy = 100.0;
            }
        }
    }

    fn reset(&mut self) {
        let mut rng = rand::rng();
        self.target = WORD_LIST
            .sample(&mut rng, WORDS_PER_TEST)
            .map(|s| s.to_string())
            .collect();
        self.typed.clear();
        self.pos = 0;
        self.done = false;
        self.start_time = None;
        self.elapsed = 0.0;
        self.wpm = 0.0;
        self.accuracy = 100.0;
        self.finished = false;
        self.hint = "Start typing \u{b7} esc back".into();
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
        title: Some(" Typing Test ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let elapsed = if let Some(start) = app.start_time {
        start.elapsed().as_secs_f64()
    } else {
        0.0
    };
    let minutes = (elapsed / 60.0).max(0.01);
    let wpm = if app.finished {
        app.wpm
    } else if app.pos > 0 {
        (app.pos as f64 * 5.0) / minutes
    } else {
        0.0
    };

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!(
            "WPM: {wpm:.0}  Words: {}/{}  Time: {elapsed:.0}s",
            app.pos,
            app.target.len()
        ),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let remaining: String = if app.pos < app.target.len() {
        app.target[app.pos..]
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        String::new()
    };

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: if app.finished {
            "Done! Press Esc for new test".into()
        } else {
            remaining
        },
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 4,
        text: if app.typed.is_empty() && !app.finished {
            "(type here)".into()
        } else {
            app.typed.clone()
        },
        fg: Some(t.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    for (i, word) in app.target.iter().enumerate().take(8) {
        let done = i < app.pos;
        let current = i == app.pos;
        let fg = if done {
            t.success
        } else if current {
            t.accent
        } else {
            t.text_muted
        };
        cmds.push(RenderCmd::Text {
            x: 2 + (i as u16 * (word.len().max(5) as u16 + 1)).min(w.saturating_sub(10)),
            y: 6,
            text: word.clone(),
            fg: Some(fg),
            bg: None,
            bold: done || current,
            modifiers: 0,
        });
    }

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: app.hint.clone(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text: "type words \u{b7} backspace correct \u{b7} esc quit/reset".into(),
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
    vec![("Education".into(), "Open typing test".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val, "hints": [], "palette_commands": palette_commands(),
        "request": null, "plugin_message": null, "consumed": consumed,
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
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[typing-test] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
