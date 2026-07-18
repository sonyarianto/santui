use std::io::{BufRead, BufReader};

use rand::RngExt;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

struct Card {
    front: &'static str,
    back: &'static str,
}

const CARDS: &[Card] = &[
    Card { front: "What does the `?` operator do in Rust?", back: "Propagates errors: returns `Err` from the function or unwraps `Ok`." },
    Card { front: "What is ownership?", back: "Each value has exactly one owner. When owner goes out of scope, value is dropped." },
    Card { front: "What is a borrow?", back: "A reference to a value without taking ownership. Can be immutable (&T) or mutable (&mut T)." },
    Card { front: "What is the difference between `String` and `&str`?", back: "`String` owns heap-allocated data. `&str` is a borrowed reference to a string slice." },
    Card { front: "What is a trait?", back: "A collection of methods that types can implement. Like interfaces in other languages." },
    Card { front: "What is `impl Trait`?", back: "A concise way to specify an anonymous type that implements a trait, used in argument/return positions." },
    Card { front: "What is a macro?", back: "Code that writes code. Macros operate on the AST and produce code at compile time." },
    Card { front: "What is pattern matching?", back: "A control flow construct that matches values against patterns, commonly used with `match` and `if let`." },
    Card { front: "What is the difference between `Box<T>` and `Rc<T>`?", back: "`Box<T>` provides single-ownership heap allocation. `Rc<T>` provides reference-counted shared ownership." },
    Card { front: "What is a lifetime?", back: "A compile-time annotation that describes how long a reference is valid. Prevents dangling references." },
    Card { front: "What is `#[derive(Debug)]`?", back: "Automatically implements the `Debug` trait, enabling `{:?}` formatting for the type." },
    Card { front: "What is a closure?", back: "An anonymous function that can capture variables from its surrounding scope." },
    Card { front: "What is a module?", back: "Organizes code into namespaces. Defined with `mod` keyword." },
    Card { front: "What is `cargo`?", back: "Rust's build system and package manager. Handles building, testing, and dependency management." },
    Card { front: "What is `panic!`?", back: "A macro that causes the program to abort or unwind, used for unrecoverable errors." },
    Card { front: "What is `Result<T, E>`?", back: "An enum for recoverable errors: `Ok(T)` for success, `Err(E)` for failure." },
    Card { front: "What is `Option<T>`?", back: "An enum representing optional values: `Some(T)` for presence, `None` for absence." },
    Card { front: "What is a struct?", back: "A composite data type that groups related values together with named fields." },
    Card { front: "What is an enum?", back: "A type that can have multiple variants, optionally carrying data." },
    Card { front: "What is the `match` expression?", back: "A control flow construct that compares a value against patterns and executes the matching arm." },
];

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    current: usize,
    flipped: bool,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            current: 0,
            flipped: false,
            status:
                "space flip \u{b7} n random \u{b7} \u{2191}\u{2193} browse \u{b7} c copy \u{b7} esc"
                    .into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char(' ') if !modifiers.ctrl => {
                self.flipped = !self.flipped;
                true
            }
            IpcKey::Char('n') if !modifiers.ctrl => {
                let mut rng = rand::rng();
                self.current = rng.random_range(0..CARDS.len());
                self.flipped = false;
                self.status = "Random card".into();
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                let card = &CARDS[self.current];
                let text = if self.flipped {
                    format!("Q: {}\nA: {}", card.front, card.back)
                } else {
                    card.front.to_string()
                };
                match copy_to_clipboard(&text) {
                    Ok(()) => self.status = "Copied to clipboard".into(),
                    Err(e) => self.status = format!("Clipboard error: {e}"),
                }
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.current = self.current.saturating_sub(1);
                self.flipped = false;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                if self.current < CARDS.len().saturating_sub(1) {
                    self.current += 1;
                }
                self.flipped = false;
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
    let w = app.area.w.max(48);
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
        title: Some(" Flashcards ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!("Card {}/{}", app.current + 1, CARDS.len()),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let card = &CARDS[app.current];
    let content = if app.flipped { card.back } else { card.front };
    let label = if app.flipped { "Back" } else { "Front" };

    let box_y = 3;
    let box_w = w.saturating_sub(4);
    let box_h = h.saturating_sub(6).max(4);

    cmds.push(RenderCmd::Border {
        x: 2,
        y: box_y,
        w: box_w,
        h: box_h,
        fg: if app.flipped { t.success } else { t.accent },
        borders: BORDER_ALL,
        bg: Some(t.background),
        title: Some(format!(" {label} ")),
        title_fg: Some(if app.flipped { t.success } else { t.accent }),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let lines = word_wrap(content, box_w.saturating_sub(4) as usize);
    for (i, line) in lines
        .iter()
        .enumerate()
        .take(box_h.saturating_sub(2) as usize)
    {
        cmds.push(RenderCmd::Text {
            x: 4,
            y: box_y + 1 + i as u16,
            text: line.clone(),
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
        ("space".into(), "flip".into()),
        ("n".into(), "random".into()),
        ("↑↓".into(), "browse".into()),
        ("c".into(), "copy".into()),
        ("esc".into(), "back".into()),
    ]
}

fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.len() + word.len() + 1 > max_width && !line.is_empty() {
            lines.push(line.clone());
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
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
    vec![("Education".into(), "Open flashcards".into())]
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
                log::error!("[flashcards] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
