use std::io::{BufRead, BufReader};
use std::sync::mpsc;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct IssueItem {
    number: u64,
    title: String,
    state: String,
    author: String,
    created_at: String,
    body: String,
}

enum FetchMsg {
    Items(Vec<IssueItem>),
    Error(String),
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    token: String,
    items: Vec<IssueItem>,
    selected: usize,
    page: u32,
    status: String,
    mode: Mode,
    token_buffer: String,
    fetching: bool,
    rx: Option<mpsc::Receiver<FetchMsg>>,
}

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    TokenInput,
    List,
    Detail,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            token: String::new(),
            items: Vec::new(),
            selected: 0,
            page: 1,
            status: String::from("Enter GitHub token and press Enter"),
            mode: Mode::TokenInput,
            token_buffer: String::new(),
            fetching: false,
            rx: None,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match self.mode {
            Mode::TokenInput => match key {
                IpcKey::Esc => false,
                IpcKey::Enter => {
                    let token = self.token_buffer.trim().to_string();
                    if !token.is_empty() {
                        self.token = token;
                        self.mode = Mode::List;
                        self.page = 1;
                        self.fetch_issues();
                    }
                    true
                }
                IpcKey::Backspace => {
                    self.token_buffer.pop();
                    true
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.token_buffer.push(c);
                    true
                }
                _ => true,
            },
            Mode::List => match key {
                IpcKey::Esc => {
                    self.mode = Mode::TokenInput;
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    self.selected = self.selected.saturating_sub(1);
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    let max = self.items.len().saturating_sub(1);
                    self.selected = self.selected.saturating_add(1).min(max);
                    true
                }
                IpcKey::Enter => {
                    if !self.items.is_empty() {
                        self.mode = Mode::Detail;
                    }
                    true
                }
                IpcKey::Right | IpcKey::Char('n') => {
                    if !self.fetching {
                        self.page += 1;
                        self.fetch_issues();
                    }
                    true
                }
                IpcKey::Left | IpcKey::Char('p') => {
                    if self.page > 1 && !self.fetching {
                        self.page -= 1;
                        self.fetch_issues();
                    }
                    true
                }
                _ => true,
            },
            Mode::Detail => match key {
                IpcKey::Esc | IpcKey::Left => {
                    self.mode = Mode::List;
                    true
                }
                _ => true,
            },
        }
    }

    fn fetch_issues(&mut self) {
        let token = self.token.clone();
        let page = self.page;
        self.status = format!("Fetching page {page}...");
        self.fetching = true;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let url = format!(
                "https://api.github.com/repos/rust-lang/rust/issues?state=open&per_page=30&page={page}"
            );
            let result = match ureq::get(&url)
                .header("Authorization", &format!("Bearer {token}"))
                .header("User-Agent", "santui-github-browser")
                .header("Accept", "application/vnd.github.v3+json")
                .call()
            {
                Ok(mut resp) => {
                    let body_str = resp.body_mut().read_to_string().unwrap_or_default();
                    match serde_json::from_str::<Value>(&body_str) {
                        Ok(v) => {
                            let items: Vec<IssueItem> = v
                                .as_array()
                                .map(|arr| {
                                    arr.iter()
                                        .map(|item| IssueItem {
                                            number: item["number"].as_u64().unwrap_or(0),
                                            title: item["title"].as_str().unwrap_or("").to_string(),
                                            state: item["state"].as_str().unwrap_or("").to_string(),
                                            author: item["user"]["login"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string(),
                                            created_at: item["created_at"]
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string(),
                                            body: item["body"].as_str().unwrap_or("").to_string(),
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            Ok(items)
                        }
                        Err(e) => Err(format!("Parse error: {e}")),
                    }
                }
                Err(e) => Err(format!("API error: {e}")),
            };
            match result {
                Ok(items) => {
                    let _ = tx.send(FetchMsg::Items(items));
                }
                Err(e) => {
                    let _ = tx.send(FetchMsg::Error(e));
                }
            }
        });
    }

    fn handle_tick(&mut self) {
        if let Some(ref rx) = self.rx {
            if let Ok(msg) = rx.try_recv() {
                self.fetching = false;
                match msg {
                    FetchMsg::Items(items) => {
                        self.items = items;
                        self.selected = 0;
                        self.status = format!("Page {}  {} items", self.page, self.items.len());
                    }
                    FetchMsg::Error(e) => {
                        self.status = format!("Error: {e}");
                    }
                }
                self.dirty = true;
            }
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        self.cached_commands.clone()
    }
}

fn render_ui(app: &App) -> Vec<Value> {
    let t = &app.theme;
    let w = app.area.w.max(48);
    let h = app.area.h.max(14);
    let mut cmds: Vec<Value> = Vec::new();

    cmds.push(json!({
        String::from("type"): String::from("Rect"),
        String::from("x"): 0, String::from("y"): 0,
        String::from("w"): w, String::from("h"): h,
        String::from("bg"): t.background,
    }));
    cmds.push(json!({
        String::from("type"): String::from("Border"),
        String::from("x"): 0, String::from("y"): 0,
        String::from("w"): w, String::from("h"): h,
        String::from("fg"): t.border,
        String::from("borders"): BORDER_ALL,
        String::from("bg"): t.background_panel,
        String::from("title"): String::from(" GitHub Browser "),
        String::from("title_fg"): t.text,
        String::from("title_dash_fg"): t.border,
    }));

    match app.mode {
        Mode::TokenInput => {
            cmds.push(json!({
                String::from("type"): String::from("Text"),
                String::from("x"): 2, String::from("y"): 2,
                String::from("text"): String::from("Enter GitHub personal access token:"),
                String::from("fg"): t.text,
                String::from("bold"): false,
                String::from("modifiers"): 0,
            }));
            cmds.push(json!({
                String::from("type"): String::from("Border"),
                String::from("x"): 2, String::from("y"): 4,
                String::from("w"): w.saturating_sub(4), String::from("h"): 3,
                String::from("fg"): t.accent,
                String::from("borders"): BORDER_ALL,
                String::from("bg"): t.background,
            }));
            let masked: String = app.token_buffer.chars().map(|_| '*').collect();
            cmds.push(json!({
                String::from("type"): String::from("Text"),
                String::from("x"): 4, String::from("y"): 5,
                String::from("text"): masked,
                String::from("fg"): t.text,
                String::from("bold"): false,
                String::from("modifiers"): 0,
            }));
        }
        Mode::List => {
            for (i, item) in app.items.iter().enumerate() {
                let y = 2 + i as u16;
                if y >= h.saturating_sub(4) {
                    break;
                }
                let is_sel = i == app.selected;
                let prefix = if is_sel { "▸" } else { " " };
                let state_icon = if item.state == "open" { "#" } else { "✓" };
                let line = truncate(
                    &format!("{prefix} {state_icon}{} {}", item.number, item.title),
                    w as usize - 4,
                );
                cmds.push(json!({
                    String::from("type"): String::from("Text"),
                    String::from("x"): 2, String::from("y"): y,
                    String::from("text"): line,
                    String::from("fg"): if is_sel { t.highlight } else { t.text },
                    String::from("bold"): is_sel,
                    String::from("modifiers"): 0,
                }));
            }

            cmds.push(json!({
                String::from("type"): String::from("Text"),
                String::from("x"): 2, String::from("y"): h.saturating_sub(3),
                String::from("text"): format!("Page {}  {} items", app.page, app.items.len()),
                String::from("fg"): t.text_muted,
                String::from("bold"): false,
                String::from("modifiers"): 0,
            }));
        }
        Mode::Detail => {
            if let Some(item) = app.items.get(app.selected) {
                let title = truncate(&format!("#{}  {}", item.number, item.title), w as usize - 4);
                cmds.push(json!({
                    String::from("type"): String::from("Text"),
                    String::from("x"): 2, String::from("y"): 2,
                    String::from("text"): title,
                    String::from("fg"): t.text,
                    String::from("bold"): true,
                    String::from("modifiers"): 0,
                }));
                let meta = format!("@{}  {}  {}", item.author, item.state, item.created_at);
                cmds.push(json!({
                    String::from("type"): String::from("Text"),
                    String::from("x"): 2, String::from("y"): 3,
                    String::from("text"): meta,
                    String::from("fg"): t.text_muted,
                    String::from("bold"): false,
                    String::from("modifiers"): 0,
                }));

                for (i, line) in item.body.lines().enumerate() {
                    let y = 5 + i as u16;
                    if y >= h.saturating_sub(3) {
                        break;
                    }
                    let text = truncate(line, w as usize - 4);
                    cmds.push(json!({
                        String::from("type"): String::from("Text"),
                        String::from("x"): 2, String::from("y"): y,
                        String::from("text"): text,
                        String::from("fg"): t.text,
                        String::from("bold"): false,
                        String::from("modifiers"): 0,
                    }));
                }
            }
        }
    }

    let status_y = h.saturating_sub(2);
    cmds.push(json!({
        String::from("type"): String::from("Text"),
        String::from("x"): 2, String::from("y"): status_y,
        String::from("text"): app.status.clone(),
        String::from("fg"): t.text_muted,
        String::from("bold"): false,
        String::from("modifiers"): 0,
    }));

    cmds
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max && max > 3 {
        format!("{}...", &s[..max - 3])
    } else if s.len() > max {
        s.chars().take(max).collect()
    } else {
        s.to_string()
    }
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
        ("enter".into(), "confirm/detail".into()),
        ("←/→".into(), "page".into()),
        ("esc".into(), "back".into()),
    ]
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("Development".into(), "Open GitHub Browser".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app
            .render()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect(),
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
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: Result<HostMsg, _> = serde_json::from_str(&line);
                match msg {
                    Ok(HostMsg::Init { theme, area, .. }) => {
                        app.theme = theme;
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::Resize { area }) => {
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::ThemeChange { theme }) => {
                        app.theme = theme;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::Key { key, modifiers }) => {
                        let consumed = app.handle_key(key, modifiers);
                        respond(&mut app, consumed);
                    }
                    Ok(HostMsg::Tick) => {
                        app.handle_tick();
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::PaletteCommand { .. }) => {
                        app.dirty = true;
                        respond(&mut app, true);
                    }
                    Ok(HostMsg::Shutdown) => break,
                    Ok(_) => {
                        respond(&mut app, false);
                    }
                    Err(e) => {
                        log::error!("[github-browser] parse error: {e}: {line}");
                        respond(&mut app, false);
                    }
                }
            }
        }
    }
}
