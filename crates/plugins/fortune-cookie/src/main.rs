use std::io::{BufRead, BufReader, Write};

use rand::seq::IndexedRandom;
use rand::RngExt;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

struct Quote {
    text: &'static str,
    author: &'static str,
    category: &'static str,
}

const QUOTES: &[Quote] = &[
    Quote { text: "First, solve the problem. Then, write the code.", author: "John Johnson", category: "Programming" },
    Quote { text: "Any fool can write code that a computer can understand. Good programmers write code that humans can understand.", author: "Martin Fowler", category: "Programming" },
    Quote { text: "Simplicity is prerequisite for reliability.", author: "Edsger W. Dijkstra", category: "Programming" },
    Quote { text: "Talk is cheap. Show me the code.", author: "Linus Torvalds", category: "Programming" },
    Quote { text: "The only way to go fast is to go well.", author: "Robert C. Martin", category: "Programming" },
    Quote { text: "Debugging is twice as hard as writing the code in the first place.", author: "Brian Kernighan", category: "Programming" },
    Quote { text: "The best programs are written so that computing machines can perform them quickly.", author: "Alan Perlis", category: "Programming" },
    Quote { text: "Measuring programming progress by lines of code is like measuring aircraft building progress by weight.", author: "Bill Gates", category: "Programming" },
    Quote { text: "The most important property of a program is whether it accomplishes the intention of its user.", author: "C.A.R. Hoare", category: "Programming" },
    Quote { text: "It's not a bug — it's an undocumented feature.", author: "Anonymous", category: "Humor" },
    Quote { text: "I have not failed. I've just found 10,000 ways that won't work.", author: "Thomas Edison", category: "Wisdom" },
    Quote { text: "The best time to plant a tree was 20 years ago. The second best time is now.", author: "Chinese Proverb", category: "Wisdom" },
    Quote { text: "In the middle of every difficulty lies opportunity.", author: "Albert Einstein", category: "Wisdom" },
    Quote { text: "The only true wisdom is in knowing you know nothing.", author: "Socrates", category: "Philosophy" },
    Quote { text: "Science is what we understand well enough to explain to a computer.", author: "Donald Knuth", category: "Science" },
    Quote { text: "If you can't explain it simply, you don't understand it well enough.", author: "Albert Einstein", category: "Science" },
    Quote { text: "Well done is better than well said.", author: "Benjamin Franklin", category: "Proverbs" },
    Quote { text: "Actions speak louder than words.", author: "Proverb", category: "Proverbs" },
    Quote { text: "A journey of a thousand miles begins with a single step.", author: "Lao Tzu", category: "Philosophy" },
    Quote { text: "The impediment to action advances action. What stands in the way becomes the way.", author: "Marcus Aurelius", category: "Philosophy" },
    Quote { text: "The purpose of life is not to be happy. It is to be useful, to be honorable.", author: "Ralph Waldo Emerson", category: "Philosophy" },
    Quote { text: "Stay hungry, stay foolish.", author: "Steve Jobs", category: "Wisdom" },
    Quote { text: "There are only two hard things in Computer Science: cache invalidation and naming things.", author: "Phil Karlton", category: "Programming" },
    Quote { text: "Before software can be reusable it first has to be usable.", author: "Ralph Johnson", category: "Programming" },
    Quote { text: "Make it work, make it right, make it fast.", author: "Kent Beck", category: "Programming" },
    Quote { text: "The best error message is the one that never shows up.", author: "Thomas Fuchs", category: "Programming" },
    Quote { text: "A language that doesn't affect the way you think about programming is not worth knowing.", author: "Alan Perlis", category: "Programming" },
    Quote { text: "The function of good software is to make the complex appear to be simple.", author: "Grady Booch", category: "Programming" },
    Quote { text: "Optimism is an occupational hazard of programming: feedback is the treatment.", author: "Kent Beck", category: "Programming" },
    Quote { text: "If debugging is the process of removing bugs, then programming must be the process of putting them in.", author: "Edsger W. Dijkstra", category: "Humor" },
    Quote { text: "There are two ways to write error-free programs; only the third one works.", author: "Anonymous", category: "Humor" },
    Quote { text: "The computer was born to solve problems that did not exist before.", author: "Bill Gates", category: "Humor" },
    Quote { text: "E pur si muove.", author: "Galileo Galilei", category: "Science" },
    Quote { text: "Equipped with his five senses, man explores the universe around him.", author: "Edwin Hubble", category: "Science" },
    Quote { text: "The important thing is not to stop questioning.", author: "Albert Einstein", category: "Science" },
    Quote { text: "An expert is a person who has made all the mistakes that can be made.", author: "Niels Bohr", category: "Science" },
    Quote { text: "A wise man speaks because he has something to say; a fool because he has to say something.", author: "Plato", category: "Philosophy" },
    Quote { text: "Happiness depends upon ourselves.", author: "Aristotle", category: "Philosophy" },
    Quote { text: "It does not matter how slowly you go as long as you do not stop.", author: "Confucius", category: "Wisdom" },
    Quote { text: "The only limit to our realization of tomorrow will be our doubts of today.", author: "Franklin D. Roosevelt", category: "Wisdom" },
    Quote { text: "When action grows unprofitable, gather information; when information grows unprofitable, sleep.", author: "Ursula K. Le Guin", category: "Wisdom" },
    Quote { text: "Better to remain silent and be thought a fool than to speak and remove all doubt.", author: "Maurice Switzer", category: "Proverbs" },
    Quote { text: "The pen is mightier than the sword.", author: "Edward Bulwer-Lytton", category: "Proverbs" },
    Quote { text: "When in Rome, do as the Romans do.", author: "St. Ambrose", category: "Proverbs" },
    Quote { text: "Birds of a feather flock together.", author: "Proverb", category: "Proverbs" },
];

const CATEGORIES: &[&str] = &[
    "All",
    "Programming",
    "Wisdom",
    "Humor",
    "Science",
    "Philosophy",
    "Proverbs",
];

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    current: usize,
    category_idx: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let mut rng = rand::rng();
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            current: rng.random_range(0..QUOTES.len()),
            category_idx: 0,
            status: "n random \u{b7} c copy \u{b7} esc".into(),
        }
    }
}

impl App {
    fn filtered(&self) -> Vec<usize> {
        if self.category_idx == 0 {
            (0..QUOTES.len()).collect()
        } else {
            let cat = CATEGORIES[self.category_idx];
            QUOTES
                .iter()
                .enumerate()
                .filter(|(_, q)| q.category == cat)
                .map(|(i, _)| i)
                .collect()
        }
    }

    fn random_quote(&mut self) {
        let pool = self.filtered();
        if pool.is_empty() {
            return;
        }
        let mut rng = rand::rng();
        self.current = *pool.choose(&mut rng).unwrap_or(&0);
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.category_idx = (self.category_idx + 1) % CATEGORIES.len();
                self.status = format!("Category: {}", CATEGORIES[self.category_idx]);
                true
            }
            IpcKey::Char('n') if !modifiers.ctrl => {
                self.random_quote();
                self.status = "New quote".into();
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                let q = &QUOTES[self.current];
                let text = format!("\"{}\" — {}", q.text, q.author);
                match copy_to_clipboard(&text) {
                    Ok(()) => self.status = "Copied to clipboard".into(),
                    Err(e) => self.status = format!("Clipboard error: {e}"),
                }
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                let pool = self.filtered();
                let idx = pool.iter().position(|&x| x == self.current).unwrap_or(0);
                if idx > 0 {
                    self.current = pool[idx - 1];
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let pool = self.filtered();
                let idx = pool.iter().position(|&x| x == self.current).unwrap_or(0);
                if idx < pool.len().saturating_sub(1) {
                    self.current = pool[idx + 1];
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
        title: Some(" Fortune Cookie ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let cat_str = CATEGORIES
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i == app.category_idx {
                format!("[{}]", c)
            } else {
                format!(" {}", c)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: cat_str,
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let q = &QUOTES[app.current];
    let box_y = 3;
    let box_w = w.saturating_sub(4);
    let box_h = h.saturating_sub(6).max(4);

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

    let lines = word_wrap(q.text, box_w.saturating_sub(4) as usize);
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
        x: 4,
        y: box_y + box_h.saturating_sub(2),
        text: format!("— {}", q.author),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 4,
        y: box_y + box_h.saturating_sub(1),
        text: format!("Category: {}  #{}", q.category, app.current + 1),
        fg: Some(t.text_muted),
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
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text: "tab category \u{b7} n new \u{b7} \u{2191}\u{2193} browse \u{b7} c copy \u{b7} esc"
            .into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
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
    vec![("Media & Fun".into(), "Open fortune cookie".into())]
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
                log::error!("[fortune-cookie] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
