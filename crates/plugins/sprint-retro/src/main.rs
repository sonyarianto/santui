use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct RetroItem {
    text: String,
    column: usize,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    items: Vec<RetroItem>,
    cursor: usize,
    current_column: usize,
    editing: bool,
    edit_text: String,
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
            items: vec![
                RetroItem {
                    text: String::from("Good team communication"),
                    column: 0,
                },
                RetroItem {
                    text: String::from("Met sprint goals"),
                    column: 0,
                },
                RetroItem {
                    text: String::from("Too many meetings"),
                    column: 1,
                },
                RetroItem {
                    text: String::from("Scope creep on tasks"),
                    column: 1,
                },
                RetroItem {
                    text: String::from("Reduce WIP limit"),
                    column: 2,
                },
            ],
            cursor: 0,
            current_column: 0,
            editing: false,
            edit_text: String::new(),
            edit_cursor: 0,
            status: String::from(
                "\u{2190}\u{2192} column \u{b7} \u{2191}\u{2193} item \u{b7} a add \u{b7} e edit \u{b7} d delete \u{b7} m move",
            ),
        }
    }
}

impl App {
    fn column_items(&self, col: usize) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.column == col)
            .map(|(i, _)| i)
            .collect()
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.editing {
            return self.handle_edit_key(key);
        }
        match key {
            IpcKey::Left => {
                if self.current_column > 0 {
                    let indices = self.column_items(self.current_column);
                    let rel = indices.iter().position(|&i| i == self.cursor).unwrap_or(0);
                    self.current_column -= 1;
                    let new_indices = self.column_items(self.current_column);
                    self.cursor = new_indices
                        .get(rel)
                        .copied()
                        .unwrap_or(new_indices.last().copied().unwrap_or(self.cursor));
                }
                true
            }
            IpcKey::Right => {
                if self.current_column < 2 {
                    let indices = self.column_items(self.current_column);
                    let rel = indices.iter().position(|&i| i == self.cursor).unwrap_or(0);
                    self.current_column += 1;
                    let new_indices = self.column_items(self.current_column);
                    self.cursor = new_indices
                        .get(rel)
                        .copied()
                        .unwrap_or(new_indices.last().copied().unwrap_or(self.cursor));
                }
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                let indices = self.column_items(self.current_column);
                if let Some(pos) = indices.iter().position(|&i| i == self.cursor) {
                    if pos > 0 {
                        self.cursor = indices[pos - 1];
                    }
                } else if !indices.is_empty() {
                    self.cursor = indices[0];
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let indices = self.column_items(self.current_column);
                if let Some(pos) = indices.iter().position(|&i| i == self.cursor) {
                    if pos + 1 < indices.len() {
                        self.cursor = indices[pos + 1];
                    }
                } else if !indices.is_empty() {
                    self.cursor = indices[0];
                }
                true
            }
            IpcKey::Char('a') | IpcKey::Char('A') => {
                self.editing = true;
                self.edit_text = String::new();
                self.edit_cursor = 0;
                self.status = String::from("Add item: type text and press Enter");
                true
            }
            IpcKey::Char('e') | IpcKey::Char('E') => {
                let indices = self.column_items(self.current_column);
                if indices.contains(&self.cursor) {
                    self.editing = true;
                    self.edit_text = self.items[self.cursor].text.clone();
                    self.edit_cursor = self.edit_text.len();
                    self.status = String::from("Edit item: type text and press Enter");
                }
                true
            }
            IpcKey::Char('d') | IpcKey::Char('D') => {
                let indices = self.column_items(self.current_column);
                if let Some(pos) = indices.iter().position(|&i| i == self.cursor) {
                    self.items.remove(self.cursor);
                    let new_indices = self.column_items(self.current_column);
                    if !new_indices.is_empty() {
                        let new_pos = pos.min(new_indices.len() - 1);
                        self.cursor = new_indices[new_pos];
                    }
                    self.status = String::from("Item deleted");
                }
                true
            }
            IpcKey::Char('m') | IpcKey::Char('M') => {
                let next_col = (self.current_column + 1) % 3;
                let col_indices = self.column_items(self.current_column);
                if let Some(pos_in_col) = col_indices.iter().position(|&i| i == self.cursor) {
                    let item_idx = col_indices[pos_in_col];
                    if item_idx < self.items.len() {
                        self.items[item_idx].column = next_col;
                    }
                    self.current_column = next_col;
                    let new_indices = self.column_items(next_col);
                    self.cursor = new_indices.last().copied().unwrap_or(self.cursor);
                    self.status = format!("Moved to column {}", column_name(next_col));
                }
                true
            }
            IpcKey::Esc => false,
            _ => true,
        }
    }

    fn handle_edit_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Enter => {
                if self.edit_text.is_empty() {
                    self.status = String::from("Text cannot be empty");
                    return true;
                }
                let indices = self.column_items(self.current_column);
                if indices.contains(&self.cursor) {
                    self.items[self.cursor].text = self.edit_text.clone();
                    self.status = String::from("Item updated");
                } else {
                    self.items.push(RetroItem {
                        text: self.edit_text.clone(),
                        column: self.current_column,
                    });
                    self.cursor = self.items.len() - 1;
                    self.status = String::from("Item added");
                }
                self.editing = false;
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.editing = false;
                self.dirty = true;
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                let pos = self.edit_cursor;
                if pos <= self.edit_text.len() {
                    self.edit_text.insert(pos, c);
                    self.edit_cursor += 1;
                }
                true
            }
            IpcKey::Backspace => {
                if self.edit_cursor > 0 {
                    let pos = self.edit_cursor - 1;
                    if pos < self.edit_text.len() {
                        self.edit_text.remove(pos);
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
                if self.edit_cursor < self.edit_text.len() {
                    self.edit_cursor += 1;
                }
                true
            }
            IpcKey::Home => {
                self.edit_cursor = 0;
                true
            }
            IpcKey::End => {
                self.edit_cursor = self.edit_text.len();
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

fn column_name(col: usize) -> &'static str {
    match col {
        0 => "Well",
        1 => "Wrong",
        2 => "Actions",
        _ => "?",
    }
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(60);
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
        title: Some(String::from(" Sprint Retro ")),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    if app.editing {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: format!("> {}", app.edit_text),
            fg: Some(t.highlight),
            bg: None,
            bold: true,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: h.saturating_sub(1),
            text: String::from("enter save \u{b7} esc cancel"),
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
        return cmds;
    }

    let col_w = (w.saturating_sub(4)) / 3;
    let col_labels = ["What went well", "What went wrong", "Action items"];
    let col_colors = [t.success, t.error, t.highlight];

    let max_vis = (h.saturating_sub(5)) as usize;

    for col in 0..3 {
        let x = 2 + col as u16 * col_w;
        cmds.push(RenderCmd::Text {
            x,
            y: 1,
            text: format!("[{}]", col_labels[col]),
            fg: Some(col_colors[col]),
            bg: None,
            bold: true,
            modifiers: 0,
        });

        let indices: Vec<usize> = app
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.column == col)
            .map(|(i, _)| i)
            .collect();

        let scroll = if app.current_column == col {
            let rel = indices.iter().position(|&i| i == app.cursor).unwrap_or(0);
            if rel >= max_vis {
                rel - max_vis + 1
            } else {
                0
            }
        } else {
            0
        };

        for i in 0..max_vis {
            let idx = scroll + i;
            if idx >= indices.len() {
                break;
            }
            let item_idx = indices[idx];
            let item = &app.items[item_idx];
            let selected = item_idx == app.cursor && col == app.current_column;
            let display = if item.text.len() > col_w as usize - 3 {
                format!(
                    "{:.width$}",
                    item.text,
                    width = (col_w as usize).saturating_sub(4)
                )
            } else {
                item.text.clone()
            };
            cmds.push(RenderCmd::Text {
                x,
                y: 3 + i as u16,
                text: if selected {
                    format!("> {}", display)
                } else {
                    format!("  {}", display)
                },
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
        text: format!(
            "col:{}/{} items:{}",
            column_name(app.current_column),
            app.current_column + 1,
            app.column_items(app.current_column).len()
        ),
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

fn palette_commands() -> serde_json::Value {
    serde_json::json!([("Retro".to_string(), "Open Sprint Retro".to_string())])
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
                log::error!("[sprint-retro] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
