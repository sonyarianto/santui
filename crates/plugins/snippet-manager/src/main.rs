use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct Snippet {
    title: String,
    language: String,
    code: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Screen {
    List,
    Detail,
    Edit,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    snippets: Vec<Snippet>,
    cursor: usize,
    screen: Screen,
    edit_snippet: Snippet,
    edit_field: u8,
    edit_cursor: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            snippets: vec![
                Snippet {
                    title: String::from("Hello World"),
                    language: String::from("rust"),
                    code: String::from("fn main() {\n    println!(\"Hello, world!\");\n}"),
                },
                Snippet {
                    title: String::from("Fibonacci"),
                    language: String::from("python"),
                    code: String::from("def fib(n):\n    a, b = 0, 1\n    for _ in range(n):\n        print(a)\n        a, b = b, a + b"),
                },
            ],
            cursor: 0,
            screen: Screen::List,
            edit_snippet: Snippet {
                title: String::new(),
                language: String::new(),
                code: String::new(),
            },
            edit_field: 0,
            edit_cursor: 0,
            status: String::from("2 snippets. n=new, d=delete, enter=view"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match self.screen {
            Screen::List => self.handle_list_key(key),
            Screen::Detail => self.handle_detail_key(key),
            Screen::Edit => self.handle_edit_key(key),
        }
    }

    fn handle_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.snippets.len().saturating_sub(1);
                self.cursor = self.cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Enter => {
                if !self.snippets.is_empty() {
                    self.screen = Screen::Detail;
                    self.status = String::new();
                }
                true
            }
            IpcKey::Char('n') | IpcKey::Char('N') => {
                self.edit_snippet = Snippet {
                    title: String::new(),
                    language: String::new(),
                    code: String::new(),
                };
                self.edit_field = 0;
                self.edit_cursor = 0;
                self.screen = Screen::Edit;
                self.status = String::from("New snippet");
                true
            }
            IpcKey::Char('d') | IpcKey::Char('D') => {
                if self.cursor < self.snippets.len() {
                    let name = self.snippets[self.cursor].title.clone();
                    self.snippets.remove(self.cursor);
                    if self.cursor >= self.snippets.len() && self.cursor > 0 {
                        self.cursor -= 1;
                    }
                    self.status = format!("Deleted {}", name);
                }
                true
            }
            IpcKey::Esc => false,
            _ => true,
        }
    }

    fn handle_detail_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Esc | IpcKey::Char('q') => {
                self.screen = Screen::List;
                true
            }
            IpcKey::Char('e') | IpcKey::Char('E') => {
                if let Some(s) = self.snippets.get(self.cursor).cloned() {
                    self.edit_snippet = s;
                    self.edit_field = 0;
                    self.edit_cursor = 0;
                    self.screen = Screen::Edit;
                    self.status = String::from("Editing snippet");
                }
                true
            }
            IpcKey::Char('d') | IpcKey::Char('D') => {
                if self.cursor < self.snippets.len() {
                    let name = self.snippets[self.cursor].title.clone();
                    self.snippets.remove(self.cursor);
                    if self.cursor >= self.snippets.len() && self.cursor > 0 {
                        self.cursor -= 1;
                    }
                    self.screen = Screen::List;
                    self.status = format!("Deleted {}", name);
                }
                true
            }
            _ => true,
        }
    }

    fn handle_edit_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Tab => {
                self.edit_field = (self.edit_field + 1) % 3;
                self.edit_cursor = 0;
                true
            }
            IpcKey::Esc => {
                self.screen = if self.cursor < self.snippets.len() {
                    Screen::Detail
                } else {
                    Screen::List
                };
                true
            }
            IpcKey::Enter => {
                if self.edit_field == 2 {
                    let snippet = self.edit_snippet.clone();
                    if snippet.title.is_empty() {
                        self.status = String::from("Title cannot be empty");
                        return true;
                    }
                    if self.cursor < self.snippets.len() {
                        self.snippets[self.cursor] = snippet;
                        self.status = String::from("Snippet updated");
                    } else {
                        self.snippets.push(snippet);
                        self.status = String::from("Snippet added");
                    }
                    self.screen = Screen::List;
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                let field = match self.edit_field {
                    0 => &mut self.edit_snippet.title,
                    1 => &mut self.edit_snippet.language,
                    2 => &mut self.edit_snippet.code,
                    _ => return true,
                };
                let pos = self.edit_cursor;
                if pos <= field.len() {
                    field.insert(pos, c);
                    self.edit_cursor += 1;
                }
                true
            }
            IpcKey::Backspace => {
                let field = match self.edit_field {
                    0 => &mut self.edit_snippet.title,
                    1 => &mut self.edit_snippet.language,
                    2 => &mut self.edit_snippet.code,
                    _ => return true,
                };
                if self.edit_cursor > 0 {
                    let pos = self.edit_cursor - 1;
                    if pos < field.len() {
                        field.remove(pos);
                    }
                    self.edit_cursor -= 1;
                }
                true
            }
            IpcKey::Left => {
                self.edit_cursor = self.edit_cursor.saturating_sub(1);
                true
            }
            IpcKey::Right => {
                let field = match self.edit_field {
                    0 => &self.edit_snippet.title,
                    1 => &self.edit_snippet.language,
                    2 => &self.edit_snippet.code,
                    _ => return true,
                };
                if self.edit_cursor < field.len() {
                    self.edit_cursor += 1;
                }
                true
            }
            IpcKey::Home => {
                self.edit_cursor = 0;
                true
            }
            IpcKey::End => {
                let field = match self.edit_field {
                    0 => &self.edit_snippet.title,
                    1 => &self.edit_snippet.language,
                    2 => &self.edit_snippet.code,
                    _ => return true,
                };
                self.edit_cursor = field.len();
                true
            }
            _ => true,
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
        title: Some(String::from(" Snippet Manager ")),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    match app.screen {
        Screen::List => render_list(app, &mut cmds, t, w, h),
        Screen::Detail => render_detail(app, &mut cmds, t, w, h),
        Screen::Edit => render_edit(app, &mut cmds, t, w, h),
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
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text: match app.screen {
            Screen::List => String::from(
                "\u{2191}\u{2193} navigate \u{b7} n new \u{b7} d delete \u{b7} enter view \u{b7} esc",
            ),
            Screen::Detail => {
                String::from("e edit \u{b7} d delete \u{b7} esc/q back")
            }
            Screen::Edit => {
                String::from("tab field \u{b7} enter save \u{b7} esc cancel")
            }
        },
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

fn render_list(app: &App, cmds: &mut Vec<RenderCmd>, t: &ThemeData, _w: u16, h: u16) {
    let max_vis = (h.saturating_sub(6)) as usize;
    for i in 0..max_vis.min(app.snippets.len()) {
        if i >= app.snippets.len() {
            break;
        }
        let snippet = &app.snippets[i];
        let selected = i == app.cursor;
        let line = format!("  [{:.8}]  {}", snippet.language, snippet.title);
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1 + i as u16,
            text: line,
            fg: Some(if selected { t.highlight } else { t.text }),
            bg: if selected {
                Some(t.background_overlay)
            } else {
                None
            },
            bold: selected,
            modifiers: 0,
        });
    }
    if app.snippets.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 2,
            text: String::from("No snippets. Press n to create one."),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }
}

fn render_detail(app: &App, cmds: &mut Vec<RenderCmd>, t: &ThemeData, _w: u16, h: u16) {
    if let Some(snippet) = app.snippets.get(app.cursor) {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: format!("Title:    {}", snippet.title),
            fg: Some(t.text),
            bg: None,
            bold: true,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 2,
            text: format!("Language: {}", snippet.language),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        let code_h = (h.saturating_sub(7)) as usize;
        let lines: Vec<&str> = snippet.code.lines().collect();
        for (i, line) in lines.iter().enumerate().take(code_h) {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 4 + i as u16,
                text: line.to_string(),
                fg: Some(t.accent),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    }
}

fn render_edit(app: &App, cmds: &mut Vec<RenderCmd>, t: &ThemeData, _w: u16, _h: u16) {
    let fields = [
        ("Title", &app.edit_snippet.title),
        ("Language", &app.edit_snippet.language),
        ("Code", &app.edit_snippet.code),
    ];
    for (i, (label, val)) in fields.iter().enumerate() {
        let active = i == app.edit_field as usize;
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1 + i as u16 * 2,
            text: format!("{}: {}", label, val),
            fg: Some(if active { t.highlight } else { t.text }),
            bg: if active {
                Some(t.background_overlay)
            } else {
                None
            },
            bold: active,
            modifiers: 0,
        });
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

fn palette_commands() -> serde_json::Value {
    serde_json::json!([("Snippets".to_string(), "Open Snippet Manager".to_string())])
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
                log::error!("[snippet-manager] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
