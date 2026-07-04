mod state;
mod ui;

use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginMessage, PluginRequest, RenderCmd, ThemeData,
};

use state::{generate_id, unix_now, Screen, SshState};
use ui::render_ui;

struct App {
    state: SshState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    pending_plugin_message: Option<PluginMessage>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: SshState::default(),
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
                key: "ssh-bookmarks".into(),
            }),
            pending_plugin_message: None,
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
        match self.state.screen {
            Screen::List => self.handle_list_key(key),
            Screen::Detail => self.handle_detail_key(key),
            Screen::Connect => {
                self.state.screen = Screen::List;
                self.state.connect_in_progress = false;
                self.dirty = true;
                true
            }
        }
    }

    fn handle_list_key(&mut self, key: IpcKey) -> bool {
        if self.state.filter_active {
            return self.handle_filter_key(key);
        }
        if let Some(ref mut msg) = self.state.message {
            msg.1 = msg.1.saturating_sub(1);
            if msg.1 == 0 {
                self.state.message = None;
            }
            if matches!(key, IpcKey::Char('d')) {
                self.confirm_delete();
                return true;
            }
            self.state.message = None;
            self.dirty = true;
            return true;
        }
        match key {
            IpcKey::Char('/') => {
                self.state.filter_active = true;
                self.state.filter_text.clear();
                self.state.rebuild_filtered_indices();
                self.dirty = true;
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.cursor = self.state.cursor.saturating_sub(1);
                self.scroll_into_view();
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.filtered_indices.len().saturating_sub(1);
                self.state.cursor = self.state.cursor.min(max).saturating_add(1).min(max);
                self.scroll_into_view();
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                if let Some(&bm_idx) = self.state.filtered_indices.get(self.state.cursor) {
                    self.handle_connect(bm_idx);
                }
                true
            }
            IpcKey::Char('e') => {
                if let Some(&bm_idx) = self.state.filtered_indices.get(self.state.cursor) {
                    self.open_detail(bm_idx);
                }
                true
            }
            IpcKey::Char('n') => {
                self.create_new_bookmark();
                true
            }
            IpcKey::Char('d') => {
                if !self.state.filtered_indices.is_empty() {
                    self.show_delete_confirmation();
                }
                true
            }
            IpcKey::Esc => {
                if self.state.filter_active {
                    self.state.filter_active = false;
                    self.state.filter_text.clear();
                    self.state.rebuild_filtered_indices();
                    self.dirty = true;
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn handle_filter_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char(c) if !c.is_control() => {
                self.state.filter_text.push(c);
                self.state.rebuild_filtered_indices();
                self.state.cursor = 0;
                self.state.scroll = 0;
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.filter_text.pop();
                self.state.rebuild_filtered_indices();
                self.state.cursor = 0;
                self.state.scroll = 0;
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                self.state.filter_active = false;
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.filter_active = false;
                self.state.filter_text.clear();
                self.state.rebuild_filtered_indices();
                self.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn handle_detail_key(&mut self, key: IpcKey) -> bool {
        if self.state.editing {
            return self.handle_detail_editing_key(key);
        }
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.detail_edit_field = self.state.detail_edit_field.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.state.detail_edit_field =
                    self.state.detail_edit_field.min(6).saturating_add(1).min(6);
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                let bm = self
                    .state
                    .detail_idx
                    .and_then(|i| self.state.data.bookmarks.get(i))
                    .cloned()
                    .unwrap_or_default();
                self.state.edit_buffer = self.field_value(&bm, self.state.detail_edit_field);
                self.state.editing = true;
                self.dirty = true;
                true
            }
            IpcKey::Esc | IpcKey::F(2) => {
                self.save_detail_and_return();
                true
            }
            _ => false,
        }
    }

    fn handle_detail_editing_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char(c) if !c.is_control() => {
                self.state.edit_buffer.push(c);
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.edit_buffer.pop();
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                self.commit_edit_field();
                self.state.editing = false;
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.editing = false;
                self.state.edit_buffer.clear();
                self.dirty = true;
                true
            }
            IpcKey::F(2) => {
                self.commit_edit_field();
                self.state.editing = false;
                self.save_detail_and_return();
                true
            }
            _ => true,
        }
    }

    fn field_value(&self, bm: &state::SshBookmark, field: usize) -> String {
        match field {
            0 => bm.name.clone(),
            1 => bm.host.clone(),
            2 => bm.port.to_string(),
            3 => bm.user.clone(),
            4 => bm.key_path.clone().unwrap_or_default(),
            5 => bm.category.clone(),
            6 => bm.description.clone(),
            _ => String::new(),
        }
    }

    fn commit_edit_field(&mut self) {
        let Some(idx) = self.state.detail_idx else {
            return;
        };
        let Some(bm) = self.state.data.bookmarks.get_mut(idx) else {
            return;
        };
        match self.state.detail_edit_field {
            0 => bm.name = std::mem::take(&mut self.state.edit_buffer),
            1 => bm.host = std::mem::take(&mut self.state.edit_buffer),
            2 => {
                bm.port = self.state.edit_buffer.parse::<u16>().unwrap_or(22);
                self.state.edit_buffer.clear();
            }
            3 => bm.user = std::mem::take(&mut self.state.edit_buffer),
            4 => {
                let val = std::mem::take(&mut self.state.edit_buffer);
                bm.key_path = if val.is_empty() { None } else { Some(val) };
            }
            5 => bm.category = std::mem::take(&mut self.state.edit_buffer),
            6 => bm.description = std::mem::take(&mut self.state.edit_buffer),
            _ => {}
        }
        self.schedule_db_save();
    }

    fn save_detail_and_return(&mut self) {
        if let Some(idx) = self.state.detail_idx {
            let id = self.state.data.bookmarks[idx].id.clone();
            if id.is_empty() {
                self.state.data.bookmarks[idx].id = generate_id();
            }
            if self.state.data.bookmarks[idx].name.is_empty() {
                self.state.data.bookmarks[idx].name =
                    format!("Unnamed {}", self.state.data.bookmarks[idx].host);
            }
        }
        self.schedule_db_save();
        self.state.screen = Screen::List;
        self.state.rebuild_filtered_indices();
        self.state.detail_idx = None;
        self.state.editing = false;
        self.state.edit_buffer.clear();
        self.dirty = true;
    }

    fn scroll_into_view(&mut self) {
        if self.state.scroll > self.state.cursor {
            self.state.scroll = self.state.cursor;
        }
        let max_items = self.area.h.saturating_sub(3).saturating_sub(4) as usize;
        let max_scroll = self.state.filtered_indices.len().saturating_sub(max_items);
        self.state.scroll = self.state.scroll.min(max_scroll);
        let visible_end = self.state.scroll.saturating_add(max_items);
        if self.state.cursor >= visible_end && self.state.cursor > 0 {
            self.state.scroll = self
                .state
                .cursor
                .saturating_sub(max_items.saturating_sub(1));
            self.state.scroll = self.state.scroll.min(max_scroll);
        }
    }

    fn handle_connect(&mut self, idx: usize) {
        let bm = &self.state.data.bookmarks[idx];
        let mut cmd = String::from("ssh");
        if let Some(ref key) = bm.key_path {
            cmd.push_str(&format!(" -i {}", key));
        }
        if bm.port != 22 {
            cmd.push_str(&format!(" -p {}", bm.port));
        }
        cmd.push_str(&format!(" {}@{}", bm.user, bm.host));

        self.pending_plugin_message = Some(PluginMessage {
            to: "host".into(),
            action: "ssh_connect".into(),
            data: serde_json::json!({ "command": cmd }),
        });

        self.state.data.bookmarks[idx].last_connected_at = Some(unix_now());
        self.schedule_db_save();

        self.state.screen = Screen::Connect;
        self.state.detail_idx = Some(idx);
        self.state.connect_in_progress = true;
        self.dirty = true;
    }

    fn open_detail(&mut self, idx: usize) {
        self.state.detail_idx = Some(idx);
        self.state.detail_edit_field = 0;
        self.state.edit_buffer.clear();
        self.state.editing = false;
        self.state.screen = Screen::Detail;
        self.dirty = true;
    }

    fn create_new_bookmark(&mut self) {
        let bm = state::SshBookmark::default();
        self.state.data.bookmarks.push(bm);
        let idx = self.state.data.bookmarks.len() - 1;
        self.open_detail(idx);
    }

    fn show_delete_confirmation(&mut self) {
        self.state.message = Some(("Press d again to confirm delete".into(), 4));
        self.dirty = true;
    }

    fn confirm_delete(&mut self) {
        if let Some(&bm_idx) = self.state.filtered_indices.get(self.state.cursor) {
            self.state.data.bookmarks.remove(bm_idx);
            self.state.message = Some(("Bookmark deleted".into(), 3));
            self.state.rebuild_filtered_indices();
            self.schedule_db_save();
            self.dirty = true;
        }
    }

    fn handle_tick(&mut self) {
        if let Some(ref mut msg) = self.state.message {
            msg.1 = msg.1.saturating_sub(1);
            if msg.1 == 0 {
                self.state.message = None;
                self.dirty = true;
            }
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "ssh-bookmarks" {
            if let Some(json) = value {
                if let Ok(data) = serde_json::from_str::<state::SshData>(&json) {
                    self.state.data = data;
                }
            }
            self.state.rebuild_filtered_indices();
            self.dirty = true;
        }
    }

    fn schedule_db_save(&mut self) {
        self.pending_request = Some(PluginRequest::DbSet {
            key: "ssh-bookmarks".into(),
            value: serde_json::to_string(&self.state.data).unwrap(),
        });
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.state.screen = Screen::List;
                self.state.rebuild_filtered_indices();
                self.dirty = true;
            }
            1 => {
                self.create_new_bookmark();
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match &self.state.screen {
            Screen::List => {
                if self.state.filter_active {
                    vec![
                        ("type".into(), "filter".into()),
                        ("esc".into(), "clear".into()),
                        ("enter".into(), "done".into()),
                    ]
                } else if self.state.message.is_some() {
                    vec![
                        ("d".into(), "confirm".into()),
                        ("other".into(), "cancel".into()),
                    ]
                } else {
                    vec![
                        ("↑↓".into(), "navigate".into()),
                        ("enter".into(), "connect".into()),
                        ("e".into(), "edit".into()),
                        ("n".into(), "new".into()),
                        ("d".into(), "delete".into()),
                        ("/".into(), "filter".into()),
                    ]
                }
            }
            Screen::Detail => {
                if self.state.editing {
                    vec![
                        ("enter".into(), "commit".into()),
                        ("esc".into(), "cancel".into()),
                    ]
                } else {
                    vec![
                        ("↑↓".into(), "field".into()),
                        ("enter".into(), "edit".into()),
                        ("esc".into(), "save".into()),
                        ("F2".into(), "save".into()),
                    ]
                }
            }
            Screen::Connect => vec![("...".into(), "connecting".into())],
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
        ("SSH".into(), "Open SSH bookmarks".into()),
        ("SSH".into(), "New bookmark".into()),
    ]
}

fn respond(app: &mut App, consumed: bool) {
    let commands_val = match serde_json::to_value(app.render()) {
        Ok(v) => v,
        Err(e) => {
            log::error!("failed to serialize render commands: {e}");
            return;
        }
    };
    let hints = app.status_hints();
    let palette = palette_commands();
    let request = app.pending_request.take();
    let plugin_message = app.pending_plugin_message.take();
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": hints,
        "palette_commands": palette,
        "request": request,
        "plugin_message": plugin_message,
        "consumed": consumed,
    });
    let Ok(json_str) = serde_json::to_string(&json) else {
        return;
    };
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json_str}");
    let _ = out.flush();
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
                        log::error!("[ssh-bookmarks] parse error: {e}: {line}");
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
                    HostMsg::Shutdown => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> ThemeData {
        ThemeData {
            text: [200; 3],
            text_muted: [100; 3],
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
        }
    }

    fn test_bm(name: &str, host: &str, user: &str) -> state::SshBookmark {
        state::SshBookmark {
            id: name.to_lowercase().replace(' ', "-"),
            name: name.into(),
            host: host.into(),
            port: 22,
            user: user.into(),
            key_path: None,
            category: "General".into(),
            description: String::new(),
            last_connected_at: None,
        }
    }

    fn app_with_bookmarks(bookmarks: Vec<state::SshBookmark>) -> App {
        let mut app = App::default();
        app.pending_request = None;
        app.state.data.bookmarks = bookmarks;
        app.state.rebuild_filtered_indices();
        app
    }

    #[test]
    fn app_implements_default() {
        let app = App::default();
        assert!(app.state.data.bookmarks.is_empty());
        assert_eq!(app.state.screen, Screen::List);
        assert!(app.pending_request.is_some());
    }

    #[test]
    fn handle_key_up_down_navigates() {
        let mut app = app_with_bookmarks(vec![
            test_bm("Alpha", "10.0.0.1", "root"),
            test_bm("Beta", "10.0.0.2", "root"),
            test_bm("Gamma", "10.0.0.3", "root"),
        ]);
        app.handle_key(IpcKey::Down);
        assert_eq!(app.state.cursor, 1);
        app.handle_key(IpcKey::Down);
        assert_eq!(app.state.cursor, 2);
        app.handle_key(IpcKey::Down);
        assert_eq!(app.state.cursor, 2);
        app.handle_key(IpcKey::Up);
        assert_eq!(app.state.cursor, 1);
        app.handle_key(IpcKey::Up);
        assert_eq!(app.state.cursor, 0);
        app.handle_key(IpcKey::Up);
        assert_eq!(app.state.cursor, 0);
    }

    #[test]
    fn handle_key_enter_triggers_connect() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        let consumed = app.handle_key(IpcKey::Enter);
        assert!(consumed);
        assert_eq!(app.state.screen, Screen::Connect);
        assert!(app.pending_plugin_message.is_some());
        let msg = app.pending_plugin_message.as_ref().unwrap();
        assert_eq!(msg.to, "host");
        assert_eq!(msg.action, "ssh_connect");
        assert!(app.state.data.bookmarks[0].last_connected_at.is_some());
    }

    #[test]
    fn handle_key_e_opens_detail() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        let consumed = app.handle_key(IpcKey::Char('e'));
        assert!(consumed);
        assert_eq!(app.state.screen, Screen::Detail);
        assert_eq!(app.state.detail_idx, Some(0));
    }

    #[test]
    fn handle_key_n_creates_new() {
        let mut app = app_with_bookmarks(vec![]);
        let consumed = app.handle_key(IpcKey::Char('n'));
        assert!(consumed);
        assert_eq!(app.state.screen, Screen::Detail);
        assert_eq!(app.state.data.bookmarks.len(), 1);
    }

    #[test]
    fn handle_key_d_deletes_bookmark() {
        let mut app = app_with_bookmarks(vec![
            test_bm("Alpha", "10.0.0.1", "root"),
            test_bm("Beta", "10.0.0.2", "root"),
        ]);
        let consumed = app.handle_key(IpcKey::Char('d'));
        assert!(consumed);
        assert!(app.state.message.is_some());
        let consumed2 = app.handle_key(IpcKey::Char('d'));
        assert!(consumed2);
        assert_eq!(app.state.data.bookmarks.len(), 1);
        assert_eq!(app.state.data.bookmarks[0].name, "Beta");
    }

    #[test]
    fn handle_key_slash_activates_filter() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        let consumed = app.handle_key(IpcKey::Char('/'));
        assert!(consumed);
        assert!(app.state.filter_active);
    }

    #[test]
    fn handle_key_esc_clears_filter() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.state.filter_active = true;
        app.state.filter_text = "Al".into();
        app.state.filtered_indices = vec![0];
        let consumed = app.handle_key(IpcKey::Esc);
        assert!(consumed);
        assert!(!app.state.filter_active);
        assert!(app.state.filter_text.is_empty());
    }

    #[test]
    fn handle_key_esc_on_list_not_consumed() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        let consumed = app.handle_key(IpcKey::Esc);
        assert!(!consumed);
    }

    #[test]
    fn handle_db_value_loads_data() {
        let mut app = App::default();
        let data = state::SshData {
            bookmarks: vec![test_bm("Loaded", "10.0.0.1", "root")],
        };
        let json = serde_json::to_string(&data).unwrap();
        app.handle_db_value("ssh-bookmarks", Some(json));
        assert_eq!(app.state.data.bookmarks.len(), 1);
        assert_eq!(app.state.data.bookmarks[0].name, "Loaded");
    }

    #[test]
    fn handle_db_value_none_empty_vec() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.handle_db_value("ssh-bookmarks", None);
        assert_eq!(app.state.data.bookmarks.len(), 1);
        assert_eq!(app.state.data.bookmarks[0].name, "Alpha");
    }

    #[test]
    fn filter_char_appends_and_filters() {
        let mut app = app_with_bookmarks(vec![
            test_bm("Prod Web", "10.0.1.10", "root"),
            test_bm("Dev Box", "10.0.2.20", "dev"),
        ]);
        app.state.filter_active = true;
        app.handle_key(IpcKey::Char('P'));
        app.handle_key(IpcKey::Char('r'));
        assert_eq!(app.state.filter_text, "Pr");
        assert_eq!(app.state.filtered_indices.len(), 1);
    }

    #[test]
    fn palette_command_0_opens_list() {
        let mut app = app_with_bookmarks(vec![]);
        app.state.screen = Screen::Detail;
        app.handle_palette_command(0);
        assert_eq!(app.state.screen, Screen::List);
    }

    #[test]
    fn palette_command_1_creates_new() {
        let mut app = app_with_bookmarks(vec![]);
        app.handle_palette_command(1);
        assert_eq!(app.state.screen, Screen::Detail);
        assert_eq!(app.state.data.bookmarks.len(), 1);
    }

    #[test]
    fn detail_up_down_navigates_fields() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.open_detail(0);
        app.handle_key(IpcKey::Down);
        assert_eq!(app.state.detail_edit_field, 1);
        app.handle_key(IpcKey::Up);
        assert_eq!(app.state.detail_edit_field, 0);
    }

    #[test]
    fn detail_enter_edits_field() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.open_detail(0);
        let consumed = app.handle_key(IpcKey::Enter);
        assert!(consumed);
        assert!(app.state.editing);
        assert_eq!(app.state.edit_buffer, "Alpha");
    }

    #[test]
    fn detail_editing_char_appends() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.open_detail(0);
        app.handle_key(IpcKey::Enter);
        app.handle_key(IpcKey::Char('!'));
        assert_eq!(app.state.edit_buffer, "Alpha!");
    }

    #[test]
    fn detail_editing_enter_commits() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.open_detail(0);
        app.handle_key(IpcKey::Enter);
        app.state.edit_buffer = "NewName".into();
        app.handle_key(IpcKey::Enter);
        assert!(!app.state.editing);
        assert_eq!(app.state.data.bookmarks[0].name, "NewName");
    }

    #[test]
    fn detail_editing_esc_cancels() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.open_detail(0);
        app.handle_key(IpcKey::Enter);
        app.state.edit_buffer = "Changed".into();
        app.handle_key(IpcKey::Esc);
        assert!(!app.state.editing);
        assert_eq!(app.state.data.bookmarks[0].name, "Alpha");
    }

    #[test]
    fn detail_esc_saves_and_returns() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.open_detail(0);
        let consumed = app.handle_key(IpcKey::Esc);
        assert!(consumed);
        assert_eq!(app.state.screen, Screen::List);
    }

    #[test]
    fn connect_screen_key_returns_to_list() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.state.screen = Screen::Connect;
        let consumed = app.handle_key(IpcKey::Enter);
        assert!(consumed);
        assert_eq!(app.state.screen, Screen::List);
    }

    #[test]
    fn handle_tick_decrements_message_timeout() {
        let mut app = app_with_bookmarks(vec![]);
        app.state.message = Some(("test".into(), 3));
        app.handle_tick();
        assert_eq!(app.state.message.as_ref().unwrap().1, 2);
        app.handle_tick();
        assert_eq!(app.state.message.as_ref().unwrap().1, 1);
        app.handle_tick();
        assert!(app.state.message.is_none());
    }

    #[test]
    fn connect_with_non_default_port() {
        let mut app = app_with_bookmarks(vec![state::SshBookmark {
            id: "custom".into(),
            name: "Custom".into(),
            host: "example.com".into(),
            port: 2222,
            user: "admin".into(),
            key_path: Some("/home/admin/.ssh/id_rsa".into()),
            category: "Dev".into(),
            description: String::new(),
            last_connected_at: None,
        }]);
        app.handle_key(IpcKey::Enter);
        let msg = app.pending_plugin_message.as_ref().unwrap();
        let cmd = msg.data.get("command").and_then(|v| v.as_str()).unwrap();
        assert!(cmd.contains("-p 2222"));
        assert!(cmd.contains("-i /home/admin/.ssh/id_rsa"));
        assert!(cmd.contains("admin@example.com"));
    }

    #[test]
    fn schedule_db_save_sets_pending_request() {
        let mut app = app_with_bookmarks(vec![test_bm("Alpha", "10.0.0.1", "root")]);
        app.schedule_db_save();
        let req = app.pending_request.take();
        assert!(matches!(req, Some(PluginRequest::DbSet { .. })));
    }
}
