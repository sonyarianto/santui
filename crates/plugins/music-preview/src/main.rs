mod api;
mod state;
mod ui;

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginMessage, PluginRequest, RenderCmd, ThemeData,
};

use state::{FetchState, MusicState};
use ui::{max_visible_tracks, render_ui};

enum FetchMsg {
    SearchDone(Vec<api::ItunesTrack>),
    SearchError(String),
}

struct App {
    state: MusicState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    pending_plugin_message: Option<PluginMessage>,
    rx_fetch: Option<mpsc::Receiver<FetchMsg>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: MusicState::default(),
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
            pending_plugin_message: None,
            rx_fetch: None,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Backspace => {
                self.state.query.pop();
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                let q = self.state.query.trim().to_string();
                if !q.is_empty() {
                    self.trigger_search(&q);
                }
                self.dirty = true;
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.selected = self.state.selected.saturating_sub(1);
                self.adjust_scroll_up();
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.results.len().saturating_sub(1);
                self.state.selected = self.state.selected.min(max).saturating_add(1).min(max);
                self.adjust_scroll_down();
                self.dirty = true;
                true
            }
            IpcKey::Char(' ') => {
                if !self.state.results.is_empty() {
                    self.play_selected();
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.state.query.push(c);
                self.dirty = true;
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn adjust_scroll_up(&mut self) {
        if self.state.selected < self.state.scroll {
            self.state.scroll = self.state.selected;
        }
    }

    fn adjust_scroll_down(&mut self) {
        let max_visible = max_visible_tracks(self.area.h);
        if self.state.selected >= self.state.scroll + max_visible {
            self.state.scroll = self
                .state
                .selected
                .saturating_sub(max_visible.saturating_sub(1));
        }
    }

    fn play_selected(&mut self) {
        if let Some(track) = self.state.results.get(self.state.selected) {
            log::info!(
                "play preview: {} — {} ({})",
                track.track_name,
                track.artist_name,
                track.preview_url
            );
            self.pending_plugin_message = Some(PluginMessage {
                to: "host".into(),
                action: "open_url".into(),
                data: serde_json::json!({"url": track.preview_url}),
            });
        }
    }

    fn trigger_search(&mut self, query: &str) {
        let q = query.to_string();
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.state.fetch_state = FetchState::Fetching;
        self.dirty = true;
        std::thread::spawn(move || match api::search(&q) {
            Ok(results) => {
                let _ = tx.send(FetchMsg::SearchDone(results));
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::SearchError(e));
            }
        });
    }

    fn handle_tick(&mut self) {
        if let Some(ref rx) = self.rx_fetch {
            match rx.try_recv() {
                Ok(FetchMsg::SearchDone(results)) => {
                    self.state.results = results;
                    self.state.fetch_state = FetchState::Done;
                    self.state.selected = 0;
                    self.state.scroll = 0;
                    self.dirty = true;
                }
                Ok(FetchMsg::SearchError(e)) => {
                    self.state.fetch_state = FetchState::Error(e);
                    self.dirty = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(&self.state, &self.theme, self.area.w, self.area.h);
            self.state.dirty = false;
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("Music".into(), "Search iTunes previews".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let commands_val = match serde_json::to_value(app.render()) {
        Ok(v) => v,
        Err(e) => {
            log::error!("failed to serialize render commands: {e}");
            return;
        }
    };
    let request = app.pending_request.take();
    let plugin_message = app.pending_plugin_message.take();
    let palette = palette_commands();
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": [],
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
                        log::error!("[music-preview] parse error: {e}: {line}");
                        continue;
                    }
                };

                match msg {
                    HostMsg::Init {
                        theme,
                        area,
                        data_dir: _,
                    } => {
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
                        if index == 0 {
                            app.dirty = true;
                        }
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
                    HostMsg::DbValue { .. } => {
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
    use crate::api::ItunesTrack;

    fn make_track(id: u32, name: &str, url: &str) -> ItunesTrack {
        ItunesTrack {
            track_id: id,
            track_name: name.into(),
            artist_name: "Artist".into(),
            collection_name: "Album".into(),
            artwork_url_100: String::new(),
            preview_url: url.into(),
            track_time_millis: Some(180000),
            primary_genre_name: "Rock".into(),
        }
    }

    #[test]
    fn handle_key_char_appends_to_query() {
        let mut app = App::default();
        assert!(app.handle_key(IpcKey::Char('a')));
        assert_eq!(app.state.query, "a");
        assert!(app.handle_key(IpcKey::Char('b')));
        assert_eq!(app.state.query, "ab");
    }

    #[test]
    fn handle_key_backspace_removes_from_query() {
        let mut app = App::default();
        app.state.query = "ab".into();
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.query, "a");
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.query, "");
        // Backspace on empty is harmless
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.query, "");
    }

    #[test]
    fn handle_key_enter_triggers_search() {
        let mut app = App::default();
        app.state.query = "eminem".into();
        assert!(app.handle_key(IpcKey::Enter));
        assert!(matches!(app.state.fetch_state, FetchState::Fetching));
        assert!(app.rx_fetch.is_some());
    }

    #[test]
    fn handle_key_enter_empty_query_does_not_trigger() {
        let mut app = App::default();
        app.state.query = "   ".into();
        assert!(app.handle_key(IpcKey::Enter));
        assert!(matches!(app.state.fetch_state, FetchState::Idle));
    }

    #[test]
    fn handle_key_up_down_navigates() {
        let mut app = App::default();
        app.state.results = vec![
            make_track(1, "A", "http://a"),
            make_track(2, "B", "http://b"),
            make_track(3, "C", "http://c"),
        ];
        // Down
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 1);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 2);
        // At end, stays
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 2);
        // Up
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 1);
    }

    #[test]
    fn handle_key_up_at_top_stays() {
        let mut app = App::default();
        app.state.results = vec![make_track(1, "A", "http://a")];
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 0);
    }

    #[test]
    fn handle_key_esc_not_consumed() {
        let mut app = App::default();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_space_is_consumed() {
        let mut app = App::default();
        app.state.results = vec![make_track(1, "A", "http://preview/1")];
        assert!(app.handle_key(IpcKey::Char(' ')));
    }

    #[test]
    fn handle_key_space_sets_plugin_message() {
        let mut app = App::default();
        app.state.results = vec![make_track(1, "A", "http://preview/1")];
        app.handle_key(IpcKey::Char(' '));
        assert!(app.pending_plugin_message.is_some());
        let msg = app.pending_plugin_message.as_ref().unwrap();
        assert_eq!(msg.to, "host");
        assert_eq!(msg.action, "open_url");
        assert_eq!(msg.data["url"], "http://preview/1");
    }

    #[test]
    fn handle_key_space_no_results_no_op() {
        let mut app = App::default();
        app.handle_key(IpcKey::Char(' '));
        assert!(app.pending_plugin_message.is_none());
    }

    #[test]
    fn handle_key_char_k_navigates_up() {
        let mut app = App::default();
        app.state.selected = 1;
        app.handle_key(IpcKey::Char('k'));
        assert_eq!(app.state.selected, 0);
    }

    #[test]
    fn handle_key_char_j_navigates_down() {
        let mut app = App::default();
        app.state.results = vec![
            make_track(1, "A", "http://a"),
            make_track(2, "B", "http://b"),
        ];
        app.handle_key(IpcKey::Char('j'));
        assert_eq!(app.state.selected, 1);
    }

    #[test]
    fn handle_tick_drains_search_done() {
        let mut app = App::default();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        app.state.fetch_state = FetchState::Fetching;
        let _ = tx.send(FetchMsg::SearchDone(vec![make_track(
            1,
            "Track",
            "http://url",
        )]));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Done));
        assert_eq!(app.state.results.len(), 1);
        assert_eq!(app.state.selected, 0);
        assert_eq!(app.state.scroll, 0);
    }

    #[test]
    fn handle_tick_drains_search_error() {
        let mut app = App::default();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        app.state.fetch_state = FetchState::Fetching;
        let _ = tx.send(FetchMsg::SearchError("network error".into()));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Error(_)));
        match &app.state.fetch_state {
            FetchState::Error(msg) => assert_eq!(msg, "network error"),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn handle_tick_empty_queue_no_change() {
        let mut app = App::default();
        app.state.fetch_state = FetchState::Fetching;
        let (_tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Fetching));
    }

    #[test]
    fn palette_commands_returns_music_entry() {
        let cmds = palette_commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].0, "Music");
        assert_eq!(cmds[0].1, "Search iTunes previews");
    }

    #[test]
    fn app_default_has_default_state() {
        let app = App::default();
        assert!(app.state.query.is_empty());
        assert!(app.dirty);
    }

    #[test]
    fn scroll_adjusts_up_when_selected_above_view() {
        let mut app = App::default();
        app.state.results = (0..20)
            .map(|i| make_track(i as u32, &format!("Track {}", i), "http://x"))
            .collect();
        app.state.selected = 10;
        app.state.scroll = 10;
        app.handle_key(IpcKey::Up);
        assert_eq!(app.state.selected, 9);
        assert_eq!(app.state.scroll, 9);
    }

    #[test]
    fn scroll_adjusts_down_when_selected_below_view() {
        let mut app = App::default();
        app.state.results = (0..20)
            .map(|i| make_track(i as u32, &format!("Track {}", i), "http://x"))
            .collect();
        app.state.selected = 5;
        app.state.scroll = 0;
        let max_vis = max_visible_tracks(app.area.h);
        // Move selected to just past the visible range
        for _ in 0..max_vis {
            app.handle_key(IpcKey::Down);
        }
        assert!(app.state.scroll > 0);
    }
}
