mod state;
mod ui;

use std::io::{BufRead, BufReader};

use chrono::Local;
use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};

use state::{FocusField, HabitState, Screen, COLOR_PRESETS};
use ui::render_ui;

struct App {
    state: HabitState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    confirming_delete: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: HabitState::default(),
            theme: ThemeData {
                text: [220; 3],
                text_muted: [140; 3],
                accent: [180; 3],
                highlight: [220; 3],
                logo: [255; 3],
                background: [0; 3],
                background_panel: [20; 3],
                background_overlay: [10; 3],
                border: [150; 3],
                success: [80; 3],
                error: [255; 3],
                inverted_text: [255; 3],
            },
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet {
                key: "habit-tracker".into(),
            }),
            confirming_delete: false,
        }
    }
}

impl App {
    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.dirty = true;
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        match self.state.screen.clone() {
            Screen::Overview => self.handle_overview_key(key),
            Screen::Detail => self.handle_detail_key(key),
            Screen::Editor => self.handle_editor_key(key),
            Screen::DayDetail => self.handle_day_detail_key(key),
        }
    }

    fn handle_overview_key(&mut self, key: IpcKey) -> bool {
        if self.state.note_editing {
            return self.handle_note_edit_key(key);
        }

        if self.state.filter_mode {
            match key {
                IpcKey::Esc => {
                    self.state.filter_mode = false;
                    self.state.filter_query.clear();
                    self.state.dirty = true;
                    true
                }
                IpcKey::Backspace => {
                    self.state.filter_query.pop();
                    self.state.cursor = self
                        .state
                        .cursor
                        .min(self.state.filtered_habits().len().saturating_sub(1));
                    self.state.dirty = true;
                    true
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.state.filter_query.push(c);
                    self.state.cursor = 0;
                    self.state.dirty = true;
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    self.state.cursor = self.state.cursor.saturating_sub(1);
                    self.state.dirty = true;
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    let max = self.state.filtered_habits().len().saturating_sub(1);
                    self.state.cursor = self.state.cursor.min(max).saturating_add(1).min(max);
                    self.state.dirty = true;
                    true
                }
                IpcKey::Enter => {
                    self.open_detail();
                    true
                }
                _ => true,
            }
        } else {
            let habit_count = self.state.filtered_habits().len();
            match key {
                IpcKey::Char('n') => {
                    self.open_editor_new();
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    self.state.cursor = self.state.cursor.saturating_sub(1);
                    self.confirming_delete = false;
                    self.state.dirty = true;
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    let max = habit_count.saturating_sub(1);
                    self.state.cursor = self.state.cursor.min(max).saturating_add(1).min(max);
                    self.confirming_delete = false;
                    self.state.dirty = true;
                    true
                }
                IpcKey::Enter => {
                    self.open_detail();
                    true
                }
                IpcKey::Char('d') => {
                    if self.confirming_delete {
                        self.delete_selected_habit();
                        self.confirming_delete = false;
                    } else if !self.state.filtered_habits().is_empty() {
                        self.confirming_delete = true;
                        self.state.dirty = true;
                    }
                    true
                }
                IpcKey::Char('/') => {
                    self.state.filter_mode = true;
                    self.state.filter_query.clear();
                    self.state.dirty = true;
                    true
                }
                IpcKey::Esc => false,
                _ => false,
            }
        }
    }

    fn handle_detail_key(&mut self, key: IpcKey) -> bool {
        if self.state.note_editing {
            return self.handle_note_edit_key(key);
        }

        let weeks = self.state.heatmap_weeks(
            self.state
                .data
                .habits
                .get(self.state.detail_habit_idx)
                .map(|h| h.id.as_str())
                .unwrap_or(""),
        );
        let max_row = weeks.len().saturating_sub(1);

        match key {
            IpcKey::Left | IpcKey::Char('h') => {
                if self.state.heatmap_col > 0 {
                    self.state.heatmap_col -= 1;
                } else if self.state.heatmap_row > 0 {
                    self.state.heatmap_row -= 1;
                    self.state.heatmap_col = 6;
                }
                self.state.dirty = true;
                true
            }
            IpcKey::Right | IpcKey::Char('l') => {
                if self.state.heatmap_col < 6 {
                    self.state.heatmap_col += 1;
                } else if self.state.heatmap_row < max_row {
                    self.state.heatmap_row += 1;
                    self.state.heatmap_col = 0;
                }
                self.state.dirty = true;
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.heatmap_row = self.state.heatmap_row.saturating_sub(1);
                self.adjust_heatmap_scroll();
                self.state.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.state.heatmap_row = self
                    .state
                    .heatmap_row
                    .min(max_row)
                    .saturating_add(1)
                    .min(max_row);
                self.adjust_heatmap_scroll();
                self.state.dirty = true;
                true
            }
            IpcKey::Enter => {
                self.toggle_heatmap_day();
                true
            }
            IpcKey::Char('e') => {
                self.open_editor_for_selected_habit();
                true
            }
            IpcKey::Char('n') => {
                self.start_note_for_heatmap_day();
                true
            }
            IpcKey::Esc => {
                if self.state.note_editing {
                    self.state.note_editing = false;
                    self.state.note_buffer.clear();
                    self.state.dirty = true;
                } else {
                    self.state.screen = Screen::Overview;
                    self.state.dirty = true;
                }
                true
            }
            _ => false,
        }
    }

    fn handle_editor_key(&mut self, key: IpcKey) -> bool {
        if self.state.editing {
            match key {
                IpcKey::Backspace => {
                    self.state.editor_buffer.pop();
                    self.state.dirty = true;
                    true
                }
                IpcKey::Enter => {
                    self.commit_editor_field();
                    self.state.dirty = true;
                    true
                }
                IpcKey::Esc => {
                    self.state.editing = false;
                    self.state.editor_buffer.clear();
                    self.state.dirty = true;
                    true
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.state.editor_buffer.push(c);
                    self.state.dirty = true;
                    true
                }
                _ => true,
            }
        } else {
            match key {
                IpcKey::Up | IpcKey::Char('k') => {
                    self.state.editor_focus = match self.state.editor_focus {
                        FocusField::Name => FocusField::Color,
                        FocusField::Description => FocusField::Name,
                        FocusField::Color => FocusField::Description,
                    };
                    self.state.dirty = true;
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    self.state.editor_focus = match self.state.editor_focus {
                        FocusField::Name => FocusField::Description,
                        FocusField::Description => FocusField::Color,
                        FocusField::Color => FocusField::Name,
                    };
                    self.state.dirty = true;
                    true
                }
                IpcKey::Enter => {
                    match self.state.editor_focus {
                        FocusField::Name | FocusField::Description => {
                            let current = match &self.state.editor_habit {
                                Some(h) => match self.state.editor_focus {
                                    FocusField::Name => h.name.clone(),
                                    FocusField::Description => h.description.clone(),
                                    _ => String::new(),
                                },
                                None => String::new(),
                            };
                            self.state.editor_buffer = current;
                            self.state.editing = true;
                            self.state.dirty = true;
                        }
                        FocusField::Color => {}
                    }
                    true
                }
                IpcKey::Left => {
                    if self.state.editor_focus == FocusField::Color {
                        self.cycle_color_backward();
                        self.state.dirty = true;
                    }
                    true
                }
                IpcKey::Right => {
                    if self.state.editor_focus == FocusField::Color {
                        self.cycle_color_forward();
                        self.state.dirty = true;
                    }
                    true
                }
                IpcKey::Esc => {
                    self.save_habit_and_return();
                    true
                }
                _ => false,
            }
        }
    }

    fn handle_day_detail_key(&mut self, key: IpcKey) -> bool {
        if self.state.note_editing {
            return self.handle_note_edit_key(key);
        }

        let habit_count = self.state.filtered_habits().len();
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.cursor = self.state.cursor.saturating_sub(1);
                self.state.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = habit_count.saturating_sub(1);
                self.state.cursor = self.state.cursor.min(max).saturating_add(1).min(max);
                self.state.dirty = true;
                true
            }
            IpcKey::Enter => {
                self.toggle_day_detail_habit();
                true
            }
            IpcKey::Char('n') => {
                self.start_note_for_day_detail();
                true
            }
            IpcKey::Esc => {
                if self.state.note_editing {
                    self.state.note_editing = false;
                    self.state.note_buffer.clear();
                    self.state.dirty = true;
                } else {
                    self.state.screen = Screen::Detail;
                    self.state.dirty = true;
                }
                true
            }
            _ => false,
        }
    }

    fn handle_note_edit_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Backspace => {
                self.state.note_buffer.pop();
                self.state.dirty = true;
                true
            }
            IpcKey::Enter => {
                self.commit_note();
                self.state.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.note_editing = false;
                self.state.note_buffer.clear();
                self.state.dirty = true;
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.state.note_buffer.push(c);
                self.state.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn open_detail(&mut self) {
        let habits = self.state.filtered_habits();
        if let Some(habit) = habits.get(self.state.cursor) {
            let idx = self
                .state
                .data
                .habits
                .iter()
                .position(|h| h.id == habit.id)
                .unwrap_or(0);
            self.state.detail_habit_idx = idx;
            self.state.screen = Screen::Detail;
            self.state.heatmap_row = 16;
            self.state.heatmap_col = 6;
            self.state.heatmap_scroll = 0;
            self.confirming_delete = false;
            self.state.filter_mode = false;
            self.state.filter_query.clear();
            self.state.dirty = true;
        }
    }

    fn open_editor_new(&mut self) {
        self.state.editor_habit = Some(state::Habit {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            color: "green".into(),
            created_at: Local::now().format("%Y-%m-%d").to_string(),
            archived: false,
        });
        self.state.editor_focus = FocusField::Name;
        self.state.editor_buffer.clear();
        self.state.editing = false;
        self.state.screen = Screen::Editor;
        self.confirming_delete = false;
        self.state.dirty = true;
    }

    fn open_editor_for_selected_habit(&mut self) {
        let habit = self
            .state
            .data
            .habits
            .get(self.state.detail_habit_idx)
            .cloned();
        self.state.editor_habit = habit;
        self.state.editor_focus = FocusField::Name;
        self.state.editor_buffer.clear();
        self.state.editing = false;
        self.state.screen = Screen::Editor;
        self.state.dirty = true;
    }

    fn delete_selected_habit(&mut self) {
        let habits = self.state.filtered_habits();
        if let Some(habit) = habits.get(self.state.cursor) {
            let id = habit.id.clone();
            self.state.data.habits.retain(|h| h.id != id);
            self.state.data.entries.retain(|e| e.habit_id != id);
            self.state.rebuild_sorted();
            self.state.cursor = self
                .state
                .cursor
                .min(self.state.filtered_habits().len().saturating_sub(1));
            self.schedule_db_save();
            self.state.dirty = true;
        }
    }

    fn toggle_heatmap_day(&mut self) {
        let habit_id = self
            .state
            .data
            .habits
            .get(self.state.detail_habit_idx)
            .map(|h| h.id.clone())
            .unwrap_or_default();
        let weeks = self.state.heatmap_weeks(&habit_id);
        if let Some(week) = weeks.get(self.state.heatmap_row) {
            if let Some((date, _)) = week.get(self.state.heatmap_col) {
                self.state.toggle_entry(&habit_id, date);
                self.schedule_db_save();
                self.state.dirty = true;
            }
        }
    }

    fn toggle_day_detail_habit(&mut self) {
        let habits = self.state.filtered_habits();
        if let Some(habit) = habits.get(self.state.cursor) {
            let date = self.state.day_detail_date.clone();
            let habit_id = habit.id.clone();
            self.state.toggle_entry(&habit_id, &date);
            self.schedule_db_save();
            self.state.dirty = true;
        }
    }

    fn start_note_for_heatmap_day(&mut self) {
        let habit_id = self
            .state
            .data
            .habits
            .get(self.state.detail_habit_idx)
            .map(|h| h.id.clone())
            .unwrap_or_default();
        let weeks = self.state.heatmap_weeks(&habit_id);
        if let Some(week) = weeks.get(self.state.heatmap_row) {
            if let Some((date, _)) = week.get(self.state.heatmap_col) {
                let existing_note = self
                    .state
                    .get_entry(&habit_id, date)
                    .map(|e| e.note.clone())
                    .unwrap_or_default();
                self.state.note_buffer = existing_note;
                self.state.note_editing = true;
                self.state.dirty = true;
            }
        }
    }

    fn start_note_for_day_detail(&mut self) {
        let habits = self.state.filtered_habits();
        if let Some(habit) = habits.get(self.state.cursor) {
            let existing_note = self
                .state
                .get_entry(&habit.id, &self.state.day_detail_date)
                .map(|e| e.note.clone())
                .unwrap_or_default();
            self.state.note_editing = true;
            self.state.note_habit_idx = self.state.cursor;
            self.state.note_buffer = existing_note;
            self.state.dirty = true;
        }
    }

    fn commit_note(&mut self) {
        let habits = self.state.filtered_habits();
        let habit_id = if self.state.screen == Screen::Detail {
            self.state
                .data
                .habits
                .get(self.state.detail_habit_idx)
                .map(|h| h.id.clone())
        } else {
            habits.get(self.state.note_habit_idx).map(|h| h.id.clone())
        };

        let date_str = if self.state.screen == Screen::Detail {
            let habit_id_ref = self
                .state
                .data
                .habits
                .get(self.state.detail_habit_idx)
                .map(|h| h.id.as_str())
                .unwrap_or("");
            let weeks = self.state.heatmap_weeks(habit_id_ref);
            weeks
                .get(self.state.heatmap_row)
                .and_then(|w| w.get(self.state.heatmap_col))
                .map(|(d, _)| d.clone())
                .unwrap_or_default()
        } else {
            self.state.day_detail_date.clone()
        };

        if let Some(hid) = habit_id {
            let mut found = false;
            for entry in self.state.data.entries.iter_mut() {
                if entry.habit_id == hid && entry.date == date_str {
                    entry.note = self.state.note_buffer.clone();
                    found = true;
                    break;
                }
            }
            if !found && !self.state.note_buffer.is_empty() {
                self.state.data.entries.push(state::HabitEntry {
                    date: date_str,
                    habit_id: hid,
                    completed: false,
                    note: self.state.note_buffer.clone(),
                });
            }
        }

        self.state.note_editing = false;
        self.state.note_buffer.clear();
        self.schedule_db_save();
        self.state.dirty = true;
    }

    fn commit_editor_field(&mut self) {
        let value = self.state.editor_buffer.clone();
        if let Some(ref mut habit) = self.state.editor_habit {
            match self.state.editor_focus {
                FocusField::Name => habit.name = value,
                FocusField::Description => habit.description = value,
                FocusField::Color => {}
            }
        }
        self.state.editing = false;
        self.state.editor_buffer.clear();
    }

    fn save_habit_and_return(&mut self) {
        if let Some(ref habit) = self.state.editor_habit {
            if habit.name.is_empty() {
                self.state.screen = Screen::Overview;
                self.state.editor_habit = None;
                self.state.dirty = true;
                return;
            }

            let id = if habit.id.is_empty() {
                format!(
                    "{}-{}",
                    habit.name.to_lowercase().replace(' ', "-"),
                    Local::now().format("%Y-%m-%d")
                )
            } else {
                habit.id.clone()
            };

            let updated_habit = state::Habit {
                id,
                name: habit.name.clone(),
                description: habit.description.clone(),
                color: habit.color.clone(),
                created_at: habit.created_at.clone(),
                archived: habit.archived,
            };

            let existing_idx = self
                .state
                .data
                .habits
                .iter()
                .position(|h| h.id == updated_habit.id);
            match existing_idx {
                Some(idx) => self.state.data.habits[idx] = updated_habit,
                None => self.state.data.habits.push(updated_habit),
            }

            self.state.rebuild_sorted();
            self.state.screen = Screen::Overview;
            self.state.editor_habit = None;
            self.state.dirty = true;
            self.schedule_db_save();
        }
    }

    fn cycle_color_forward(&mut self) {
        if let Some(ref mut habit) = self.state.editor_habit {
            let current = habit.color.clone();
            let pos = COLOR_PRESETS
                .iter()
                .position(|c| *c == current)
                .unwrap_or(0);
            let next = (pos + 1) % COLOR_PRESETS.len();
            habit.color = COLOR_PRESETS[next].into();
        }
    }

    fn cycle_color_backward(&mut self) {
        if let Some(ref mut habit) = self.state.editor_habit {
            let current = habit.color.clone();
            let pos = COLOR_PRESETS
                .iter()
                .position(|c| *c == current)
                .unwrap_or(0);
            let prev = (pos + COLOR_PRESETS.len() - 1) % COLOR_PRESETS.len();
            habit.color = COLOR_PRESETS[prev].into();
        }
    }

    fn adjust_heatmap_scroll(&mut self) {
        let visible_weeks = ((self.area.w.saturating_sub(4)) / 21).max(1) as usize;
        if self.state.heatmap_row < self.state.heatmap_scroll {
            self.state.heatmap_scroll = self.state.heatmap_row;
        }
        if self.state.heatmap_row >= self.state.heatmap_scroll + visible_weeks {
            self.state.heatmap_scroll = self
                .state
                .heatmap_row
                .saturating_sub(visible_weeks.saturating_sub(1));
        }
    }

    fn handle_tick(&mut self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        if self.state.day_detail_date != today {
            self.state.dirty = true;
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "habit-tracker" {
            if let Some(json) = value {
                if let Ok(data) = serde_json::from_str::<state::HabitData>(&json) {
                    self.state.data = data;
                }
            } else {
                self.state.data = state::HabitData::default();
            }
            self.state.rebuild_sorted();
            self.state.dirty = true;
        }
    }

    fn schedule_db_save(&mut self) {
        self.pending_request = Some(PluginRequest::DbSet {
            key: "habit-tracker".into(),
            value: serde_json::to_string(&self.state.data).unwrap(),
        });
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.state.screen = Screen::Overview;
                self.state.dirty = true;
            }
            1 => {
                self.open_editor_new();
            }
            2 => {
                self.state.screen = Screen::DayDetail;
                self.state.day_detail_date = Local::now().format("%Y-%m-%d").to_string();
                self.state.dirty = true;
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match &self.state.screen {
            Screen::Overview => {
                let mut hints = vec![
                    ("n".into(), "new".into()),
                    ("\u{2191}\u{2193}".into(), "navigate".into()),
                    ("enter".into(), "detail".into()),
                ];
                if self.confirming_delete {
                    hints.push(("d".into(), "confirm delete".into()));
                } else {
                    hints.push(("d".into(), "delete".into()));
                }
                if !self.state.filtered_habits().is_empty() {
                    hints.push(("/".into(), "filter".into()));
                }
                if self.state.filter_mode {
                    hints.push(("esc".into(), "clear filter".into()));
                }
                hints
            }
            Screen::Detail => {
                vec![
                    ("\u{2190}\u{2191}\u{2193}\u{2192}".into(), "move".into()),
                    ("enter".into(), "toggle".into()),
                    ("e".into(), "edit".into()),
                    ("n".into(), "note".into()),
                    ("esc".into(), "back".into()),
                ]
            }
            Screen::Editor => {
                vec![
                    ("\u{2191}\u{2193}".into(), "navigate".into()),
                    ("enter".into(), "edit".into()),
                    ("esc".into(), "save & back".into()),
                ]
            }
            Screen::DayDetail => {
                vec![
                    ("\u{2191}\u{2193}".into(), "navigate".into()),
                    ("enter".into(), "toggle".into()),
                    ("n".into(), "note".into()),
                    ("esc".into(), "back".into()),
                ]
            }
        }
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(&self.state, &self.theme, self.area.w, self.area.h);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn palette_commands() -> Vec<(String, String)> {
    vec![
        ("Habits".into(), "Open habit tracker".into()),
        ("Habits".into(), "New habit".into()),
        ("Habits".into(), "Today's overview".into()),
    ]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: app.status_hints(),
        palette_commands: palette_commands(),
        request: app.pending_request.take(),
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
    let mut reader = BufReader::new(std::io::stdin().lock());

    let mut app = App::default();
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: HostMsg = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("[habit-tracker] parse error: {e}: {line}");
                        continue;
                    }
                };

                match msg {
                    HostMsg::Init {
                        theme,
                        area,
                        data_dir: _,
                    } => {
                        app.handle_init(theme, area);
                        respond(&mut app, false);
                    }
                    HostMsg::Key { key, .. } => {
                        let consumed = app.handle_key(key);
                        respond(&mut app, consumed);
                    }
                    HostMsg::Tick => {
                        app.handle_tick();
                        respond(&mut app, false);
                    }
                    HostMsg::Focus | HostMsg::Blur => {
                        respond(&mut app, false);
                    }
                    HostMsg::ThemeChange { theme } => {
                        app.theme = theme;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    HostMsg::Resize { area } => {
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    HostMsg::PaletteCommand { index } => {
                        app.handle_palette_command(index);
                        respond(&mut app, false);
                    }
                    HostMsg::PluginMessage { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::Mouse { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::UserUpdate { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::DbValue { key, value } => {
                        app.handle_db_value(&key, value);
                        respond(&mut app, false);
                    }
                    HostMsg::LogEntries { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::Shutdown => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_habits() -> App {
        let mut app = App::default();
        app.state.data.habits.push(state::Habit {
            id: "exercise-2026".into(),
            name: "Exercise".into(),
            description: "Daily workout".into(),
            color: "green".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        });
        app.state.data.habits.push(state::Habit {
            id: "read-2026".into(),
            name: "Read".into(),
            description: "Read books".into(),
            color: "blue".into(),
            created_at: "2026-06-02".into(),
            archived: false,
        });
        app.state.rebuild_sorted();
        app.state.dirty = true;
        app
    }

    #[test]
    fn handle_key_n_opens_editor() {
        let mut app = app_with_habits();
        assert!(app.handle_key(IpcKey::Char('n')));
        assert_eq!(app.state.screen, Screen::Editor);
        assert!(app.state.editor_habit.is_some());
    }

    #[test]
    fn handle_key_enter_opens_detail() {
        let mut app = app_with_habits();
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.screen, Screen::Detail);
    }

    #[test]
    fn handle_key_up_down_navigates() {
        let mut app = app_with_habits();
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.cursor, 1);
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.cursor, 0);
    }

    #[test]
    fn handle_key_d_deletes_habit_with_confirmation() {
        let mut app = app_with_habits();
        assert!(app.handle_key(IpcKey::Char('d')));
        assert!(app.confirming_delete);
        assert!(app.handle_key(IpcKey::Char('d')));
        assert!(!app.confirming_delete);
        assert_eq!(app.state.filtered_habits().len(), 1);
    }

    #[test]
    fn handle_key_esc_on_overview_not_consumed() {
        let mut app = app_with_habits();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_arrows_navigate_heatmap() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Detail;
        app.state.detail_habit_idx = 0;
        app.state.heatmap_row = 8;
        app.state.heatmap_col = 3;
        assert!(app.handle_key(IpcKey::Left));
        assert_eq!(app.state.heatmap_col, 2);
        assert!(app.handle_key(IpcKey::Right));
        assert_eq!(app.state.heatmap_col, 3);
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.heatmap_row, 7);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.heatmap_row, 8);
    }

    #[test]
    fn handle_key_enter_toggles_day_in_heatmap() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Detail;
        app.state.detail_habit_idx = 0;
        app.state.heatmap_row = 16;
        app.state.heatmap_col = 6;
        assert!(app.handle_key(IpcKey::Enter));
    }

    #[test]
    fn handle_key_esc_on_detail_returns_to_overview() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Detail;
        app.state.detail_habit_idx = 0;
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.screen, Screen::Overview);
    }

    #[test]
    fn handle_key_esc_on_editor_returns_to_overview() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Editor;
        app.state.editor_habit = Some(state::Habit {
            id: String::new(),
            name: "Test".into(),
            description: "Desc".into(),
            color: "green".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        });
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.screen, Screen::Overview);
    }

    #[test]
    fn handle_key_esc_on_day_detail_returns_to_detail() {
        let mut app = app_with_habits();
        app.state.screen = Screen::DayDetail;
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.screen, Screen::Detail);
    }

    #[test]
    fn handle_db_value_loads_data() {
        let mut app = App::default();
        let json = serde_json::json!({
            "habits": [{
                "id": "test-1",
                "name": "Test Habit",
                "description": "A test",
                "color": "green",
                "created_at": "2026-06-01",
                "archived": false
            }],
            "entries": []
        });
        app.handle_db_value("habit-tracker", Some(json.to_string()));
        assert_eq!(app.state.data.habits.len(), 1);
        assert_eq!(app.state.data.habits[0].name, "Test Habit");
    }

    #[test]
    fn handle_db_value_none_empty_data() {
        let mut app = app_with_habits();
        assert_eq!(app.state.data.habits.len(), 2);
        app.handle_db_value("habit-tracker", None);
        assert_eq!(app.state.data.habits.len(), 0);
    }

    #[test]
    fn handle_tick_no_date_rollover_noop() {
        let mut app = app_with_habits();
        app.state.day_detail_date = Local::now().format("%Y-%m-%d").to_string();
        app.state.dirty = false;
        app.handle_tick();
    }

    #[test]
    fn schedule_db_save_sets_pending_request() {
        let mut app = app_with_habits();
        assert!(app.pending_request.is_some());
        app.pending_request = None;
        app.schedule_db_save();
        assert!(app.pending_request.is_some());
        match app.pending_request {
            Some(PluginRequest::DbSet { ref key, ref value }) => {
                assert_eq!(key, "habit-tracker");
                assert!(value.contains("Exercise"));
            }
            _ => panic!("expected DbSet"),
        }
    }

    #[test]
    fn palette_command_0_opens_overview() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Editor;
        app.handle_palette_command(0);
        assert_eq!(app.state.screen, Screen::Overview);
    }

    #[test]
    fn palette_command_1_opens_editor() {
        let mut app = app_with_habits();
        app.handle_palette_command(1);
        assert_eq!(app.state.screen, Screen::Editor);
    }

    #[test]
    fn palette_command_2_opens_today_overview() {
        let mut app = app_with_habits();
        app.handle_palette_command(2);
        assert_eq!(app.state.screen, Screen::DayDetail);
    }

    #[test]
    fn handle_key_n_in_detail_starts_note_editing() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Detail;
        app.state.detail_habit_idx = 0;
        app.state.heatmap_row = 16;
        app.state.heatmap_col = 6;
        assert!(app.handle_key(IpcKey::Char('n')));
        assert!(app.state.note_editing);
    }

    #[test]
    fn handle_key_e_opens_editor_for_selected_habit() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Detail;
        app.state.detail_habit_idx = 0;
        assert!(app.handle_key(IpcKey::Char('e')));
        assert_eq!(app.state.screen, Screen::Editor);
    }

    #[test]
    fn handle_key_slash_enters_filter_mode() {
        let mut app = app_with_habits();
        assert!(!app.state.filter_mode);
        assert!(app.handle_key(IpcKey::Char('/')));
        assert!(app.state.filter_mode);
    }

    #[test]
    fn handle_key_esc_clears_filter_mode() {
        let mut app = app_with_habits();
        app.state.filter_mode = true;
        app.state.filter_query = "ex".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(!app.state.filter_mode);
        assert!(app.state.filter_query.is_empty());
    }

    #[test]
    fn handle_key_filter_chars() {
        let mut app = app_with_habits();
        app.state.filter_mode = true;
        assert!(app.handle_key(IpcKey::Char('e')));
        assert_eq!(app.state.filter_query, "e");
        assert!(app.handle_key(IpcKey::Char('x')));
        assert_eq!(app.state.filter_query, "ex");
    }

    #[test]
    fn handle_key_editor_up_down_cycles_focus() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Editor;
        app.state.editor_habit = Some(state::Habit {
            id: String::new(),
            name: "Test".into(),
            description: "Desc".into(),
            color: "green".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        });
        assert_eq!(app.state.editor_focus, FocusField::Name);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.editor_focus, FocusField::Description);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.editor_focus, FocusField::Color);
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.editor_focus, FocusField::Description);
    }

    #[test]
    fn handle_key_editor_cycles_color() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Editor;
        app.state.editor_habit = Some(state::Habit {
            id: String::new(),
            name: "Test".into(),
            description: "Desc".into(),
            color: "green".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        });
        app.state.editor_focus = FocusField::Color;
        assert!(app.handle_key(IpcKey::Right));
        assert_eq!(app.state.editor_habit.as_ref().unwrap().color, "blue");
        assert!(app.handle_key(IpcKey::Left));
        assert_eq!(app.state.editor_habit.as_ref().unwrap().color, "green");
    }

    #[test]
    fn handle_key_enter_toggles_day_detail_entry() {
        let mut app = app_with_habits();
        app.state.screen = Screen::DayDetail;
        app.state.day_detail_date = "2026-06-15".into();
        app.state.cursor = 0;
        assert!(!app.state.is_completed_on("exercise-2026", "2026-06-15"));
        assert!(app.handle_key(IpcKey::Enter));
        assert!(app.state.is_completed_on("exercise-2026", "2026-06-15"));
    }

    #[test]
    fn handle_key_n_in_day_detail_starts_note() {
        let mut app = app_with_habits();
        app.state.screen = Screen::DayDetail;
        app.state.day_detail_date = "2026-06-15".into();
        app.state.cursor = 0;
        assert!(app.handle_key(IpcKey::Char('n')));
        assert!(app.state.note_editing);
    }

    #[test]
    fn handle_note_edit_enter_commits_note() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Detail;
        app.state.detail_habit_idx = 0;
        app.state.heatmap_row = 16;
        app.state.heatmap_col = 6;
        app.state.note_editing = true;
        app.state.note_buffer = "good job".into();
        assert!(app.handle_key(IpcKey::Enter));
        assert!(!app.state.note_editing);
    }

    #[test]
    fn handle_note_edit_esc_cancels_note() {
        let mut app = app_with_habits();
        app.state.note_editing = true;
        app.state.note_buffer = "some note".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(!app.state.note_editing);
        assert!(app.state.note_buffer.is_empty());
    }

    #[test]
    fn heatmap_left_wraps_to_previous_week() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Detail;
        app.state.detail_habit_idx = 0;
        app.state.heatmap_row = 5;
        app.state.heatmap_col = 0;
        assert!(app.handle_key(IpcKey::Left));
        assert_eq!(app.state.heatmap_row, 4);
        assert_eq!(app.state.heatmap_col, 6);
    }

    #[test]
    fn heatmap_right_wraps_to_next_week() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Detail;
        app.state.detail_habit_idx = 0;
        app.state.heatmap_row = 5;
        app.state.heatmap_col = 6;
        assert!(app.handle_key(IpcKey::Right));
        assert_eq!(app.state.heatmap_row, 6);
        assert_eq!(app.state.heatmap_col, 0);
    }

    #[test]
    fn cursor_clamped_at_bounds() {
        let mut app = app_with_habits();
        assert_eq!(app.state.cursor, 0);
        app.handle_key(IpcKey::Up);
        assert_eq!(app.state.cursor, 0);
        app.handle_key(IpcKey::Down);
        app.handle_key(IpcKey::Down);
        app.handle_key(IpcKey::Down);
        assert_eq!(app.state.cursor, 1);
        app.handle_key(IpcKey::Down);
        assert_eq!(app.state.cursor, 1);
    }

    #[test]
    fn ignore_unmapped_keys() {
        let mut app = app_with_habits();
        assert!(!app.handle_key(IpcKey::Tab));
        assert!(!app.handle_key(IpcKey::F(1)));
    }

    #[test]
    fn editor_commit_field_updates_habit() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Editor;
        app.state.editor_habit = Some(state::Habit {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            color: "green".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        });
        app.state.editor_focus = FocusField::Name;
        app.state.editor_buffer = "Running".into();
        app.state.editing = true;
        assert!(app.handle_key(IpcKey::Enter));
        assert!(!app.state.editing);
        assert_eq!(app.state.editor_habit.as_ref().unwrap().name, "Running");
    }

    #[test]
    fn editor_esc_cancels_edit() {
        let mut app = app_with_habits();
        app.state.screen = Screen::Editor;
        app.state.editor_habit = Some(state::Habit {
            id: String::new(),
            name: "Original".into(),
            description: String::new(),
            color: "green".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        });
        app.state.editing = true;
        app.state.editor_buffer = "Changed".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(!app.state.editing);
        assert_eq!(app.state.editor_habit.as_ref().unwrap().name, "Original");
    }
}
