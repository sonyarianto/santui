use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct Recipe {
    title: String,
    ingredients: Vec<String>,
    instructions: Vec<String>,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    recipes: Vec<Recipe>,
    selected: usize,
    scroll: u16,
    detail_scroll: u16,
    view_detail: bool,
    filter: String,
    input_mode: bool,
    input_buffer: String,
    input_field: u8,
    input_ingredients: Vec<String>,
    input_instructions: Vec<String>,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            recipes: vec![Recipe {
                title: String::from("Spaghetti Carbonara"),
                ingredients: vec![
                    String::from("400g spaghetti"),
                    String::from("200g guanciale"),
                    String::from("4 eggs"),
                    String::from("100g Pecorino Romano"),
                    String::from("Black pepper"),
                ],
                instructions: vec![
                    String::from("Boil pasta in salted water"),
                    String::from("Fry guanciale until crispy"),
                    String::from("Mix eggs with grated cheese"),
                    String::from("Toss hot pasta with egg mixture"),
                    String::from("Add guanciale and pepper"),
                ],
            }],
            selected: 0,
            scroll: 0,
            detail_scroll: 0,
            view_detail: false,
            filter: String::new(),
            input_mode: false,
            input_buffer: String::new(),
            input_field: 0,
            input_ingredients: Vec::new(),
            input_instructions: Vec::new(),
            status: String::from("Ready"),
        }
    }
}

impl App {
    fn filtered_recipes(&self) -> Vec<usize> {
        if self.filter.is_empty() {
            (0..self.recipes.len()).collect()
        } else {
            let f = self.filter.to_lowercase();
            self.recipes
                .iter()
                .enumerate()
                .filter(|(_, r)| r.title.to_lowercase().contains(&f))
                .map(|(i, _)| i)
                .collect()
        }
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.input_mode {
            match key {
                IpcKey::Esc => {
                    self.input_mode = false;
                    self.input_buffer.clear();
                    self.input_ingredients.clear();
                    self.input_instructions.clear();
                    self.status = String::from("Cancelled");
                    true
                }
                IpcKey::Enter => {
                    if self.input_field == 0 {
                        let title = self.input_buffer.trim().to_string();
                        if !title.is_empty() {
                            self.input_ingredients.clear();
                            self.input_instructions.clear();
                            self.input_field = 1;
                            self.input_buffer.clear();
                            self.status = String::from(
                                "Enter ingredients (one per line, empty line to finish):",
                            );
                        }
                    } else if self.input_field == 1 {
                        let line = self.input_buffer.trim().to_string();
                        if line.is_empty() {
                            if self.input_ingredients.is_empty() {
                                self.status = String::from("Need at least one ingredient");
                            } else {
                                self.input_field = 2;
                                self.input_buffer.clear();
                                self.status = String::from(
                                    "Enter instructions (one per line, empty line to finish):",
                                );
                            }
                        } else {
                            self.input_ingredients.push(line);
                            self.input_buffer.clear();
                        }
                    } else if self.input_field == 2 {
                        let line = self.input_buffer.trim().to_string();
                        if line.is_empty() {
                            if self.input_instructions.is_empty() {
                                self.status = String::from("Need at least one instruction");
                            } else {
                                let new_title = self.input_buffer.clone();
                                let recipe = Recipe {
                                    title: new_title,
                                    ingredients: self.input_ingredients.clone(),
                                    instructions: self.input_instructions.clone(),
                                };
                                self.recipes.push(recipe);
                                self.input_mode = false;
                                self.input_buffer.clear();
                                self.input_ingredients.clear();
                                self.input_instructions.clear();
                                self.input_field = 0;
                                self.status = String::from("Recipe added");
                            }
                        } else {
                            self.input_instructions.push(line);
                            self.input_buffer.clear();
                        }
                    }
                    true
                }
                IpcKey::Backspace => {
                    self.input_buffer.pop();
                    true
                }
                IpcKey::Char(c) => {
                    self.input_buffer.push(c);
                    true
                }
                _ => true,
            }
        } else if self.view_detail {
            match key {
                IpcKey::Esc => {
                    self.view_detail = false;
                    self.detail_scroll = 0;
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                    true
                }
                _ => true,
            }
        } else {
            match key {
                IpcKey::Char('a') => {
                    self.input_mode = true;
                    self.input_field = 0;
                    self.input_buffer.clear();
                    self.input_ingredients.clear();
                    self.input_instructions.clear();
                    self.status = String::from("Enter recipe title:");
                    true
                }
                IpcKey::Char('/') => {
                    self.input_mode = true;
                    self.input_field = 3;
                    self.input_buffer.clear();
                    self.status = String::from("Search:");
                    true
                }
                IpcKey::Enter => {
                    let filtered = self.filtered_recipes();
                    if self.selected < filtered.len() {
                        self.view_detail = true;
                        self.detail_scroll = 0;
                    }
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    if self.selected > 0 {
                        self.selected -= 1;
                        self.update_scroll();
                    }
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    let filtered = self.filtered_recipes();
                    if self.selected + 1 < filtered.len() {
                        self.selected += 1;
                        self.update_scroll();
                    }
                    true
                }
                IpcKey::Esc => false,
                _ => true,
            }
        }
    }

    fn update_scroll(&mut self) {
        let list_h = self.area.h.saturating_sub(5) as usize;
        if self.selected < self.scroll as usize {
            self.scroll = self.selected as u16;
        }
        if self.selected >= self.scroll as usize + list_h {
            self.scroll = (self.selected.saturating_sub(list_h).saturating_add(1)) as u16;
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let mut cmds = Vec::new();
        let t = &self.theme;
        let w = self.area.w.max(40);
        let h = self.area.h.max(10);

        cmds.push(json!({"Rect": {
        "x": 0, "y": 0, "w": w, "h": h, "bg": t.background

        }}));
        cmds.push(json!({"Border": {
        "x": 0, "y": 0, "w": w, "h": h, "fg": t.border,
                    "borders": BORDER_ALL, "bg": t.background_panel,
                    "title": " Recipe Manager ",
                    "title_fg": t.text, "title_dash_fg": t.border

        }}));

        if self.input_mode && self.input_field == 3 {
            cmds.push(json!({"Text": {
            "x": 2, "y": 2, "text": String::from("Search: "),
                            "fg": t.text, "bg": null, "bold": false, "modifiers": 0

            }}));
            let cursor_text = format!("{}_", self.input_buffer);
            cmds.push(json!({"Text": {
            "x": 10, "y": 2, "text": cursor_text,
                            "fg": t.accent, "bg": null, "bold": false, "modifiers": 0

            }}));
            let filtered = self.filtered_recipes();
            for (i, idx) in filtered.iter().enumerate() {
                let y = 4 + i as u16;
                if y >= h.saturating_sub(2) {
                    break;
                }
                let is_sel = i == self.selected;
                cmds.push(json!({"Text": {
                "x": 2, "y": y, "text": self.recipes[*idx].title.clone(),
                                    "fg": if is_sel { t.highlight } else { t.text },
                                    "bg": if is_sel { Some(t.background_overlay) } else { None },
                                    "bold": is_sel, "modifiers": 0

                }}));
            }
        } else if self.input_mode {
            let label = match self.input_field {
                0 => String::from("Title: "),
                1 => String::from("Ingredient: "),
                _ => String::from("Instruction: "),
            };
            cmds.push(json!({"Text": {
            "x": 2, "y": 2, "text": label,
                            "fg": t.text, "bg": null, "bold": false, "modifiers": 0

            }}));
            let cursor_text = format!("{}_", self.input_buffer);
            cmds.push(json!({"Text": {
            "x": 2, "y": 3, "text": cursor_text,
                            "fg": t.accent, "bg": null, "bold": false, "modifiers": 0

            }}));
            let mut list_y = 5u16;
            for ing in &self.input_ingredients {
                if list_y >= h.saturating_sub(2) {
                    break;
                }
                cmds.push(json!({"Text": {
                "x": 4, "y": list_y, "text": format!("- {}", ing),
                                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

                }}));
                list_y += 1;
            }
            for instr in &self.input_instructions {
                if list_y >= h.saturating_sub(2) {
                    break;
                }
                cmds.push(json!({"Text": {
"x": 4, "y": list_y, "text": format!("{}. {}", list_y - 5 - self.input_ingredients.len() as u16 + 1, instr),
                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

}}));
                list_y += 1;
            }
        } else if self.view_detail {
            let filtered = self.filtered_recipes();
            if let Some(&idx) = filtered.get(self.selected) {
                let recipe = &self.recipes[idx];
                cmds.push(json!({"Text": {
                "x": 2, "y": 2, "text": recipe.title.clone(),
                                    "fg": t.accent, "bg": null, "bold": true, "modifiers": 0

                }}));
                let content_y = 4u16;
                let max_y = h.saturating_sub(3);
                let mut row = content_y;
                let scroll = self.detail_scroll;
                let mut lines = Vec::new();
                lines.push(String::from("Ingredients:"));
                for ing in &recipe.ingredients {
                    lines.push(format!("  \u{2022} {}", ing));
                }
                lines.push(String::new());
                lines.push(String::from("Instructions:"));
                for (i, instr) in recipe.instructions.iter().enumerate() {
                    lines.push(format!("  {}. {}", i + 1, instr));
                }
                for line in lines.iter().skip(scroll as usize) {
                    if row >= max_y {
                        break;
                    }
                    cmds.push(json!({"Text": {
                    "x": 2, "y": row, "text": line.clone(),
                                            "fg": t.text, "bg": null, "bold": false, "modifiers": 0

                    }}));
                    row += 1;
                }
            }
        } else {
            let filtered = self.filtered_recipes();
            let list_y = 2u16;
            let list_h = h.saturating_sub(4) as usize;
            for (i, &idx) in filtered
                .iter()
                .enumerate()
                .skip(self.scroll as usize)
                .take(list_h)
            {
                let y = list_y + (i as u16).saturating_sub(self.scroll);
                let is_sel = i == self.selected;
                cmds.push(json!({"Text": {
                "x": 2, "y": y, "text": self.recipes[idx].title.clone(),
                                    "fg": if is_sel { t.highlight } else { t.text },
                                    "bg": if is_sel { Some(t.background_overlay) } else { None },
                                    "bold": is_sel, "modifiers": 0

                }}));
            }
        }

        cmds.push(json!({"Text": {
        "x": 2, "y": h.saturating_sub(1),
                    "text": self.status.clone(),
                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

        }}));

        if !self.input_mode && !self.view_detail {
            cmds.push(json!({"Text": {
"x": 2, "y": h,
                "text": String::from("a add  \u{b7} / search  \u{b7} enter view  \u{b7} \u{2191}\u{2193} nav  \u{b7} esc"),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

}}));
        }

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
    vec![]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app
            .render()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect(),
        hints: Vec::new(),
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
            Ok(HostMsg::Tick) => false,
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[recipe-manager] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
