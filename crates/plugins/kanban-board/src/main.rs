use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone)]
struct Card {
    title: String,
}

struct Column {
    name: String,
    cards: Vec<Card>,
}
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    columns: Vec<Column>,
    selected_col: usize,
    selected_card: usize,
    mode: Mode,
    edit_buffer: String,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Browse,
    AddCol,
    AddCard,
    EditCard,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            columns: vec![
                Column { name: "Todo".into(), cards: vec![
                    Card { title: "Set up project".into() },
                ]},
                Column { name: "In Progress".into(), cards: vec![
                    Card { title: "Implement feature".into() },
                ]},
                Column { name: "Done".into(), cards: vec![
                    Card { title: "Plan architecture".into() },
                ]},
            ],
            selected_col: 0,
            selected_card: 0,
            mode: Mode::Browse,
            edit_buffer: String::new(),
            status: "Arrows navigate \u{b7} a add card \u{b7} d delete \u{b7} m move \u{b7} c add col \u{b7} e edit \u{b7} esc".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match self.mode {
            Mode::AddCol | Mode::AddCard | Mode::EditCard => {
                match key {
                    IpcKey::Esc => {
                        self.mode = Mode::Browse;
                    }
                    IpcKey::Char('\n') | IpcKey::Char('\r') => {
                        let text = self.edit_buffer.trim().to_string();
                        if !text.is_empty() {
                            match self.mode {
                                Mode::AddCol => self.columns.push(Column {
                                    name: text,
                                    cards: Vec::new(),
                                }),
                                Mode::AddCard => {
                                    if !self.columns.is_empty() {
                                        self.columns[self.selected_col]
                                            .cards
                                            .push(Card { title: text });
                                    }
                                }
                                Mode::EditCard
                                    if !self.columns.is_empty()
                                        && self.selected_card
                                            < self.columns[self.selected_col].cards.len() =>
                                {
                                    self.columns[self.selected_col].cards[self.selected_card]
                                        .title = text;
                                }
                                _ => {}
                            }
                        }
                        self.mode = Mode::Browse;
                    }
                    IpcKey::Char(c) if !modifiers.ctrl => {
                        if c == '\u{7f}' || c == '\x08' {
                            self.edit_buffer.pop();
                        } else {
                            self.edit_buffer.push(c);
                        }
                    }
                    IpcKey::Backspace => {
                        self.edit_buffer.pop();
                    }
                    _ => {}
                }
                return true;
            }
            Mode::Browse => {}
        }
        match key {
            IpcKey::Esc => false,
            IpcKey::Left => {
                self.selected_col = self.selected_col.saturating_sub(1);
                self.selected_card = 0;
                true
            }
            IpcKey::Right => {
                self.selected_col =
                    (self.selected_col + 1).min(self.columns.len().saturating_sub(1));
                self.selected_card = 0;
                true
            }
            IpcKey::Up => {
                if !self.columns.is_empty() {
                    self.selected_card = self.selected_card.saturating_sub(1);
                }
                true
            }
            IpcKey::Down => {
                if !self.columns.is_empty() {
                    let max = self.columns[self.selected_col]
                        .cards
                        .len()
                        .saturating_sub(1);
                    self.selected_card = (self.selected_card + 1).min(max);
                }
                true
            }
            IpcKey::Char('a') if !modifiers.ctrl => {
                self.edit_buffer.clear();
                self.mode = Mode::AddCard;
                self.status = "Enter card title:".into();
                true
            }
            IpcKey::Char('d') if !modifiers.ctrl => {
                if !self.columns.is_empty()
                    && self.selected_card < self.columns[self.selected_col].cards.len()
                {
                    self.columns[self.selected_col]
                        .cards
                        .remove(self.selected_card);
                }
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.edit_buffer.clear();
                self.mode = Mode::AddCol;
                self.status = "Enter column name:".into();
                true
            }
            IpcKey::Char('m') if !modifiers.ctrl => {
                if self.columns.len() > 1
                    && self.selected_card < self.columns[self.selected_col].cards.len()
                {
                    let card = self.columns[self.selected_col]
                        .cards
                        .remove(self.selected_card);
                    let next_col = (self.selected_col + 1) % self.columns.len();
                    self.columns[next_col].cards.push(card);
                    self.selected_col = next_col;
                    self.selected_card = self.columns[next_col].cards.len().saturating_sub(1);
                }
                true
            }
            IpcKey::Char('e') if !modifiers.ctrl => {
                if !self.columns.is_empty()
                    && self.selected_card < self.columns[self.selected_col].cards.len()
                {
                    self.edit_buffer = self.columns[self.selected_col].cards[self.selected_card]
                        .title
                        .clone();
                    self.mode = Mode::EditCard;
                    self.status = "Edit card title:".into();
                }
                true
            }
            _ => true,
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(60);
        let h = self.area.h.max(14);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": " Kanban Board ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        if self.mode != Mode::Browse {
            cmds.push(json!({"Text": {
                "x": 2, "y": 1, "text": self.status.clone(),
                "fg": t.accent, "bg": null, "bold": true, "modifiers": 0,
            }}));
            cmds.push(json!({"Text": {
                "x": 2, "y": 2, "text": if self.edit_buffer.is_empty() { String::from("(type here)") } else { self.edit_buffer.clone() },
                "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
            }}));
        } else {
            if self.columns.is_empty() {
                cmds.push(json!({"Text": {
                    "x": 2, "y": 1, "text": String::from("Press c to add a column"),
                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
                }}));
            } else {
                let col_w = (w.saturating_sub(4)) / self.columns.len().max(1) as u16;
                let content_y = 2u16;
                for (ci, col) in self.columns.iter().enumerate() {
                    let cx = 2 + ci as u16 * col_w;
                    let is_active = ci == self.selected_col;
                    cmds.push(json!({"Border": {
                        "x": cx, "y": content_y, "w": col_w.saturating_sub(1).max(10), "h": h.saturating_sub(5),
                        "fg": if is_active { t.accent } else { t.border },
                        "borders": BORDER_ALL, "bg": t.background,
                        "title": format!(" {} ({}) ", col.name, col.cards.len()),
                        "title_fg": if is_active { t.accent } else { t.text_muted },
                        "title_dash_fg": t.border, "border_type": null,
                    }}));
                    let max_rows = h.saturating_sub(7) as usize;
                    for (ri, card) in col.cards.iter().enumerate().take(max_rows) {
                        let is_card_selected = is_active && ri == self.selected_card;
                        cmds.push(json!({"Text": {
                            "x": cx + 1, "y": content_y + 1 + ri as u16,
                            "text": format!("{} {}", if is_card_selected { ">" } else { " " }, card.title),
                            "fg": t.text, "bg": if is_card_selected { json!(t.highlight) } else { json!(null) },
                            "bold": is_card_selected, "modifiers": 0,
                        }}));
                    }
                }
            }
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": String::from("\u{2190}\u{2191}\u{2193}\u{2192} \u{b7} a add \u{b7} d del \u{b7} e edit \u{b7} m move \u{b7} c col \u{b7} esc"),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
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

fn palette_commands() -> Value {
    json!([
        {"key": "esc", "hint": "close"},
        {"key": "a", "hint": "add card"},
        {"key": "d", "hint": "delete card"},
        {"key": "m", "hint": "move card"},
        {"key": "c", "hint": "add column"},
        {"key": "e", "hint": "edit card"},
    ])
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = json!({
        "commands": commands_val, "hints": [], "palette_commands": palette_commands(),
        "request": null, "plugin_message": null, "consumed": consumed,
    });
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json}");
    let _ = out.flush();
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
        if reader.read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
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
                log::error!("[kanban-board] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
