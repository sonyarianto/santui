mod lrclib;
mod state;
mod ui;

use std::io::{BufRead, BufReader};
use std::sync::mpsc;

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginMessage, PluginRequest, RenderCmd, ThemeData,
};

use state::{FetchState, LyricsState};
use ui::{max_visible_tracks, render_ui};

enum FetchMsg {
    SearchDone(String, Vec<lrclib::LRCLibTrack>),
    SearchError(String),
}

struct App {
    state: LyricsState,
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
            state: LyricsState::default(),
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
        if self.state.show_lyrics {
            match key {
                IpcKey::Esc => {
                    self.state.show_lyrics = false;
                    self.state.lyrics_text.clear();
                    self.state.lyrics_source.clear();
                    self.state.lyrics_scroll = 0;
                    self.dirty = true;
                    true
                }
                IpcKey::Up => {
                    self.state.lyrics_scroll = self.state.lyrics_scroll.saturating_sub(1);
                    self.dirty = true;
                    true
                }
                IpcKey::Down => {
                    let line_count = self.state.lyrics_text.lines().count().saturating_sub(1);
                    self.state.lyrics_scroll =
                        self.state.lyrics_scroll.saturating_add(1).min(line_count);
                    self.dirty = true;
                    true
                }
                IpcKey::PageUp => {
                    let page = (self.area.h.saturating_sub(5) as usize).max(1);
                    self.state.lyrics_scroll = self.state.lyrics_scroll.saturating_sub(page);
                    self.dirty = true;
                    true
                }
                IpcKey::PageDown => {
                    let page = (self.area.h.saturating_sub(5) as usize).max(1);
                    let line_count = self.state.lyrics_text.lines().count().saturating_sub(1);
                    self.state.lyrics_scroll = self
                        .state
                        .lyrics_scroll
                        .saturating_add(page)
                        .min(line_count);
                    self.dirty = true;
                    true
                }
                IpcKey::Char('c') => {
                    self.state.results.clear();
                    self.state.fetch_state = FetchState::Idle;
                    self.state.query.clear();
                    self.state.selected = 0;
                    self.state.scroll = 0;
                    self.state.show_lyrics = false;
                    self.state.lyrics_text.clear();
                    self.state.lyrics_source.clear();
                    self.state.lyrics_scroll = 0;
                    self.dirty = true;
                    true
                }
                _ => true,
            }
        } else if self.state.search_mode {
            match key {
                IpcKey::Esc => {
                    self.state.search_mode = false;
                    self.state.query.clear();
                    self.dirty = true;
                    true
                }
                IpcKey::Enter => {
                    let q = self.state.query.trim().to_string();
                    if !q.is_empty() {
                        self.state.search_mode = false;
                        self.trigger_search(q);
                    }
                    self.dirty = true;
                    true
                }
                IpcKey::Backspace => {
                    self.state.query.pop();
                    self.dirty = true;
                    true
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.state.query.push(c);
                    self.dirty = true;
                    true
                }
                _ => true,
            }
        } else {
            match key {
                IpcKey::Char('/') => {
                    self.state.show_lyrics = false;
                    self.state.search_mode = true;
                    self.state.query.clear();
                    self.dirty = true;
                    true
                }
                IpcKey::Enter => {
                    if matches!(self.state.fetch_state, FetchState::Done)
                        && !self.state.results.is_empty()
                    {
                        self.show_lyrics_for_selected();
                    }
                    self.dirty = true;
                    true
                }
                IpcKey::Up => {
                    self.state.selected = self.state.selected.saturating_sub(1);
                    self.adjust_scroll_up();
                    self.dirty = true;
                    true
                }
                IpcKey::Down => {
                    let max = self.state.results.len().saturating_sub(1);
                    self.state.selected = self.state.selected.min(max).saturating_add(1).min(max);
                    self.adjust_scroll_down();
                    self.dirty = true;
                    true
                }
                IpcKey::PageUp => {
                    let page_size = max_visible_tracks(self.area.h).max(1);
                    self.state.selected = self.state.selected.saturating_sub(page_size);
                    self.adjust_scroll_up();
                    self.dirty = true;
                    true
                }
                IpcKey::PageDown => {
                    let page_size = max_visible_tracks(self.area.h).max(1);
                    let max = self.state.results.len().saturating_sub(1);
                    self.state.selected = self.state.selected.saturating_add(page_size).min(max);
                    self.adjust_scroll_down();
                    self.dirty = true;
                    true
                }
                IpcKey::Char('c') => {
                    if !self.state.results.is_empty() {
                        self.state.results.clear();
                        self.state.fetch_state = FetchState::Idle;
                        self.state.query.clear();
                        self.state.selected = 0;
                        self.state.scroll = 0;
                        self.state.show_lyrics = false;
                        self.state.lyrics_text.clear();
                        self.state.lyrics_source.clear();
                        self.state.lyrics_scroll = 0;
                        self.dirty = true;
                    }
                    true
                }
                IpcKey::Esc => false,
                _ => false,
            }
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

    fn show_lyrics_for_selected(&mut self) {
        let track = self.state.results.get(self.state.selected);
        if let Some(track) = track {
            if let Some(lyrics) = lrclib::extract_lyrics(track) {
                self.state.lyrics_text = lyrics.text;
                self.state.lyrics_source = lyrics.source;
            } else {
                self.state.lyrics_text = String::new();
                self.state.lyrics_source = String::new();
            }
            self.state.lyrics_title = track.track_name.clone();
            self.state.lyrics_artist = track.artist_name.clone();
            self.state.lyrics_scroll = 0;
            self.state.show_lyrics = true;
            self.dirty = true;
        }
    }

    fn trigger_search(&mut self, q: String) {
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.state.fetch_state = FetchState::Fetching;
        self.dirty = true;
        std::thread::spawn(move || match lrclib::search(&q) {
            Ok(results) => {
                let _ = tx.send(FetchMsg::SearchDone(q, results));
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::SearchError(e));
            }
        });
    }

    fn handle_tick(&mut self) {
        self.state.tick_counter += 1;
        if self.state.tick_counter.is_multiple_of(3) {
            self.dirty = true;
        }

        if let Some(ref rx) = self.rx_fetch {
            match rx.try_recv() {
                Ok(FetchMsg::SearchDone(q, results)) => {
                    if q == self.state.query {
                        self.state.results = results;
                        self.state.fetch_state = FetchState::Done;
                        self.state.selected = 0;
                        self.state.scroll = 0;
                        self.dirty = true;
                    }
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

fn hints(state: &LyricsState) -> Vec<(String, String)> {
    if state.show_lyrics {
        vec![
            ("\u{2191}\u{2193}".into(), "scroll".into()),
            ("pg up/dn".into(), "page".into()),
            ("esc".into(), "back".into()),
        ]
    } else {
        vec![
            ("/".into(), "search".into()),
            ("c".into(), "clear".into()),
            ("\u{21B5}".into(), "lyrics".into()),
            ("esc".into(), "back".into()),
        ]
    }
}

fn palette_commands() -> Vec<(String, String)> {
    vec![]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: hints(&app.state),
        palette_commands: palette_commands(),
        request: app.pending_request.take(),
        plugin_message: app.pending_plugin_message.take(),
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
                        log::error!("[music-lyrics] parse error: {e}: {line}");
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
    use crate::lrclib::LRCLibTrack;

    fn make_track(id: u64, name: &str) -> LRCLibTrack {
        LRCLibTrack {
            id,
            track_name: name.into(),
            artist_name: "Artist".into(),
            album_name: "Album".into(),
            duration: 200.0,
            instrumental: false,
            plain_lyrics: Some("line one\nline two".into()),
            synced_lyrics: None,
        }
    }

    fn make_track_no_lyrics(id: u64, name: &str) -> LRCLibTrack {
        LRCLibTrack {
            id,
            track_name: name.into(),
            artist_name: "Artist".into(),
            album_name: "Album".into(),
            duration: 200.0,
            instrumental: false,
            plain_lyrics: None,
            synced_lyrics: None,
        }
    }

    #[test]
    fn handle_key_char_outside_search_ignored() {
        let mut app = App::default();
        assert!(!app.handle_key(IpcKey::Char('a')));
        assert_eq!(app.state.query, "");
    }

    #[test]
    fn handle_key_backspace_removes_from_query() {
        let mut app = App::default();
        app.state.search_mode = true;
        app.state.query = "ab".into();
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.query, "a");
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.query, "");
    }

    #[test]
    fn handle_key_slash_enters_search_mode() {
        let mut app = App::default();
        assert!(app.handle_key(IpcKey::Char('/')));
        assert!(app.state.search_mode);
    }

    #[test]
    fn handle_key_search_enter_triggers_search() {
        let mut app = App::default();
        app.state.search_mode = true;
        app.state.query = "eminem".into();
        assert!(app.handle_key(IpcKey::Enter));
        assert!(!app.state.search_mode);
        assert!(matches!(app.state.fetch_state, FetchState::Fetching));
        assert!(app.rx_fetch.is_some());
    }

    #[test]
    fn handle_key_search_enter_empty_does_not_trigger() {
        let mut app = App::default();
        app.state.search_mode = true;
        app.state.query = "   ".into();
        assert!(app.handle_key(IpcKey::Enter));
        assert!(app.state.search_mode);
        assert!(matches!(app.state.fetch_state, FetchState::Idle));
    }

    #[test]
    fn handle_key_search_esc_exits_search_mode() {
        let mut app = App::default();
        app.state.search_mode = true;
        app.state.query = "test".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(!app.state.search_mode);
        assert!(app.state.query.is_empty());
    }

    #[test]
    fn handle_key_up_down_navigates() {
        let mut app = App::default();
        app.state.results = vec![make_track(1, "A"), make_track(2, "B"), make_track(3, "C")];
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 1);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 2);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 2);
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 1);
    }

    #[test]
    fn handle_key_up_at_top_stays() {
        let mut app = App::default();
        app.state.results = vec![make_track(1, "A")];
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 0);
    }

    #[test]
    fn handle_key_esc_not_consumed() {
        let mut app = App::default();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_enter_shows_lyrics() {
        let mut app = App::default();
        app.state.results = vec![make_track(1, "A")];
        app.state.fetch_state = FetchState::Done;
        app.handle_key(IpcKey::Enter);
        assert!(app.state.show_lyrics);
        assert_eq!(app.state.lyrics_text, "line one\nline two");
        assert_eq!(app.state.lyrics_title, "A");
    }

    #[test]
    fn handle_key_enter_shows_no_lyrics_message() {
        let mut app = App::default();
        app.state.results = vec![make_track_no_lyrics(1, "A")];
        app.state.fetch_state = FetchState::Done;
        app.handle_key(IpcKey::Enter);
        assert!(app.state.show_lyrics);
        assert!(app.state.lyrics_text.is_empty());
    }

    #[test]
    fn handle_key_esc_from_lyrics_returns_to_list() {
        let mut app = App::default();
        app.state.show_lyrics = true;
        app.state.lyrics_text = "some lyrics".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(!app.state.show_lyrics);
        assert!(app.state.lyrics_text.is_empty());
    }

    #[test]
    fn handle_key_up_down_in_lyrics_scrolls() {
        let mut app = App::default();
        app.state.show_lyrics = true;
        app.state.lyrics_text = "line1\nline2\nline3".into();
        app.state.lyrics_scroll = 1;
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.lyrics_scroll, 0);
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.lyrics_scroll, 0);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.lyrics_scroll, 1);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.lyrics_scroll, 2);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.lyrics_scroll, 2);
    }

    #[test]
    fn handle_key_c_clears_results() {
        let mut app = App::default();
        app.state.results = vec![make_track(1, "A")];
        app.state.fetch_state = FetchState::Done;
        app.state.show_lyrics = true;
        app.state.lyrics_text = "lyrics".into();
        assert!(app.handle_key(IpcKey::Char('c')));
        assert!(app.state.results.is_empty());
        assert_eq!(app.state.fetch_state, FetchState::Idle);
        assert!(!app.state.show_lyrics);
        assert!(app.state.lyrics_text.is_empty());
    }

    #[test]
    fn handle_tick_drains_search_done() {
        let mut app = App::default();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        app.state.fetch_state = FetchState::Fetching;
        app.state.query = "test".into();
        let _ = tx.send(FetchMsg::SearchDone(
            "test".into(),
            vec![make_track(1, "Track")],
        ));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Done));
        assert_eq!(app.state.results.len(), 1);
        assert_eq!(app.state.selected, 0);
        assert_eq!(app.state.scroll, 0);
    }

    #[test]
    fn handle_tick_discards_stale_results() {
        let mut app = App::default();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        app.state.fetch_state = FetchState::Fetching;
        app.state.query = "newquery".into();
        let _ = tx.send(FetchMsg::SearchDone(
            "oldquery".into(),
            vec![make_track(1, "Stale")],
        ));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Fetching));
        assert!(app.state.results.is_empty());
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
    }

    #[test]
    fn scroll_adjusts_up_when_selected_above_view() {
        let mut app = App::default();
        app.state.results = (0..20)
            .map(|i| make_track(i, &format!("Track {i}")))
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
        app.area.h = 8;
        app.state.results = (0..20)
            .map(|i| make_track(i, &format!("Track {i}")))
            .collect();
        app.state.selected = 0;
        app.state.scroll = 0;
        for _ in 0..5 {
            app.handle_key(IpcKey::Down);
        }
        assert!(app.state.scroll > 0);
    }

    #[test]
    fn hints_in_list_mode() {
        let state = LyricsState::default();
        let h = hints(&state);
        assert!(h.iter().any(|(k, _)| k == "/"));
        assert!(h.iter().any(|(k, _)| k == "c"));
        assert!(h.iter().any(|(k, _)| k == "\u{21B5}"));
    }

    #[test]
    fn hints_in_lyrics_mode() {
        let state = LyricsState {
            show_lyrics: true,
            ..LyricsState::default()
        };
        let h = hints(&state);
        assert!(h.iter().any(|(k, _)| k == "\u{2191}\u{2193}"));
        assert!(h.iter().any(|(k, _)| k == "esc"));
    }

    #[test]
    fn palette_commands_is_empty() {
        let cmds = palette_commands();
        assert!(cmds.is_empty());
    }

    #[test]
    fn app_default_has_default_state() {
        let app = App::default();
        assert!(app.state.query.is_empty());
        assert!(app.dirty);
    }
}
