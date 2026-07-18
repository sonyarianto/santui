use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use std::io::{BufRead, BufReader};

#[derive(Debug, Clone)]
struct Category {
    name: String,
    budgeted: f64,
    spent: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditMode {
    Name,
    Budgeted,
    Spent,
    None,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    categories: Vec<Category>,
    edit_mode: EditMode,
    input_name: String,
    input_budgeted: String,
    input_spent: String,
    selected: usize,
    adding: bool,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            categories: vec![
                Category {
                    name: String::from("Housing"),
                    budgeted: 1500.0,
                    spent: 1450.0,
                },
                Category {
                    name: String::from("Food"),
                    budgeted: 600.0,
                    spent: 520.0,
                },
                Category {
                    name: String::from("Transport"),
                    budgeted: 300.0,
                    spent: 275.0,
                },
            ],
            edit_mode: EditMode::None,
            input_name: String::new(),
            input_budgeted: String::new(),
            input_spent: String::new(),
            selected: 0,
            adding: false,
            status: String::from("a: add · d: delete · e: edit · Tab: browse · Esc: close"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.adding {
            match key {
                IpcKey::Esc => {
                    self.adding = false;
                    self.edit_mode = EditMode::None;
                    self.input_name.clear();
                    self.input_budgeted.clear();
                    self.input_spent.clear();
                }
                IpcKey::Tab => {
                    self.edit_mode = match self.edit_mode {
                        EditMode::None => EditMode::Name,
                        EditMode::Name => EditMode::Budgeted,
                        EditMode::Budgeted => EditMode::Spent,
                        EditMode::Spent => EditMode::Name,
                    };
                }
                IpcKey::Enter => {
                    if !self.input_name.is_empty() {
                        let budgeted = self.input_budgeted.parse::<f64>().unwrap_or(0.0);
                        let spent = self.input_spent.parse::<f64>().unwrap_or(0.0);
                        self.categories.push(Category {
                            name: self.input_name.trim().to_string(),
                            budgeted,
                            spent,
                        });
                        self.adding = false;
                        self.edit_mode = EditMode::None;
                        self.input_name.clear();
                        self.input_budgeted.clear();
                        self.input_spent.clear();
                        self.selected = self.categories.len().saturating_sub(1);
                        self.status = String::from("Category added");
                    }
                }
                IpcKey::Backspace => match self.edit_mode {
                    EditMode::Name => {
                        self.input_name.pop();
                    }
                    EditMode::Budgeted => {
                        self.input_budgeted.pop();
                    }
                    EditMode::Spent => {
                        self.input_spent.pop();
                    }
                    EditMode::None => {}
                },
                IpcKey::Char(c) if !c.is_control() => match self.edit_mode {
                    EditMode::Name => {
                        self.input_name.push(c);
                    }
                    EditMode::Budgeted => {
                        self.input_budgeted.push(c);
                    }
                    EditMode::Spent => {
                        self.input_spent.push(c);
                    }
                    EditMode::None => {}
                },
                _ => {}
            }
            return true;
        }
        match key {
            IpcKey::Esc => false,
            IpcKey::Up | IpcKey::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.categories.len().saturating_sub(1);
                self.selected = (self.selected + 1).min(max);
                true
            }
            IpcKey::Char('a') => {
                self.adding = true;
                self.edit_mode = EditMode::Name;
                self.status = String::from("Enter category details (Tab to cycle fields)");
                true
            }
            IpcKey::Char('d') => {
                if self.selected < self.categories.len() {
                    self.categories.remove(self.selected);
                    self.selected = self.selected.min(self.categories.len().saturating_sub(1));
                    self.status = String::from("Category deleted");
                }
                true
            }
            _ => true,
        }
    }

    fn total_budgeted(&self) -> f64 {
        self.categories.iter().map(|c| c.budgeted).sum()
    }

    fn total_spent(&self) -> f64 {
        self.categories.iter().map(|c| c.spent).sum()
    }

    fn render(&mut self) -> Vec<RenderCmd> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(40);
        let h = self.area.h.max(12);
        let mut cmds: Vec<RenderCmd> = Vec::new();

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
            title: Some(String::from(" Budget Tracker ")),
            title_fg: Some(t.text),
            title_dash_fg: Some(t.border),
            border_type: None,
        });

        if self.adding {
            let fields = [
                ("Name", &self.input_name, EditMode::Name),
                ("Budgeted", &self.input_budgeted, EditMode::Budgeted),
                ("Spent", &self.input_spent, EditMode::Spent),
            ];
            for (i, (label, value, mode)) in fields.iter().enumerate() {
                let active = *mode == self.edit_mode;
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: 1 + i as u16,
                    text: format!("{} {}: {}", if active { ">" } else { " " }, label, value),
                    fg: if active { Some(t.accent) } else { Some(t.text) },
                    bg: None,
                    bold: active,
                    modifiers: 0,
                });
            }
        } else {
            let header = format!(
                "{:<20} {:>12} {:>12} {:>12}",
                "Category", "Budgeted", "Spent", "Remaining"
            );
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 1,
                text: header,
                fg: Some(t.text_muted),
                bg: None,
                bold: true,
                modifiers: 0,
            });

            let list_y = 2;
            let list_h = h.saturating_sub(6).max(1);
            let items: Vec<String> = self
                .categories
                .iter()
                .map(|c| {
                    let remaining = c.budgeted - c.spent;
                    format!(
                        "{:<20} {:>12.2} {:>12.2} {:>12.2}",
                        c.name, c.budgeted, c.spent, remaining
                    )
                })
                .collect();
            cmds.push(RenderCmd::List {
                x: 2,
                y: list_y,
                w: w.saturating_sub(4),
                h: list_h,
                items,
                selected: if self.selected < self.categories.len() {
                    Some(self.selected)
                } else {
                    None
                },
                style: TextStyle {
                    fg: Some(t.text),
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

            let total_line = format!(
                "Total budgeted: ${:.2}    Total spent: ${:.2}    Remaining: ${:.2}",
                self.total_budgeted(),
                self.total_spent(),
                self.total_budgeted() - self.total_spent()
            );
            cmds.push(RenderCmd::Text {
                x: 2,
                y: list_y + list_h,
                text: total_line,
                fg: Some(t.accent),
                bg: None,
                bold: true,
                modifiers: 0,
            });
        }

        cmds.push(RenderCmd::Text {
            x: 2,
            y: h.saturating_sub(2),
            text: self.status.clone(),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });

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

fn palette_commands() -> Vec<(String, String)> {
    vec![("Plugins".to_string(), "Budget Tracker".to_string())]
}

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("a".to_string(), "add category".to_string()),
        ("d".to_string(), "delete category".to_string()),
    ]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: key_hints(),
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
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[budget-tracker] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
