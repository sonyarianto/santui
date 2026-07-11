mod client;
mod state;
mod ui;

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, PluginRequest, RenderCmd, ThemeData,
};

use client::{send_request, HttpMethod, HttpResponse};
use state::{
    ClientState, EditField, FetchState, PersistedState, RequestEntry, Screen, HISTORY_MAX,
};
use ui::render_ui;

enum FetchMsg {
    ResponseDone(HttpResponse),
    ResponseError(String),
}

struct App {
    state: ClientState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    rx_fetch: Option<mpsc::Receiver<FetchMsg>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: ClientState::default(),
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
                key: "http-client".into(),
            }),
            rx_fetch: None,
        }
    }
}

impl App {
    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.dirty = true;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        match self.state.screen {
            Screen::Editor => self.handle_editor_key(key, modifiers),
            Screen::Response => self.handle_response_key(key, modifiers),
            Screen::History => self.handle_history_key(key, modifiers),
            Screen::Saved => self.handle_saved_key(key, modifiers),
            Screen::MethodPicker => self.handle_picker_key(key, modifiers),
        }
    }

    fn handle_editor_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        match key {
            IpcKey::Tab => {
                self.state.field_focus_idx = (self.state.field_focus_idx + 1) % 4;
                self.state.edit_field = EditField::from_idx(self.state.field_focus_idx);
                self.state.edit_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::BackTab => {
                self.state.field_focus_idx = (self.state.field_focus_idx + 3) % 4;
                self.state.edit_field = EditField::from_idx(self.state.field_focus_idx);
                self.state.edit_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Char('M') if self.state.edit_field == EditField::Method => {
                self.state.screen = Screen::MethodPicker;
                self.state.picker_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Enter if self.state.edit_field == EditField::Method => {
                self.state.screen = Screen::MethodPicker;
                self.state.picker_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Char(c) if self.state.edit_field != EditField::Method && !c.is_control() => {
                self.state.insert_char(c);
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.backspace();
                self.dirty = true;
                true
            }
            IpcKey::Left => {
                self.state.cursor_left();
                self.dirty = true;
                true
            }
            IpcKey::Right => {
                self.state.cursor_right();
                self.dirty = true;
                true
            }
            IpcKey::Char('s') if modifiers.ctrl && !modifiers.shift => {
                self.send_request();
                true
            }
            IpcKey::Char('S') if modifiers.ctrl && modifiers.shift => {
                self.save_current_request();
                true
            }
            IpcKey::Char('h') if !modifiers.ctrl && !modifiers.shift => {
                self.state.screen = Screen::History;
                self.state.history_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Char('S') if !modifiers.ctrl && modifiers.shift => {
                self.state.screen = Screen::Saved;
                self.state.saved_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_response_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        match key {
            IpcKey::Char('e') | IpcKey::Char('E') => {
                self.state.screen = Screen::Editor;
                self.dirty = true;
                true
            }
            IpcKey::Char('h') | IpcKey::Char('H') => {
                self.state.screen = Screen::History;
                self.state.history_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Char('s') | IpcKey::Char('S') => {
                self.save_current_request();
                true
            }
            IpcKey::Up => {
                self.state.response_scroll = self.state.response_scroll.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down => {
                self.state.response_scroll = self.state.response_scroll.saturating_add(1);
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::Editor;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_history_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.history_cursor = self.state.history_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.history.len().saturating_sub(1);
                self.state.history_cursor = self
                    .state
                    .history_cursor
                    .min(max)
                    .saturating_add(1)
                    .min(max);
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                if let Some(entry) = self.state.history.get(self.state.history_cursor).cloned() {
                    load_entry(&mut self.state, &entry);
                }
                self.state.screen = Screen::Editor;
                self.dirty = true;
                true
            }
            IpcKey::Char('S') | IpcKey::Char('s') => {
                if let Some(entry) = self.state.history.get(self.state.history_cursor).cloned() {
                    self.state.saved_requests.push(entry);
                    self.schedule_db_save();
                }
                self.dirty = true;
                true
            }
            IpcKey::Char('d') => {
                if self.state.history_cursor < self.state.history.len() {
                    self.state.history.remove(self.state.history_cursor);
                    if self.state.history_cursor >= self.state.history.len()
                        && self.state.history_cursor > 0
                    {
                        self.state.history_cursor = self.state.history.len().saturating_sub(1);
                    }
                    self.schedule_db_save();
                }
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::Editor;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_saved_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.saved_cursor = self.state.saved_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.saved_requests.len().saturating_sub(1);
                self.state.saved_cursor =
                    self.state.saved_cursor.min(max).saturating_add(1).min(max);
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                if let Some(entry) = self
                    .state
                    .saved_requests
                    .get(self.state.saved_cursor)
                    .cloned()
                {
                    load_entry(&mut self.state, &entry);
                }
                self.state.screen = Screen::Editor;
                self.dirty = true;
                true
            }
            IpcKey::Char('d') => {
                if self.state.saved_cursor < self.state.saved_requests.len() {
                    self.state.saved_requests.remove(self.state.saved_cursor);
                    if self.state.saved_cursor >= self.state.saved_requests.len()
                        && self.state.saved_cursor > 0
                    {
                        self.state.saved_cursor = self.state.saved_requests.len().saturating_sub(1);
                    }
                    self.schedule_db_save();
                }
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::Editor;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_picker_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.picker_cursor = self.state.picker_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = HttpMethod::all().len().saturating_sub(1);
                self.state.picker_cursor =
                    self.state.picker_cursor.min(max).saturating_add(1).min(max);
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                let methods = HttpMethod::all();
                if let Some(m) = methods.get(self.state.picker_cursor) {
                    self.state.method = m.clone();
                }
                self.state.screen = Screen::Editor;
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::Editor;
                self.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn send_request(&mut self) {
        let method = self.state.method.clone();
        let url = self.state.url.clone();
        let headers = parse_headers(&self.state.headers_text);
        let body = self.state.body_text.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.state.fetch_state = FetchState::Sending;
        self.state.response = None;
        self.state.response_scroll = 0;
        std::thread::spawn(
            move || match send_request(&method, &url, &headers, &body, 30) {
                Ok(response) => {
                    let _ = tx.send(FetchMsg::ResponseDone(response));
                }
                Err(e) => {
                    let _ = tx.send(FetchMsg::ResponseError(e));
                }
            },
        );
        self.dirty = true;
    }

    fn save_current_request(&mut self) {
        let entry = RequestEntry {
            method: self.state.method.as_str().to_string(),
            url: self.state.url.clone(),
            headers: self.state.headers_text.clone(),
            body: self.state.body_text.clone(),
        };
        self.state.saved_requests.push(entry);
        self.schedule_db_save();
    }

    fn handle_tick(&mut self) {
        if let Some(ref rx) = self.rx_fetch {
            match rx.try_recv() {
                Ok(FetchMsg::ResponseDone(response)) => {
                    self.state.response = Some(response);
                    self.state.fetch_state = FetchState::Done;
                    self.state.screen = Screen::Response;
                    let entry = RequestEntry {
                        method: self.state.method.as_str().into(),
                        url: self.state.url.clone(),
                        headers: self.state.headers_text.clone(),
                        body: self.state.body_text.clone(),
                    };
                    self.state.history.insert(0, entry);
                    if self.state.history.len() > HISTORY_MAX {
                        self.state.history.truncate(HISTORY_MAX);
                    }
                    self.schedule_db_save();
                    self.dirty = true;
                }
                Ok(FetchMsg::ResponseError(e)) => {
                    self.state.fetch_state = FetchState::Error(e);
                    self.state.screen = Screen::Response;
                    self.dirty = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "http-client" {
            if let Some(json) = value {
                if let Ok(p) = serde_json::from_str::<PersistedState>(&json) {
                    self.state.history = p.history;
                    self.state.saved_requests = p.saved_requests;
                }
            }
            self.dirty = true;
        }
    }

    fn schedule_db_save(&mut self) {
        let p = PersistedState {
            history: self.state.history.clone(),
            saved_requests: self.state.saved_requests.clone(),
        };
        self.pending_request = Some(PluginRequest::DbSet {
            key: "http-client".into(),
            value: serde_json::to_string(&p).unwrap(),
        });
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.state.screen = Screen::Editor;
                self.dirty = true;
            }
            1 => {
                self.send_request();
            }
            2 => {
                self.state.screen = Screen::History;
                self.state.history_cursor = 0;
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match &self.state.screen {
            Screen::Editor => vec![
                ("Tab".into(), "next field".into()),
                ("Ctrl+S".into(), "send".into()),
                ("H".into(), "history".into()),
            ],
            Screen::Response => vec![
                ("E".into(), "edit".into()),
                ("H".into(), "history".into()),
                ("Esc".into(), "back".into()),
            ],
            Screen::History => vec![
                ("Enter".into(), "load".into()),
                ("d".into(), "delete".into()),
                ("Esc".into(), "back".into()),
            ],
            Screen::Saved => vec![
                ("Enter".into(), "load".into()),
                ("d".into(), "delete".into()),
                ("Esc".into(), "back".into()),
            ],
            Screen::MethodPicker => vec![
                ("Enter".into(), "select".into()),
                ("Esc".into(), "cancel".into()),
            ],
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

fn parse_headers(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let mut parts = line.splitn(2, ':');
            let key = parts.next()?.trim().to_string();
            let value = parts.next().unwrap_or("").trim().to_string();
            if key.is_empty() {
                None
            } else {
                Some((key, value))
            }
        })
        .collect()
}

fn load_entry(state: &mut ClientState, entry: &RequestEntry) {
    if let Some(m) = HttpMethod::from_str(&entry.method) {
        state.method = m;
    }
    state.url = entry.url.clone();
    state.headers_text = entry.headers.clone();
    state.body_text = entry.body.clone();
    state.edit_field = EditField::Url;
    state.field_focus_idx = 1;
    state.edit_cursor = 0;
    state.response = None;
    state.fetch_state = FetchState::Idle;
}

fn palette_commands() -> Vec<(String, String)> {
    vec![
        ("HTTP".into(), "Open HTTP client".into()),
        ("HTTP".into(), "Send current request".into()),
        ("HTTP".into(), "Request history".into()),
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
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": hints,
        "palette_commands": palette,
        "request": request,
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
                        log::error!("[http-client] parse error: {e}: {line}");
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
                    HostMsg::Key { key, modifiers } => {
                        let consumed = app.handle_key(key, modifiers);
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

    fn base_app() -> App {
        let mut app = App::default();
        // Clear the initial DbGet pending request so tests don't double-get
        app.pending_request = None;
        app
    }

    #[test]
    fn handle_key_tab_cycles_fields() {
        let mut app = base_app();
        assert_eq!(app.state.field_focus_idx, 0);
        assert!(app.handle_key(IpcKey::Tab, IpcKeyModifiers::default()));
        assert_eq!(app.state.field_focus_idx, 1);
        assert_eq!(app.state.edit_field, EditField::Url);
        assert!(app.handle_key(IpcKey::Tab, IpcKeyModifiers::default()));
        assert_eq!(app.state.field_focus_idx, 2);
        assert_eq!(app.state.edit_field, EditField::Headers);
        assert!(app.handle_key(IpcKey::Tab, IpcKeyModifiers::default()));
        assert_eq!(app.state.field_focus_idx, 3);
        assert_eq!(app.state.edit_field, EditField::Body);
        assert!(app.handle_key(IpcKey::Tab, IpcKeyModifiers::default()));
        assert_eq!(app.state.field_focus_idx, 0);
        assert_eq!(app.state.edit_field, EditField::Method);
    }

    #[test]
    fn handle_key_backtab_reverse_cycles_fields() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::BackTab, IpcKeyModifiers::default()));
        assert_eq!(app.state.field_focus_idx, 3);
        assert_eq!(app.state.edit_field, EditField::Body);
    }

    #[test]
    fn handle_key_char_inserts_at_cursor() {
        let mut app = base_app();
        app.state.field_focus_idx = 1; // Url
        app.state.edit_field = EditField::Url;
        assert!(app.handle_key(IpcKey::Char('h'), IpcKeyModifiers::default()));
        assert!(app.handle_key(IpcKey::Char('i'), IpcKeyModifiers::default()));
        assert_eq!(app.state.url, "hi");
        assert_eq!(app.state.edit_cursor, 2);
    }

    #[test]
    fn handle_key_backspace_deletes() {
        let mut app = base_app();
        app.state.field_focus_idx = 1;
        app.state.edit_field = EditField::Url;
        app.state.url = "abc".into();
        app.state.edit_cursor = 3;
        assert!(app.handle_key(IpcKey::Backspace, IpcKeyModifiers::default()));
        assert_eq!(app.state.url, "ab");
        assert_eq!(app.state.edit_cursor, 2);
    }

    #[test]
    fn handle_key_ctrl_s_triggers_send() {
        let mut app = base_app();
        app.state.url = "http://localhost:0".into();
        let mods = IpcKeyModifiers {
            ctrl: true,
            shift: false,
            alt: false,
        };
        assert!(app.handle_key(IpcKey::Char('s'), mods));
        assert!(matches!(app.state.fetch_state, FetchState::Sending));
        assert!(app.rx_fetch.is_some());
    }

    #[test]
    fn handle_key_ctrl_shift_s_saves() {
        let mut app = base_app();
        app.state.url = "https://example.com".into();
        let mods = IpcKeyModifiers {
            ctrl: true,
            shift: true,
            alt: false,
        };
        assert!(app.handle_key(IpcKey::Char('S'), mods));
        assert_eq!(app.state.saved_requests.len(), 1);
        assert_eq!(app.state.saved_requests[0].url, "https://example.com");
    }

    #[test]
    fn handle_key_m_opens_method_picker() {
        let mut app = base_app();
        assert_eq!(app.state.screen, Screen::Editor);
        assert_eq!(app.state.edit_field, EditField::Method);
        assert!(app.handle_key(IpcKey::Char('M'), IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::MethodPicker);
    }

    #[test]
    fn handle_key_enter_on_method_opens_picker() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Enter, IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::MethodPicker);
    }

    #[test]
    fn handle_key_h_opens_history() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('h'), IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::History);
    }

    #[test]
    fn handle_key_shift_s_opens_saved() {
        let mut app = base_app();
        let mods = IpcKeyModifiers {
            ctrl: false,
            shift: true,
            alt: false,
        };
        assert!(app.handle_key(IpcKey::Char('S'), mods));
        assert_eq!(app.state.screen, Screen::Saved);
    }

    #[test]
    fn handle_key_enter_on_history_loads_request() {
        let mut app = base_app();
        app.state.history.push(RequestEntry {
            method: "POST".into(),
            url: "https://api.example.com/data".into(),
            headers: "Content-Type: application/json".into(),
            body: r#"{"test":true}"#.into(),
        });
        app.state.screen = Screen::History;
        app.state.history_cursor = 0;
        assert!(app.handle_key(IpcKey::Enter, IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::Editor);
        assert_eq!(app.state.url, "https://api.example.com/data");
        assert_eq!(app.state.method, HttpMethod::POST);
        assert_eq!(app.state.headers_text, "Content-Type: application/json");
        assert_eq!(app.state.body_text, r#"{"test":true}"#);
    }

    #[test]
    fn handle_key_enter_on_saved_loads_request() {
        let mut app = base_app();
        app.state.saved_requests.push(RequestEntry {
            method: "PUT".into(),
            url: "https://api.example.com/update".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.screen = Screen::Saved;
        app.state.saved_cursor = 0;
        assert!(app.handle_key(IpcKey::Enter, IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::Editor);
        assert_eq!(app.state.method, HttpMethod::PUT);
        assert_eq!(app.state.url, "https://api.example.com/update");
    }

    #[test]
    fn handle_key_esc_on_editor_not_consumed() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Esc, IpcKeyModifiers::default()));
    }

    #[test]
    fn handle_key_esc_on_history_returns_to_editor() {
        let mut app = base_app();
        app.state.screen = Screen::History;
        assert!(app.handle_key(IpcKey::Esc, IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::Editor);
    }

    #[test]
    fn handle_key_esc_on_saved_returns_to_editor() {
        let mut app = base_app();
        app.state.screen = Screen::Saved;
        assert!(app.handle_key(IpcKey::Esc, IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::Editor);
    }

    #[test]
    fn handle_key_esc_on_response_returns_to_editor() {
        let mut app = base_app();
        app.state.screen = Screen::Response;
        assert!(app.handle_key(IpcKey::Esc, IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::Editor);
    }

    #[test]
    fn handle_key_esc_on_picker_returns_to_editor() {
        let mut app = base_app();
        app.state.screen = Screen::MethodPicker;
        assert!(app.handle_key(IpcKey::Esc, IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::Editor);
    }

    #[test]
    fn handle_key_up_down_in_history() {
        let mut app = base_app();
        app.state.history.push(RequestEntry {
            method: "GET".into(),
            url: "a".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.history.push(RequestEntry {
            method: "GET".into(),
            url: "b".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.screen = Screen::History;
        app.state.history_cursor = 0;
        assert!(app.handle_key(IpcKey::Down, IpcKeyModifiers::default()));
        assert_eq!(app.state.history_cursor, 1);
        assert!(app.handle_key(IpcKey::Down, IpcKeyModifiers::default()));
        assert_eq!(app.state.history_cursor, 1); // clamped
        assert!(app.handle_key(IpcKey::Up, IpcKeyModifiers::default()));
        assert_eq!(app.state.history_cursor, 0);
        assert!(app.handle_key(IpcKey::Up, IpcKeyModifiers::default()));
        assert_eq!(app.state.history_cursor, 0); // clamped
    }

    #[test]
    fn handle_key_up_down_in_saved() {
        let mut app = base_app();
        app.state.saved_requests.push(RequestEntry {
            method: "GET".into(),
            url: "a".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.saved_requests.push(RequestEntry {
            method: "GET".into(),
            url: "b".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.screen = Screen::Saved;
        app.state.saved_cursor = 0;
        assert!(app.handle_key(IpcKey::Down, IpcKeyModifiers::default()));
        assert_eq!(app.state.saved_cursor, 1);
        assert!(app.handle_key(IpcKey::Up, IpcKeyModifiers::default()));
        assert_eq!(app.state.saved_cursor, 0);
    }

    #[test]
    fn handle_key_d_deletes_history_entry() {
        let mut app = base_app();
        app.state.history.push(RequestEntry {
            method: "GET".into(),
            url: "a".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.history.push(RequestEntry {
            method: "GET".into(),
            url: "b".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.screen = Screen::History;
        app.state.history_cursor = 0;
        assert!(app.handle_key(IpcKey::Char('d'), IpcKeyModifiers::default()));
        assert_eq!(app.state.history.len(), 1);
        assert_eq!(app.state.history[0].url, "b");
    }

    #[test]
    fn handle_key_d_deletes_saved_entry() {
        let mut app = base_app();
        app.state.saved_requests.push(RequestEntry {
            method: "GET".into(),
            url: "a".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.saved_requests.push(RequestEntry {
            method: "GET".into(),
            url: "b".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.screen = Screen::Saved;
        app.state.saved_cursor = 1;
        assert!(app.handle_key(IpcKey::Char('d'), IpcKeyModifiers::default()));
        assert_eq!(app.state.saved_requests.len(), 1);
        assert_eq!(app.state.saved_requests[0].url, "a");
        assert_eq!(app.state.saved_cursor, 0);
    }

    #[test]
    fn handle_key_picker_up_down_and_select() {
        let mut app = base_app();
        app.state.screen = Screen::MethodPicker;
        app.state.picker_cursor = 0;
        assert!(app.handle_key(IpcKey::Down, IpcKeyModifiers::default()));
        assert_eq!(app.state.picker_cursor, 1);
        assert!(app.handle_key(IpcKey::Up, IpcKeyModifiers::default()));
        assert_eq!(app.state.picker_cursor, 0);
        assert!(app.handle_key(IpcKey::Enter, IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::Editor);
        assert_eq!(app.state.method, HttpMethod::GET);
    }

    #[test]
    fn handle_key_picker_enter_selects_post() {
        let mut app = base_app();
        app.state.screen = Screen::MethodPicker;
        app.state.picker_cursor = 1; // POST
        assert!(app.handle_key(IpcKey::Enter, IpcKeyModifiers::default()));
        assert_eq!(app.state.method, HttpMethod::POST);
    }

    #[test]
    fn handle_tick_drains_response_done_adds_history() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        app.state.url = "https://example.com".into();
        app.state.method = HttpMethod::GET;
        let _ = tx.send(FetchMsg::ResponseDone(HttpResponse {
            status: 200,
            status_text: "OK".into(),
            headers: vec![],
            body: "{}".into(),
            elapsed_ms: 100,
            body_truncated: false,
        }));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Done));
        assert_eq!(app.state.screen, Screen::Response);
        assert_eq!(app.state.history.len(), 1);
        assert_eq!(app.state.history[0].url, "https://example.com");
    }

    #[test]
    fn handle_tick_drains_response_error() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        let _ = tx.send(FetchMsg::ResponseError("timeout".into()));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Error(_)));
        assert_eq!(app.state.screen, Screen::Response);
    }

    #[test]
    fn handle_tick_history_capped_at_50() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        for i in 0..60 {
            app.state.history.push(RequestEntry {
                method: "GET".into(),
                url: format!("url{}", i),
                headers: String::new(),
                body: String::new(),
            });
        }
        let _ = tx.send(FetchMsg::ResponseDone(HttpResponse {
            status: 200,
            status_text: "OK".into(),
            headers: vec![],
            body: "{}".into(),
            elapsed_ms: 0,
            body_truncated: false,
        }));
        app.handle_tick();
        assert!(app.state.history.len() <= HISTORY_MAX);
    }

    #[test]
    fn handle_db_value_loads_history_and_saved() {
        let mut app = base_app();
        let json = serde_json::json!({
            "history": [
                {"method": "GET", "url": "https://hist.example.com", "headers": "", "body": ""}
            ],
            "saved_requests": [
                {"method": "POST", "url": "https://saved.example.com", "headers": "X: 1", "body": "{}"}
            ]
        });
        app.handle_db_value("http-client", Some(json.to_string()));
        assert_eq!(app.state.history.len(), 1);
        assert_eq!(app.state.history[0].url, "https://hist.example.com");
        assert_eq!(app.state.saved_requests.len(), 1);
        assert_eq!(app.state.saved_requests[0].url, "https://saved.example.com");
    }

    #[test]
    fn handle_db_value_none_defaults_empty() {
        let mut app = base_app();
        app.state.history.push(RequestEntry {
            method: "GET".into(),
            url: "x".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.handle_db_value("http-client", None);
        assert_eq!(app.state.history.len(), 1); // unchanged when nil
                                                // But wrong key is ignored
        app.handle_db_value("other-key", Some("{}".into()));
        assert_eq!(app.state.history.len(), 1);
    }

    #[test]
    fn handle_db_value_wrong_key_ignored() {
        let mut app = base_app();
        app.state.history.push(RequestEntry {
            method: "GET".into(),
            url: "x".into(),
            headers: String::new(),
            body: String::new(),
        });
        let json = serde_json::json!({
            "history": [{"method": "PUT", "url": "y", "headers": "", "body": ""}]
        });
        app.handle_db_value("weather", Some(json.to_string()));
        assert_eq!(app.state.history.len(), 1);
        assert_eq!(app.state.history[0].url, "x");
    }

    #[test]
    fn schedule_db_save_sets_pending_request() {
        let mut app = base_app();
        app.state.history.push(RequestEntry {
            method: "GET".into(),
            url: "https://db.example.com".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.schedule_db_save();
        assert!(app.pending_request.is_some());
    }

    #[test]
    fn palette_command_0_opens_editor() {
        let mut app = base_app();
        app.state.screen = Screen::History;
        app.handle_palette_command(0);
        assert_eq!(app.state.screen, Screen::Editor);
    }

    #[test]
    fn palette_command_1_sends_request() {
        let mut app = base_app();
        app.state.url = "http://localhost:0".into();
        app.handle_palette_command(1);
        assert!(matches!(app.state.fetch_state, FetchState::Sending));
    }

    #[test]
    fn palette_command_2_opens_history() {
        let mut app = base_app();
        app.handle_palette_command(2);
        assert_eq!(app.state.screen, Screen::History);
    }

    #[test]
    fn handle_key_e_on_response_returns_to_editor() {
        let mut app = base_app();
        app.state.screen = Screen::Response;
        assert!(app.handle_key(IpcKey::Char('e'), IpcKeyModifiers::default()));
        assert_eq!(app.state.screen, Screen::Editor);
    }

    #[test]
    fn handle_key_s_on_response_saves() {
        let mut app = base_app();
        app.state.url = "https://example.com".into();
        app.state.screen = Screen::Response;
        assert!(app.handle_key(IpcKey::Char('s'), IpcKeyModifiers::default()));
        assert_eq!(app.state.saved_requests.len(), 1);
    }

    #[test]
    fn handle_key_left_right_in_url() {
        let mut app = base_app();
        app.state.field_focus_idx = 1;
        app.state.edit_field = EditField::Url;
        app.state.url = "abc".into();
        app.state.edit_cursor = 0;
        assert!(app.handle_key(IpcKey::Right, IpcKeyModifiers::default()));
        assert_eq!(app.state.edit_cursor, 1);
        assert!(app.handle_key(IpcKey::Right, IpcKeyModifiers::default()));
        assert_eq!(app.state.edit_cursor, 2);
        assert!(app.handle_key(IpcKey::Left, IpcKeyModifiers::default()));
        assert_eq!(app.state.edit_cursor, 1);
    }

    #[test]
    fn handle_key_s_on_history_saves_to_saved() {
        let mut app = base_app();
        app.state.history.push(RequestEntry {
            method: "GET".into(),
            url: "https://a.com".into(),
            headers: String::new(),
            body: String::new(),
        });
        app.state.screen = Screen::History;
        app.state.history_cursor = 0;
        assert!(app.handle_key(IpcKey::Char('S'), IpcKeyModifiers::default()));
        assert_eq!(app.state.saved_requests.len(), 1);
        assert_eq!(app.state.saved_requests[0].url, "https://a.com");
    }

    #[test]
    fn parse_headers_parses_key_value() {
        let result = parse_headers("Content-Type: application/json\nAuthorization: Bearer xyz");
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            ("Content-Type".to_string(), "application/json".to_string())
        );
        assert_eq!(
            result[1],
            ("Authorization".to_string(), "Bearer xyz".to_string())
        );
    }

    #[test]
    fn parse_headers_ignores_empty_lines() {
        let result = parse_headers("Content-Type: json\n\nX-Custom: val");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_headers_empty_input() {
        let result = parse_headers("");
        assert!(result.is_empty());
    }

    #[test]
    fn load_entry_populates_editor_fields() {
        let mut s = ClientState::default();
        let entry = RequestEntry {
            method: "POST".into(),
            url: "https://test.com".into(),
            headers: "X-Test: 1".into(),
            body: r#"{"a":1}"#.into(),
        };
        load_entry(&mut s, &entry);
        assert_eq!(s.method, HttpMethod::POST);
        assert_eq!(s.url, "https://test.com");
        assert_eq!(s.headers_text, "X-Test: 1");
        assert_eq!(s.body_text, r#"{"a":1}"#);
        assert_eq!(s.edit_field, EditField::Url);
        assert_eq!(s.field_focus_idx, 1);
    }

    #[test]
    fn handle_key_char_at_method_field_does_not_insert() {
        let mut app = base_app();
        app.state.field_focus_idx = 0;
        app.state.edit_field = EditField::Method;
        let consumed = app.handle_key(IpcKey::Char('x'), IpcKeyModifiers::default());
        assert_eq!(app.state.url, "");
        assert!(!consumed);
    }
}
