mod state;
mod ui;

use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};
use ui::render_ui;

use state::{NotesState, Screen};

struct App {
    state: NotesState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: NotesState::default(),
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
                success: [0; 3],
                error: [255; 3],
                inverted_text: [255; 3],
            },
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet {
                key: "notes".into(),
            }),
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
            Screen::List => self.handle_list_key(key),
            Screen::View(idx) => self.handle_view_key(key, idx),
            Screen::Edit(idx) => self.handle_edit_key(key, idx),
            Screen::NewTitle => self.handle_new_title_key(key),
            Screen::Rename(idx) => self.handle_rename_key(key, idx),
            Screen::ConfirmDelete(idx) => self.handle_confirm_delete_key(key, idx),
        }
    }

    fn handle_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char('n') => {
                self.state.title_buf.clear();
                self.state.screen = Screen::NewTitle;
                self.dirty = true;
                true
            }
            IpcKey::Char('d') => {
                if let Some(idx) = self.state.selected_note_index() {
                    self.state.screen = Screen::ConfirmDelete(idx);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Char('r') => {
                if let Some(&idx) = self.state.filtered_indices.get(self.state.list_cursor) {
                    if let Some(note) = self.state.notes.get(idx) {
                        self.state.title_buf = note.title.clone();
                        self.state.screen = Screen::Rename(idx);
                        self.dirty = true;
                    }
                }
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.list_cursor = self.state.list_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.filtered_indices.len().saturating_sub(1);
                self.state.list_cursor = self.state.list_cursor.min(max).saturating_add(1).min(max);
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                if !self.state.search_query.is_empty() {
                    self.state.search_query.pop();
                    self.state.apply_filter();
                    self.dirty = true;
                }
                true
            }
            IpcKey::Enter => {
                if let Some(idx) = self.state.selected_note_index() {
                    self.state.scroll_offset = 0;
                    self.state.screen = Screen::View(idx);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.state.search_query.push(c);
                self.state.apply_filter();
                self.state.list_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                let had_query = !self.state.search_query.is_empty();
                if had_query {
                    self.state.search_query.clear();
                    self.state.apply_filter();
                    self.dirty = true;
                }
                had_query
            }
            _ => false,
        }
    }

    fn handle_view_key(&mut self, key: IpcKey, idx: usize) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.scroll_offset = self.state.scroll_offset.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.state.scroll_offset += 1;
                self.dirty = true;
                true
            }
            IpcKey::Char('e') => {
                if let Some(note) = self.state.notes.get(idx) {
                    self.state.edit_buf = note.body.clone();
                    self.state.scroll_offset = 0;
                    self.state.screen = Screen::Edit(idx);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Char('r') => {
                if let Some(note) = self.state.notes.get(idx) {
                    self.state.title_buf = note.title.clone();
                    self.state.screen = Screen::Rename(idx);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Char('d') => {
                self.state.screen = Screen::ConfirmDelete(idx);
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::List;
                self.state.scroll_offset = 0;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_edit_key(&mut self, key: IpcKey, idx: usize) -> bool {
        match key {
            IpcKey::Up => {
                self.state.scroll_offset = self.state.scroll_offset.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down => {
                self.state.scroll_offset += 1;
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                self.state.edit_buf.push('\n');
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.edit_buf.pop();
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.save_edit(idx);
                self.schedule_db_save();
                self.state.screen = Screen::View(idx);
                self.state.scroll_offset = 0;
                self.dirty = true;
                true
            }
            IpcKey::Char(c) => {
                self.state.edit_buf.push(c);
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_new_title_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char(c) if !c.is_control() => {
                self.state.title_buf.push(c);
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.title_buf.pop();
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                if !self.state.title_buf.is_empty() {
                    let title = self.state.title_buf.clone();
                    let new_idx = self.state.add_note(title);
                    self.state.edit_buf.clear();
                    self.state.scroll_offset = 0;
                    self.state.screen = Screen::Edit(new_idx);
                    self.schedule_db_save();
                    self.dirty = true;
                }
                true
            }
            IpcKey::Esc => {
                self.state.title_buf.clear();
                self.state.screen = Screen::List;
                self.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn handle_rename_key(&mut self, key: IpcKey, idx: usize) -> bool {
        match key {
            IpcKey::Char(c) if !c.is_control() => {
                self.state.title_buf.push(c);
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.title_buf.pop();
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                if !self.state.title_buf.is_empty() && idx < self.state.notes.len() {
                    self.state.notes[idx].title = self.state.title_buf.clone();
                    self.schedule_db_save();
                    self.state.screen = Screen::View(idx);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::View(idx);
                self.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn handle_confirm_delete_key(&mut self, key: IpcKey, idx: usize) -> bool {
        match key {
            IpcKey::Char('y') => {
                self.state.delete_note(idx);
                self.schedule_db_save();
                self.state.screen = Screen::List;
                self.dirty = true;
                true
            }
            IpcKey::Char('n') | IpcKey::Esc => {
                self.state.screen = Screen::List;
                self.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "notes" {
            if let Some(json) = value {
                self.state.load(&json);
            }
            self.dirty = true;
        }
    }

    fn schedule_db_save(&mut self) {
        self.pending_request = Some(PluginRequest::DbSet {
            key: "notes".into(),
            value: self.state.serialize(),
        });
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.state.screen = Screen::List;
                self.dirty = true;
            }
            1 => {
                self.state.title_buf.clear();
                self.state.screen = Screen::NewTitle;
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match &self.state.screen {
            Screen::List => {
                let mut hints = vec![
                    ("enter".into(), "open".into()),
                    ("n".into(), "new".into()),
                    ("d".into(), "delete".into()),
                ];
                if !self.state.search_query.is_empty() {
                    hints.push(("esc".into(), "clear search".into()));
                }
                hints
            }
            Screen::View(_) => {
                vec![
                    ("e".into(), "edit".into()),
                    ("r".into(), "rename".into()),
                    ("d".into(), "delete".into()),
                    ("esc".into(), "back".into()),
                ]
            }
            Screen::Edit(_) => {
                vec![
                    ("esc".into(), "save & close".into()),
                    ("\u{2191}\u{2193}".into(), "scroll".into()),
                ]
            }
            Screen::NewTitle | Screen::Rename(_) => {
                vec![
                    ("enter".into(), "confirm".into()),
                    ("esc".into(), "cancel".into()),
                ]
            }
            Screen::ConfirmDelete(_) => {
                vec![
                    ("y".into(), "confirm".into()),
                    ("n/esc".into(), "cancel".into()),
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
        ("Notes".into(), "Open notes".into()),
        ("Notes".into(), "New note".into()),
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
                        log::error!("[notes] parse error: {e}: {line}");
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

    fn base_app() -> App {
        App::default()
    }

    fn seeded_app() -> App {
        let mut app = App::default();
        app.state.notes.push(state::Note {
            id: 1,
            title: "Note 1".into(),
            body: "body 1".into(),
            updated_at: 100,
        });
        app.state.notes.push(state::Note {
            id: 2,
            title: "Note 2".into(),
            body: "body 2".into(),
            updated_at: 200,
        });
        app.state.apply_filter();
        app
    }

    #[test]
    fn handle_key_n_opens_new_title() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('n')));
        assert_eq!(app.state.screen, Screen::NewTitle);
    }

    #[test]
    fn handle_key_enter_opens_view() {
        let mut app = seeded_app();
        assert!(app.handle_key(IpcKey::Enter));
        assert!(matches!(app.state.screen, Screen::View(_)));
    }

    #[test]
    fn handle_key_d_opens_confirm_delete() {
        let mut app = seeded_app();
        assert!(app.handle_key(IpcKey::Char('d')));
        assert!(matches!(app.state.screen, Screen::ConfirmDelete(_)));
    }

    #[test]
    fn handle_key_y_in_confirm_deletes_note() {
        let mut app = seeded_app();
        let idx = app.state.selected_note_index().unwrap();
        app.state.screen = Screen::ConfirmDelete(idx);
        assert_eq!(app.state.notes.len(), 2);
        assert!(app.handle_key(IpcKey::Char('y')));
        assert_eq!(app.state.screen, Screen::List);
        assert_eq!(app.state.notes.len(), 1);
    }

    #[test]
    fn handle_key_esc_list_empty_search_not_consumed() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_esc_list_nonempty_search_clears() {
        let mut app = base_app();
        app.state.search_query = "test".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(app.state.search_query.is_empty());
    }

    #[test]
    fn handle_key_char_in_list_filters() {
        let mut app = seeded_app();
        app.handle_key(IpcKey::Char('N'));
        app.handle_key(IpcKey::Char('o'));
        app.handle_key(IpcKey::Char('t'));
        app.handle_key(IpcKey::Char('e'));
        assert!(!app.state.filtered_indices.is_empty());
    }

    #[test]
    fn handle_key_enter_in_new_title_creates_note() {
        let mut app = base_app();
        app.state.screen = Screen::NewTitle;
        app.state.title_buf = "Hello".into();
        assert!(app.handle_key(IpcKey::Enter));
        assert!(matches!(app.state.screen, Screen::Edit(_)));
        assert_eq!(app.state.notes.len(), 1);
        assert_eq!(app.state.notes[0].title, "Hello");
    }

    #[test]
    fn handle_key_esc_in_new_title_cancels() {
        let mut app = base_app();
        app.state.screen = Screen::NewTitle;
        app.state.title_buf = "Hello".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.screen, Screen::List);
        assert!(app.state.title_buf.is_empty());
    }

    #[test]
    fn handle_key_esc_in_edit_saves() {
        let mut app = seeded_app();
        let idx = 0;
        app.state.screen = Screen::Edit(idx);
        app.state.edit_buf = "new content".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(matches!(app.state.screen, Screen::View(_)));
        assert_eq!(app.state.notes[idx].body, "new content");
    }

    #[test]
    fn handle_db_value_loads_notes() {
        let mut app = base_app();
        let json = r#"[{"id":1,"title":"Test","body":"hello","updated_at":100}]"#;
        app.handle_db_value("notes", Some(json.into()));
        assert_eq!(app.state.notes.len(), 1);
        assert_eq!(app.state.notes[0].title, "Test");
    }

    #[test]
    fn handle_db_value_none_starts_empty() {
        let mut app = seeded_app();
        app.handle_db_value("notes", None);
        assert_eq!(app.state.notes.len(), 2); // keeps existing
    }

    #[test]
    fn palette_command_1_opens_new_title() {
        let mut app = base_app();
        app.handle_palette_command(1);
        assert_eq!(app.state.screen, Screen::NewTitle);
    }
}
