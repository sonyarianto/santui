mod fetcher;
mod state;
mod ui;

use std::io::{BufRead, BufReader};
use std::sync::mpsc;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};

use fetcher::{spawn_fetch, FetchMsg};
use state::{FetchStatus, RssState, Screen, REFRESH_TICKS};
use ui::render_ui;

struct App {
    state: RssState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    tx_fetch: mpsc::Sender<FetchMsg>,
    rx_fetch: mpsc::Receiver<FetchMsg>,
}

impl Default for App {
    fn default() -> Self {
        let (tx_fetch, rx_fetch) = mpsc::channel();
        Self {
            state: RssState::new(),
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
            pending_request: Some(PluginRequest::DbGet { key: "rss".into() }),
            tx_fetch,
            rx_fetch,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.dirty = true;
        match self.state.screen.clone() {
            Screen::FeedList => self.handle_feed_list_key(key),
            Screen::ItemList(_) => self.handle_item_list_key(key),
            Screen::ItemView(idx) => self.handle_item_view_key(key, idx),
            Screen::AddFeed => self.handle_add_feed_key(key),
            Screen::ConfirmRemoveFeed(idx) => self.handle_confirm_remove_key(key, idx),
        }
    }

    fn handle_feed_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.feed_cursor = self.state.feed_cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.data.feeds.len();
                self.state.feed_cursor = self.state.feed_cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Enter => {
                if self.state.feed_cursor == 0 {
                    self.state.screen = Screen::ItemList(None);
                } else {
                    let feed_url = self.state.data.feeds[self.state.feed_cursor - 1]
                        .url
                        .clone();
                    self.state.screen = Screen::ItemList(Some(feed_url));
                }
                self.state.item_cursor = 0;
                self.state.rebuild_current_items(&self.state.screen.clone());
                true
            }
            IpcKey::Char('a') => {
                self.state.screen = Screen::AddFeed;
                self.state.add_url_buf.clear();
                true
            }
            IpcKey::Char('d') => {
                if self.state.feed_cursor > 0 {
                    self.state.screen = Screen::ConfirmRemoveFeed(self.state.feed_cursor - 1);
                }
                true
            }
            IpcKey::Char('r') => {
                self.trigger_refresh_all();
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_item_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.item_cursor = self.state.item_cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.current_items.len().saturating_sub(1);
                self.state.item_cursor = self.state.item_cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Enter => {
                let item_id = self
                    .state
                    .current_items
                    .get(self.state.item_cursor)
                    .map(|i| i.item.id.clone());
                if let Some(ref id) = item_id {
                    self.state.screen = Screen::ItemView(self.state.item_cursor);
                    self.state.mark_read(id);
                    self.schedule_db_save();
                }
                true
            }
            IpcKey::Char('o') => {
                if let Some(item) = self.state.current_items.get(self.state.item_cursor) {
                    if let Some(ref url) = item.item.url {
                        open_url(url);
                    }
                }
                true
            }
            IpcKey::Char('m') => {
                let item_id = self
                    .state
                    .current_items
                    .get(self.state.item_cursor)
                    .map(|i| i.item.id.clone());
                if let Some(ref id) = item_id {
                    self.state.mark_read(id);
                    self.schedule_db_save();
                }
                true
            }
            IpcKey::Char('r') => {
                self.trigger_refresh_all();
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::FeedList;
                self.state.feed_cursor = 0;
                true
            }
            _ => false,
        }
    }

    fn handle_item_view_key(&mut self, key: IpcKey, idx: usize) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.item_scroll = self.state.item_scroll.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.state.item_scroll += 1;
                true
            }
            IpcKey::Char('o') => {
                if let Some(item) = self.state.current_items.get(idx) {
                    if let Some(ref url) = item.item.url {
                        open_url(url);
                    }
                }
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::ItemList(
                    self.state
                        .current_items
                        .get(idx)
                        .map(|i| i.feed_url.clone()),
                );
                self.state.item_scroll = 0;
                true
            }
            _ => false,
        }
    }

    fn handle_add_feed_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char(c) => {
                self.state.add_url_buf.push(c);
                true
            }
            IpcKey::Backspace => {
                self.state.add_url_buf.pop();
                true
            }
            IpcKey::Enter => {
                let url = self.state.add_url_buf.clone();
                if !url.is_empty() {
                    self.state.add_feed(url);
                    self.trigger_refresh_all();
                    self.schedule_db_save();
                }
                self.state.screen = Screen::FeedList;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::FeedList;
                true
            }
            _ => true,
        }
    }

    fn handle_confirm_remove_key(&mut self, key: IpcKey, idx: usize) -> bool {
        match key {
            IpcKey::Char('y') => {
                self.state.remove_feed(idx);
                self.state.screen = Screen::FeedList;
                self.state.feed_cursor = 0;
                self.schedule_db_save();
                true
            }
            IpcKey::Char('n') | IpcKey::Esc => {
                self.state.screen = Screen::FeedList;
                true
            }
            _ => true,
        }
    }

    fn handle_tick(&mut self) {
        loop {
            match self.rx_fetch.try_recv() {
                Ok(FetchMsg::FeedDone { url, items }) => {
                    self.state.apply_feed_items(&url, items, None);
                    self.state.rebuild_current_items(&self.state.screen.clone());
                    self.decrement_fetch_counter();
                    self.schedule_db_save();
                    self.dirty = true;
                }
                Ok(FetchMsg::FeedError { url, error }) => {
                    log::warn!("feed fetch error for {url}: {error}");
                    self.decrement_fetch_counter();
                    self.dirty = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }

        self.state.ticks_since_refresh += 1;
        if self.state.ticks_since_refresh >= REFRESH_TICKS
            && matches!(self.state.fetch_status, FetchStatus::Idle)
            && !self.state.data.feeds.is_empty()
        {
            self.trigger_refresh_all();
            self.state.ticks_since_refresh = 0;
        }
    }

    fn trigger_refresh_all(&mut self) {
        let count = self.state.data.feeds.len();
        self.state.fetch_status = FetchStatus::Fetching(count);
        for feed in &self.state.data.feeds {
            spawn_fetch(feed.url.clone(), self.tx_fetch.clone());
        }
    }

    fn decrement_fetch_counter(&mut self) {
        if let FetchStatus::Fetching(n) = self.state.fetch_status {
            if n <= 1 {
                self.state.fetch_status = FetchStatus::Idle;
            } else {
                self.state.fetch_status = FetchStatus::Fetching(n - 1);
            }
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "rss" {
            if let Some(json) = value {
                self.state.load(&json);
            }
            self.dirty = true;
            if !self.state.data.feeds.is_empty() {
                self.trigger_refresh_all();
            }
        }
    }

    fn schedule_db_save(&mut self) {
        self.pending_request = Some(PluginRequest::DbSet {
            key: "rss".into(),
            value: self.state.serialize(),
        });
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.state.screen = Screen::FeedList;
                self.dirty = true;
            }
            1 => {
                self.state.screen = Screen::AddFeed;
                self.state.add_url_buf.clear();
                self.dirty = true;
            }
            2 => {
                self.trigger_refresh_all();
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match &self.state.screen {
            Screen::FeedList => vec![
                ("enter".into(), "open".into()),
                ("a".into(), "add feed".into()),
                ("d".into(), "remove".into()),
                ("r".into(), "refresh".into()),
            ],
            Screen::ItemList(_) => vec![
                ("enter".into(), "read".into()),
                ("o".into(), "open in browser".into()),
                ("m".into(), "mark read".into()),
                ("esc".into(), "back".into()),
            ],
            Screen::ItemView(_) => vec![
                ("o".into(), "open in browser".into()),
                ("↑↓".into(), "scroll".into()),
                ("esc".into(), "back".into()),
            ],
            Screen::AddFeed => vec![
                ("enter".into(), "add".into()),
                ("esc".into(), "cancel".into()),
            ],
            Screen::ConfirmRemoveFeed(_) => vec![
                ("y".into(), "confirm".into()),
                ("n".into(), "cancel".into()),
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

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .spawn();
}

fn palette_commands() -> Vec<(String, String)> {
    vec![
        ("Plugins".into(), "View feeds".into()),
        ("Plugins".into(), "Add feed".into()),
        ("Plugins".into(), "Refresh all feeds".into()),
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
                        log::error!("[rss-reader] parse error: {e}: {line}");
                        continue;
                    }
                };

                match msg {
                    HostMsg::Init { theme, area, .. } => {
                        app.theme = theme;
                        app.area = area;
                        app.dirty = true;
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
    use crate::fetcher::FeedItem;
    use crate::state::Feed;

    fn base_app() -> App {
        let mut app = App::default();
        app.state.data.feeds.push(Feed {
            url: "http://feed1".into(),
            title: "Feed 1".into(),
            last_fetched: None,
        });
        app.state.data.feeds.push(Feed {
            url: "http://feed2".into(),
            title: "Feed 2".into(),
            last_fetched: None,
        });
        app
    }

    fn app_with_items() -> App {
        let mut app = base_app();
        app.state.apply_feed_items(
            "http://feed1",
            vec![
                FeedItem {
                    id: "a".into(),
                    title: "A".into(),
                    summary: "s".into(),
                    url: None,
                    published: Some(100),
                },
                FeedItem {
                    id: "b".into(),
                    title: "B".into(),
                    summary: "s".into(),
                    url: None,
                    published: Some(200),
                },
            ],
            None,
        );
        app.state.apply_feed_items(
            "http://feed2",
            vec![FeedItem {
                id: "c".into(),
                title: "C".into(),
                summary: "s".into(),
                url: None,
                published: Some(300),
            }],
            None,
        );
        app
    }

    #[test]
    fn handle_key_enter_opens_all_items_at_cursor_0() {
        let mut app = base_app();
        app.state.feed_cursor = 0;
        assert!(app.handle_key(IpcKey::Enter));
        assert!(matches!(app.state.screen, Screen::ItemList(None)));
    }

    #[test]
    fn handle_key_enter_opens_feed_items_at_cursor_1() {
        let mut app = base_app();
        app.state.feed_cursor = 1;
        assert!(app.handle_key(IpcKey::Enter));
        assert!(
            matches!(app.state.screen, Screen::ItemList(Some(ref url)) if url == "http://feed1")
        );
    }

    #[test]
    fn handle_key_a_opens_add_feed() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('a')));
        assert!(matches!(app.state.screen, Screen::AddFeed));
        assert!(app.state.add_url_buf.is_empty());
    }

    #[test]
    fn handle_key_enter_in_add_feed_adds_and_fetches() {
        let mut app = base_app();
        app.state.screen = Screen::AddFeed;
        app.state.add_url_buf = "http://feed3".into();
        assert!(app.handle_key(IpcKey::Enter));
        assert!(matches!(app.state.screen, Screen::FeedList));
        assert_eq!(app.state.data.feeds.len(), 3);
        assert!(matches!(app.state.fetch_status, FetchStatus::Fetching(_)));
    }

    #[test]
    fn handle_key_esc_in_add_feed_cancels() {
        let mut app = base_app();
        app.state.screen = Screen::AddFeed;
        assert!(app.handle_key(IpcKey::Esc));
        assert!(matches!(app.state.screen, Screen::FeedList));
    }

    #[test]
    fn handle_key_d_opens_confirm_remove() {
        let mut app = base_app();
        app.state.feed_cursor = 1;
        assert!(app.handle_key(IpcKey::Char('d')));
        assert!(matches!(app.state.screen, Screen::ConfirmRemoveFeed(0)));
    }

    #[test]
    fn handle_key_y_removes_feed() {
        let mut app = base_app();
        app.state.screen = Screen::ConfirmRemoveFeed(0);
        assert!(app.handle_key(IpcKey::Char('y')));
        assert!(matches!(app.state.screen, Screen::FeedList));
        assert_eq!(app.state.data.feeds.len(), 1);
    }

    #[test]
    fn handle_key_m_marks_read() {
        let mut app = app_with_items();
        app.state.screen = Screen::ItemList(Some("http://feed1".into()));
        app.state.rebuild_current_items(&app.state.screen.clone());
        app.state.item_cursor = 0;
        assert!(!app.state.current_items[0].is_read);
        assert!(app.handle_key(IpcKey::Char('m')));
        assert!(app.state.current_items[0].is_read);
    }

    #[test]
    fn handle_key_esc_on_feed_list_not_consumed() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_esc_on_item_list_returns_to_feed_list() {
        let mut app = base_app();
        app.state.screen = Screen::ItemList(Some("http://feed1".into()));
        assert!(app.handle_key(IpcKey::Esc));
        assert!(matches!(app.state.screen, Screen::FeedList));
    }

    #[test]
    fn handle_key_esc_on_item_view_returns_to_item_list() {
        let mut app = app_with_items();
        app.state.screen = Screen::ItemList(Some("http://feed1".into()));
        app.state.rebuild_current_items(&app.state.screen.clone());
        app.state.screen = Screen::ItemView(0);
        assert!(app.handle_key(IpcKey::Esc));
        assert!(
            matches!(app.state.screen, Screen::ItemList(Some(ref url)) if url == "http://feed1")
        );
        assert_eq!(app.state.item_scroll, 0);
    }

    #[test]
    fn handle_tick_drains_feed_done() {
        let mut app = base_app();
        app.state.fetch_status = FetchStatus::Fetching(1);
        let _ = app.tx_fetch.send(FetchMsg::FeedDone {
            url: "http://feed1".into(),
            items: vec![FeedItem {
                id: "x".into(),
                title: "X".into(),
                summary: "s".into(),
                url: None,
                published: Some(1),
            }],
        });
        app.handle_tick();
        assert_eq!(app.state.all_items.len(), 1);
        assert!(matches!(app.state.fetch_status, FetchStatus::Idle));
    }

    #[test]
    fn handle_tick_drains_feed_error_gracefully() {
        let mut app = base_app();
        app.state.fetch_status = FetchStatus::Fetching(1);
        let _ = app.tx_fetch.send(FetchMsg::FeedError {
            url: "http://feed1".into(),
            error: "network error".into(),
        });
        app.handle_tick();
        assert!(matches!(app.state.fetch_status, FetchStatus::Idle));
    }

    #[test]
    fn handle_tick_triggers_refresh_at_threshold() {
        let mut app = base_app();
        app.state.ticks_since_refresh = REFRESH_TICKS - 1;
        app.handle_tick();
        assert!(matches!(app.state.fetch_status, FetchStatus::Fetching(_)));
        assert_eq!(app.state.ticks_since_refresh, 0);
    }

    #[test]
    fn handle_db_value_loads_data_and_triggers_fetch() {
        let mut app = App::default();
        let json = serde_json::json!({
            "feeds": [{"url": "http://loaded", "title": "Loaded", "last_fetched": null}],
            "read_state": {"read_ids": []}
        });
        app.handle_db_value("rss", Some(json.to_string()));
        assert_eq!(app.state.data.feeds.len(), 1);
        assert!(matches!(app.state.fetch_status, FetchStatus::Fetching(_)));
    }

    #[test]
    fn palette_command_1_opens_add_feed() {
        let mut app = base_app();
        app.handle_palette_command(1);
        assert!(matches!(app.state.screen, Screen::AddFeed));
    }

    #[test]
    fn palette_command_2_triggers_refresh() {
        let mut app = base_app();
        app.state.fetch_status = FetchStatus::Idle;
        app.handle_palette_command(2);
        assert!(matches!(app.state.fetch_status, FetchStatus::Fetching(_)));
    }
}
