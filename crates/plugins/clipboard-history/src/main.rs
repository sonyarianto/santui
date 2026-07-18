mod clipboard;
mod state;
mod ui;

use std::io::{BufRead, BufReader};
use std::sync::mpsc;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};

use clipboard::ClipMsg;
use state::{ClipState, Screen};
use ui::render_ui;

const COPY_FLASH_TICKS: u8 = 15;

struct App {
    state: ClipState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    rx_clip: Option<mpsc::Receiver<ClipMsg>>,
    copy_flash_ticks: u8,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    fn new() -> Self {
        let (tx, rx_clip) = mpsc::channel();
        clipboard::spawn_clipboard_watcher(tx);
        Self {
            state: ClipState::default(),
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
            pending_request: None,
            rx_clip: Some(rx_clip),
            copy_flash_ticks: 0,
        }
    }

    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.pending_request = Some(PluginRequest::DbGet {
            key: "clipboard".into(),
        });
        self.dirty = true;
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        match &self.state.screen {
            Screen::List => self.handle_list_key(key),
            Screen::View(_) => self.handle_view_key(key),
        }
    }

    fn handle_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char('v') => {
                if let Some(&idx) = self.state.filtered.get(self.state.cursor) {
                    self.state.screen = Screen::View(idx);
                    self.state.view_scroll = 0;
                    self.dirty = true;
                }
                true
            }
            IpcKey::Char('d') => {
                self.state.delete_selected();
                self.schedule_db_save();
                self.dirty = true;
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.cursor = self.state.cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.filtered.len().saturating_sub(1);
                self.state.cursor = self.state.cursor.min(max).saturating_add(1).min(max);
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                let entry_id = self
                    .state
                    .selected_entry()
                    .map(|e| (e.id, e.content.clone()));
                if let Some((id, content)) = entry_id {
                    self.state.last_copied_id = Some(id);
                    self.copy_flash_ticks = COPY_FLASH_TICKS;
                    if let Err(e) = clipboard::set_clipboard(&content) {
                        self.state.clipboard_error = Some(e);
                    }
                    self.dirty = true;
                }
                true
            }
            IpcKey::Backspace => {
                self.state.search_query.pop();
                self.state.apply_filter();
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                if self.state.search_query.is_empty() {
                    return false;
                }
                self.state.search_query.clear();
                self.state.apply_filter();
                self.dirty = true;
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.state.search_query.push(c);
                self.state.apply_filter();
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_view_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.view_scroll = self.state.view_scroll.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.state.view_scroll = self.state.view_scroll.saturating_add(1);
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                let idx = match self.state.screen {
                    Screen::View(idx) => idx,
                    _ => unreachable!(),
                };
                if let Some(entry) = self.state.entries.get(idx) {
                    self.state.last_copied_id = Some(entry.id);
                    self.copy_flash_ticks = COPY_FLASH_TICKS;
                    if let Err(e) = clipboard::set_clipboard(&entry.content) {
                        self.state.clipboard_error = Some(e);
                    }
                }
                self.state.screen = Screen::List;
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::List;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_tick(&mut self) {
        if let Some(ref rx) = self.rx_clip {
            loop {
                match rx.try_recv() {
                    Ok(ClipMsg::NewContent(text)) => {
                        let ts = now_secs();
                        self.state.push(text, ts);
                        self.pending_request = Some(PluginRequest::DbSet {
                            key: "clipboard".into(),
                            value: self.state.serialize(),
                        });
                        self.dirty = true;
                    }
                    Ok(ClipMsg::Error(e)) => {
                        self.state.clipboard_error = Some(e);
                        self.dirty = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.state.clipboard_error = Some("Clipboard watcher stopped".into());
                        break;
                    }
                }
            }
        }

        if self.copy_flash_ticks > 0 {
            self.copy_flash_ticks -= 1;
            if self.copy_flash_ticks == 0 {
                self.state.last_copied_id = None;
            }
            self.dirty = true;
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "clipboard" {
            if let Some(json) = value {
                self.state.load(&json);
            }
            self.dirty = true;
        }
    }

    fn schedule_db_save(&mut self) {
        self.pending_request = Some(PluginRequest::DbSet {
            key: "clipboard".into(),
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
                self.state.entries.clear();
                self.state.apply_filter();
                self.schedule_db_save();
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        ui::help_text(&self.state)
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(&self.state, &self.theme, self.area.w, self.area.h);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn palette_commands() -> Vec<(String, String)> {
    vec![
        ("Plugins".into(), "Show history".into()),
        ("Plugins".into(), "Clear history".into()),
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
                        log::error!("[clipboard-history] parse error: {e}: {line}");
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
    use state::{ClipEntry, Screen};

    fn base_app() -> App {
        App::default()
    }

    #[test]
    fn handle_key_enter_triggers_copy() {
        let mut app = base_app();
        app.state
            .entries
            .push(ClipEntry::new("test content".into(), 100));
        app.state.apply_filter();
        app.state.cursor = 0;
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.last_copied_id, Some(100));
        assert_eq!(app.copy_flash_ticks, COPY_FLASH_TICKS);
    }

    #[test]
    fn handle_key_d_deletes_entry() {
        let mut app = base_app();
        app.state.push("test content".into(), 100);
        app.state.apply_filter();
        assert_eq!(app.state.entries.len(), 1);
        assert!(app.handle_key(IpcKey::Char('d')));
        assert!(app.state.entries.is_empty());
        assert!(app.pending_request.is_some());
    }

    #[test]
    fn handle_key_v_opens_view() {
        let mut app = base_app();
        app.state.push("test content".into(), 100);
        app.state.apply_filter();
        assert!(app.handle_key(IpcKey::Char('v')));
        assert!(matches!(app.state.screen, Screen::View(0)));
    }

    #[test]
    fn handle_key_esc_empty_search_not_consumed() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_esc_nonempty_search_clears() {
        let mut app = base_app();
        app.state.search_query = "test".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(app.state.search_query.is_empty());
    }

    #[test]
    fn handle_tick_drains_new_content() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_clip = Some(rx);
        let _ = tx.send(ClipMsg::NewContent("test content".into()));
        assert_eq!(app.state.entries.len(), 0);
        app.handle_tick();
        assert_eq!(app.state.entries.len(), 1);
        assert_eq!(app.state.entries[0].content, "test content");
    }

    #[test]
    fn handle_tick_decrements_copy_flash() {
        let mut app = base_app();
        app.copy_flash_ticks = 5;
        app.handle_tick();
        assert_eq!(app.copy_flash_ticks, 4);
    }

    #[test]
    fn handle_tick_clears_last_copied_id_on_flash_end() {
        let mut app = base_app();
        app.state.last_copied_id = Some(100);
        app.copy_flash_ticks = 1;
        app.handle_tick();
        assert_eq!(app.copy_flash_ticks, 0);
        assert!(app.state.last_copied_id.is_none());
    }

    #[test]
    fn handle_tick_handles_disconnected_watcher() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_clip = Some(rx);
        drop(tx);
        app.handle_tick();
        assert!(app.state.clipboard_error.is_some());
    }

    #[test]
    fn handle_db_value_loads_entries() {
        let mut app = base_app();
        let json = r#"[{"id":100,"content":"hello","preview":"hello"}]"#;
        app.handle_db_value("clipboard", Some(json.into()));
        assert_eq!(app.state.entries.len(), 1);
        assert_eq!(app.state.entries[0].content, "hello");
    }

    #[test]
    fn handle_db_value_none_starts_empty() {
        let mut app = base_app();
        app.handle_db_value("clipboard", None);
        assert!(app.state.entries.is_empty());
    }

    #[test]
    fn palette_command_1_clears_history() {
        let mut app = base_app();
        app.state.push("test".into(), 100);
        app.handle_palette_command(1);
        assert!(app.state.entries.is_empty());
        assert!(app.pending_request.is_some());
    }
}
