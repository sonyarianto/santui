mod database;
mod http;
mod itunes;
mod lrclib;
mod player;
mod state;
mod stations;
mod ui;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use ui::{HEADER_H, TABLE_TOP};

const LIST_OVERHEAD: u16 = TABLE_TOP + HEADER_H + 1 + 2; // top + search + sep + header + bottom + footer (blank + hints)

use player::Mpv;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcMouseEvent, MouseButton, MouseEventKind, PluginRequest, RenderCmd,
    ThemeData, UserData,
};

enum MpvMsg {
    FileLoaded(String),
    Metadata(String),
    TrackInfo(u64, itunes::TrackInfo),
    Lyrics(u64, Option<lrclib::LyricsData>),
    EndFile(u32),
    MpvReset,
}

enum MpvCmd {
    LoadUrl(String),
    Stop,
    SetVolume(i64),
    Quit,
}

struct App {
    state: state::RadioState,
    theme: ThemeData,
    area: Area,
    tx_cmd: Option<mpsc::Sender<MpvCmd>>,
    rx_msg: Option<mpsc::Receiver<MpvMsg>>,
    tx_msg: Option<mpsc::Sender<MpvMsg>>,
    mpv_thread: Option<thread::JoinHandle<()>>,
    mpv_wakeup: Option<player::MpvWakeup>,
    mpv_heartbeat: Arc<AtomicU64>,
    mpv_last_seen: u64,
    mpv_stuck_since: Option<std::time::Instant>,
    init_error: Option<String>,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    user: Option<UserData>,
    db: Option<rusqlite::Connection>,
    pending_request: Option<PluginRequest>,
}

fn send_cmd(app: &App, cmd: MpvCmd) {
    if let Some(ref tx) = app.tx_cmd {
        let _ = tx.send(cmd);
    }
}

impl App {
    fn new() -> Self {
        let (db, station_list, init_error) = match database::open() {
            Ok(db) => {
                let list = stations::load(&db);
                (Some(db), list, None)
            }
            Err(e) => {
                let err = format!("Database error: {e}");
                log::error!("{err}");
                match rusqlite::Connection::open_in_memory() {
                    Ok(fallback) => (Some(fallback), Vec::new(), Some(err)),
                    Err(e2) => {
                        let err2 = format!("{err}; in-memory fallback also failed: {e2}");
                        log::error!("{err2}");
                        (None, Vec::new(), Some(err2))
                    }
                }
            }
        };
        App {
            db,
            state: state::RadioState::new(station_list),
            theme: ThemeData {
                text: [220, 220, 220],
                text_muted: [140, 140, 140],
                accent: [157, 124, 216],
                highlight: [250, 178, 131],
                logo: [255, 185, 0],
                background: [20, 20, 20],
                background_panel: [20, 20, 20],
                background_overlay: [10, 10, 10],
                border: [250, 178, 131],
                success: [127, 216, 143],
                error: [224, 108, 117],
                inverted_text: [20, 20, 20],
            },
            area: Area { w: 80, h: 24 },
            tx_cmd: None,
            rx_msg: None,
            tx_msg: None,
            mpv_thread: None,
            mpv_wakeup: None,
            mpv_heartbeat: Arc::new(AtomicU64::new(0)),
            mpv_last_seen: 0,
            mpv_stuck_since: None,
            init_error,
            dirty: true,
            cached_commands: Vec::new(),
            user: None,
            pending_request: Some(PluginRequest::DbGet {
                key: "favorites".into(),
            }),
        }
    }

    fn apply_metadata(
        state: &mut state::RadioState,
        tx_msg: &Option<mpsc::Sender<MpvMsg>>,
        title: String,
    ) {
        if title == state.last_metadata {
            return;
        }
        state.last_metadata = title.clone();
        state.song_title = title.clone();
        state.track_info = None;
        state.clear_lyrics();
        state.lyrics_loading = true;
        state.metadata_seq += 1;
        let seq = state.metadata_seq;
        let Some(ref tx) = tx_msg else {
            return;
        };
        let tx = tx.clone();
        let tx2 = tx.clone();
        let title_for_itunes = title.clone();
        thread::spawn(move || {
            if let Ok(Some(info)) = itunes::lookup(&title_for_itunes) {
                let _ = tx.send(MpvMsg::TrackInfo(seq, info));
            }
        });
        thread::spawn(move || {
            let (artist, track) = lrclib::split_title(&title);
            let lyrics = lrclib::fetch(&track, artist.as_deref());
            match lyrics {
                Ok(data) => {
                    let _ = tx2.send(MpvMsg::Lyrics(seq, data));
                }
                Err(_) => {
                    let _ = tx2.send(MpvMsg::Lyrics(seq, None));
                }
            }
        });
    }

    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.dirty = true;

        let (mut mpv, warns) = match Mpv::new() {
            Ok(v) => v,
            Err(e) => {
                self.init_error = Some(format!("{e}"));
                return;
            }
        };

        for w in &warns {
            log::warn!("  ⚠️  {w}");
        }

        if let Err(e) = mpv.observe_property(0, "metadata") {
            log::warn!("mpv observe_property(metadata) failed: {e}");
        }
        if let Err(e) = mpv.observe_property(1, "media-title") {
            log::warn!("mpv observe_property(media-title) failed: {e}");
        }
        if let Err(e) = mpv.observe_property(2, "volume") {
            log::warn!("mpv observe_property(volume) failed: {e}");
        }
        if let Err(e) = mpv.set_volume(self.state.volume) {
            log::warn!("mpv set_volume failed: {e}");
        }

        self.mpv_wakeup = Some(mpv.make_wakeup());

        let (tx_msg, rx_msg) = mpsc::channel::<MpvMsg>();
        let (tx_cmd, rx_cmd) = mpsc::channel::<MpvCmd>();

        let tx_msg_mpv = tx_msg.clone();
        let hb = self.mpv_heartbeat.clone();

        let handle = thread::spawn(move || {
            loop {
                hb.fetch_add(1, Ordering::Relaxed);
                let ev = mpv.wait_event_raw(0.1);
                if let Some(ev) = ev {
                    let id = ev.event_id;
                    let name = match id {
                        player::MPV_EVENT_SHUTDOWN => "SHUTDOWN",
                        player::MPV_EVENT_FILE_LOADED => "FILE_LOADED",
                        player::MPV_EVENT_PLAYBACK_RESTART => "PLAYBACK_RESTART",
                        player::MPV_EVENT_PROPERTY_CHANGE => "PROPERTY_CHANGE",
                        player::MPV_EVENT_END_FILE => "END_FILE",
                        7 => "START_FILE",
                        _ => "OTHER",
                    };
                    log::info!("mpv_thread: event {name} (id={id})");
                    if id == player::MPV_EVENT_SHUTDOWN {
                        break;
                    }
                    if id == player::MPV_EVENT_FILE_LOADED {
                        let title = mpv
                            .metadata_title()
                            .ok()
                            .flatten()
                            .or_else(|| mpv.media_title().ok().flatten())
                            .unwrap_or_default();
                        let _ = tx_msg_mpv.send(MpvMsg::FileLoaded(title));
                    }
                    if id == player::MPV_EVENT_PLAYBACK_RESTART {
                        let title = mpv
                            .metadata_title()
                            .ok()
                            .flatten()
                            .or_else(|| mpv.media_title().ok().flatten())
                            .unwrap_or_default();
                        let _ = tx_msg_mpv.send(MpvMsg::FileLoaded(title));
                    }
                    if id == player::MPV_EVENT_PROPERTY_CHANGE {
                        if ev.data.is_null() {
                            continue;
                        }
                        let prop: &player::MpvEventProperty = unsafe { &*(ev.data as *const _) };
                        if prop.name.is_null() {
                            continue;
                        }
                        let name = unsafe {
                            std::ffi::CStr::from_ptr(prop.name)
                                .to_string_lossy()
                                .to_string()
                        };
                        if name == "metadata" {
                            let t = mpv.metadata_title().ok().flatten().unwrap_or_default();
                            let _ = tx_msg_mpv.send(MpvMsg::Metadata(t));
                        } else if name == "media-title" {
                            let t = mpv.media_title().ok().flatten().unwrap_or_default();
                            let _ = tx_msg_mpv.send(MpvMsg::Metadata(t));
                        }
                    }
                    if id == player::MPV_EVENT_END_FILE {
                        if ev.data.is_null() {
                            continue;
                        }
                        let ef: &player::MpvEventEndFile = unsafe { &*(ev.data as *const _) };
                        let _ = tx_msg_mpv.send(MpvMsg::EndFile(ef.reason));
                    }
                }

                while let Ok(cmd) = rx_cmd.try_recv() {
                    match cmd {
                        MpvCmd::LoadUrl(mut url) => {
                            loop {
                                match rx_cmd.try_recv() {
                                    Ok(MpvCmd::LoadUrl(new_url)) => {
                                        let prev = url.chars().take(60).collect::<String>();
                                        url = new_url;
                                        log::info!(
                                            "mpv_thread: collapsed LoadUrl, prev={:?}",
                                            prev,
                                        );
                                    }
                                    Ok(MpvCmd::Stop) => {
                                        if let Err(e) = mpv.stop() {
                                            log::warn!("mpv stop failed: {e}");
                                        }
                                    }
                                    Ok(MpvCmd::SetVolume(v)) => {
                                        if let Err(e) = mpv.set_volume(v) {
                                            log::warn!("mpv set_volume failed: {e}");
                                        }
                                    }
                                    Ok(MpvCmd::Quit) => {
                                        mpv.destroy();
                                        return;
                                    }
                                    Err(_) => break,
                                }
                            }
                            // Stop previous playback and drain stale events so
                            // mpv has a clean internal state before loading the
                            // new URL.  This avoids cases where a stuck/broken
                            // network stream prevents loadfile from working.
                            let _ = mpv.stop();
                            let mut drained = 0u32;
                            while let Some(stale) = mpv.wait_event_raw(0.0) {
                                if stale.event_id == player::MPV_EVENT_SHUTDOWN {
                                    break;
                                }
                                drained += 1;
                            }
                            if drained > 0 {
                                log::info!(
                                    "mpv_thread: pre-drain discarded {drained} stale events"
                                );
                            }
                            log::info!("mpv_thread: calling load_url");
                            match mpv.load_url(&url) {
                                Ok(()) => {}
                                Err(e) => {
                                    log::warn!("mpv load_url failed: {e}");
                                    // Try stop + retry once to recover a transient state
                                    let _ = mpv.stop();
                                    while let Some(stale) = mpv.wait_event_raw(0.0) {
                                        if stale.event_id == player::MPV_EVENT_SHUTDOWN {
                                            break;
                                        }
                                    }
                                    match mpv.load_url(&url) {
                                        Ok(()) => {}
                                        Err(e2) => {
                                            log::warn!("mpv load_url retry also failed: {e2}");
                                            let _ = tx_msg_mpv.send(MpvMsg::MpvReset);
                                        }
                                    }
                                }
                            }
                        }
                        MpvCmd::Stop => {
                            if let Err(e) = mpv.stop() {
                                log::warn!("mpv stop failed: {e}");
                            }
                        }
                        MpvCmd::SetVolume(v) => {
                            if let Err(e) = mpv.set_volume(v) {
                                log::warn!("mpv set_volume failed: {e}");
                            }
                        }
                        MpvCmd::Quit => {
                            mpv.destroy();
                            return;
                        }
                    }
                }
            }
            mpv.destroy();
        });

        self.mpv_thread = Some(handle);

        self.tx_cmd = Some(tx_cmd);
        self.rx_msg = Some(rx_msg);
        self.tx_msg = Some(tx_msg);
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.state.scan_msg = None;
        self.dirty = true;

        if self.state.search_mode {
            match key {
                IpcKey::Esc => {
                    self.state.search_mode = false;
                    self.state.set_query(String::new());
                    return true;
                }
                IpcKey::Enter => {
                    self.state.search_mode = false;
                    self.play_selected_station();
                    return true;
                }
                IpcKey::Backspace => {
                    self.state.query.pop();
                    self.state.apply_filter();
                    return true;
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.state.query.push(c);
                    self.state.apply_filter();
                    return true;
                }
                IpcKey::Up => {
                    self.state.select_prev();
                    let info_h = self.state.info_h();
                    let max_visible = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                    self.state.ensure_scroll_visible(max_visible.max(1));
                    return true;
                }
                IpcKey::Down => {
                    self.state.select_next();
                    let info_h = self.state.info_h();
                    let max_visible = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                    self.state.ensure_scroll_visible(max_visible.max(1));
                    return true;
                }
                _ => return false,
            }
        }

        match key {
            IpcKey::Tab => {
                if self.state.show_lyrics {
                    self.state.lyrics_focused = !self.state.lyrics_focused;
                }
                true
            }
            IpcKey::Up => {
                if self.state.show_lyrics && self.state.lyrics_focused {
                    self.state.lyrics_scroll_up();
                } else {
                    self.state.select_prev();
                    let info_h = self.state.info_h();
                    let max_visible = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                    self.state.ensure_scroll_visible(max_visible.max(1));
                }
                true
            }
            IpcKey::Down => {
                if self.state.show_lyrics && self.state.lyrics_focused {
                    let panel_h = self.state.lyrics_content_height(self.area.h);
                    let inner_w = self.state.lyrics_inner_w(self.area.w);
                    self.state.lyrics_scroll_down(panel_h, inner_w);
                } else {
                    self.state.select_next();
                    let info_h = self.state.info_h();
                    let max_visible = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                    self.state.ensure_scroll_visible(max_visible.max(1));
                }
                true
            }
            IpcKey::PageUp => {
                if self.state.show_lyrics && self.state.lyrics_focused {
                    let panel_h = self.state.lyrics_content_height(self.area.h);
                    self.state.lyrics_page_up(panel_h.max(1));
                } else {
                    let info_h = self.state.info_h();
                    let page = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                    self.state.select_page_up(page.max(1));
                    self.state.ensure_scroll_visible(page.max(1));
                }
                true
            }
            IpcKey::PageDown => {
                if self.state.show_lyrics && self.state.lyrics_focused {
                    let panel_h = self.state.lyrics_content_height(self.area.h);
                    let inner_w = self.state.lyrics_inner_w(self.area.w);
                    self.state.lyrics_page_down(panel_h.max(1), inner_w);
                } else {
                    let info_h = self.state.info_h();
                    let page = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                    self.state.select_page_down(page.max(1));
                    self.state.ensure_scroll_visible(page.max(1));
                }
                true
            }
            IpcKey::Char('/') => {
                if !self.state.lyrics_focused {
                    self.state.search_mode = true;
                    self.state.query.clear();
                    self.state.apply_filter();
                    self.state.selected = 0;
                    self.state.scroll = 0;
                    true
                } else {
                    false
                }
            }
            IpcKey::Enter => {
                if self.state.lyrics_focused {
                    // no-op while lyrics focused
                } else {
                    self.play_selected_station();
                }
                true
            }
            IpcKey::Char('r') => {
                if !self.state.lyrics_focused {
                    if let Some(ref db) = self.db {
                        let new_stations = crate::stations::reload(db);
                        let count = new_stations.len();
                        self.state.stations = new_stations;
                        self.state.set_query(String::new());
                        self.state.selected = 0;
                        self.state.scroll = 0;
                        self.state
                            .set_scan_msg(format!("Reloaded {count} stations from database"));
                    }
                }
                true
            }
            IpcKey::Char('s') => {
                send_cmd(self, MpvCmd::Stop);
                self.state.play_state = state::PlayState::Stopped;
                self.state.current_station = None;
                self.state.last_metadata.clear();
                self.state.song_title.clear();
                self.state.track_info = None;
                self.state.clear_lyrics();
                self.state.clear_retry();
                true
            }
            IpcKey::Char('+') | IpcKey::Char('=') => {
                self.state.volume_up();
                send_cmd(self, MpvCmd::SetVolume(self.state.volume));
                self.pending_request = Some(PluginRequest::DbSet {
                    key: "volume".into(),
                    value: self.state.volume.to_string(),
                });
                true
            }
            IpcKey::Char('-') => {
                self.state.volume_down();
                send_cmd(self, MpvCmd::SetVolume(self.state.volume));
                self.pending_request = Some(PluginRequest::DbSet {
                    key: "volume".into(),
                    value: self.state.volume.to_string(),
                });
                true
            }
            IpcKey::Char('l') => {
                self.state.show_lyrics = !self.state.show_lyrics;
                self.state.lyrics_scroll = 0;
                self.state.lyrics_focused = false;
                true
            }
            IpcKey::Char(' ') => {
                if self.state.lyrics_focused {
                    return false;
                }
                let station_url = self.state.selected_station().map(|s| s.url.clone());
                if let Some(url) = station_url {
                    let is_fav = self.state.toggle_favorite(&url);
                    let favs_json =
                        serde_json::to_string(&self.state.favorites.iter().collect::<Vec<_>>())
                            .unwrap_or_default();
                    self.pending_request = Some(PluginRequest::DbSet {
                        key: "favorites".into(),
                        value: favs_json,
                    });
                    self.state.set_scan_msg(if is_fav {
                        "♥ Added to favorites".into()
                    } else {
                        "Removed from favorites".into()
                    });
                    self.state.apply_filter();
                }
                true
            }
            IpcKey::Char('f') => {
                if self.state.lyrics_focused {
                    return false;
                }
                self.state.show_favorites_only = !self.state.show_favorites_only;
                self.state.apply_filter();
                self.state.selected = 0;
                self.state.scroll = 0;
                if self.state.show_favorites_only {
                    self.state.set_scan_msg("♥ Showing favorites only".into());
                } else {
                    self.state.set_scan_msg("Showing all stations".into());
                }
                true
            }
            IpcKey::Char('c') if !self.state.lyrics_focused && !self.state.query.is_empty() => {
                self.state.set_query(String::new());
                self.state.selected = 0;
                self.state.scroll = 0;
                true
            }
            _ => false,
        }
    }

    fn play_selected_station(&mut self) {
        if let Some(station) = self.state.selected_station().cloned() {
            let idx = self.state.current_filtered_index();
            self.state.current_station = Some(idx);
            self.state.clear_retry();
            self.state.retry_mode = false;
            self.state.play_state = state::PlayState::Connecting(station.name.to_string());
            self.state.last_metadata.clear();
            self.state.song_title.clear();
            self.state.track_info = None;
            self.state.clear_lyrics();
            self.state.lyrics_loading = true;
            self.state.start_time = std::time::Instant::now();
            log::info!(
                "play_selected_station: {} → Connecting, url={:?}",
                station.name,
                station.url.chars().take(80).collect::<String>(),
            );
            // Fully recreate the mpv handle.  Destroy+replace ensures no stale
            // internal mpv state (stuck DNS, broken TCP connection, corrupted
            // audio buffer) leaks from one station to the next.
            self.reset_mpv();
        }
    }

    fn handle_mouse(&mut self, event: IpcMouseEvent) -> bool {
        let x = event.x;
        let y = event.y;
        let area_w = self.area.w;
        let area_h = self.area.h;

        let left_w = if self.state.show_lyrics {
            (area_w * 3 / 5).max(20)
        } else {
            area_w
        };
        let right_w = area_w.saturating_sub(left_w);
        let info_h = self.state.info_h();
        let stations_h = area_h.saturating_sub(info_h);

        match event.kind {
            MouseEventKind::Down => {
                if event.button != MouseButton::Left {
                    return false;
                }
                if self.state.search_mode {
                    return false;
                }

                // Stations table rows
                if x < left_w && y > ui::TABLE_TOP {
                    let table_avail =
                        stations_h.saturating_sub(ui::TABLE_TOP + ui::HEADER_H + 1 + 2);
                    let max_visible = table_avail as usize;
                    let visible_count = max_visible
                        .min(self.state.filtered.len().saturating_sub(self.state.scroll));
                    let row_start = ui::TABLE_TOP + 1;
                    let row_end = row_start + visible_count as u16;
                    if y >= row_start && y < row_end {
                        let row = (y - row_start) as usize;
                        let filtered_idx = self.state.scroll + row;
                        if filtered_idx < self.state.filtered.len() {
                            self.state.selected = filtered_idx;
                            self.state.scroll = self.state.scroll.min(self.state.selected);
                            self.play_selected_station();
                            return true;
                        }
                    }
                }

                // Lyrics panel — focus on click
                if self.state.show_lyrics && x >= left_w && x < left_w + right_w {
                    self.state.lyrics_focused = true;
                    return true;
                }

                false
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let is_up = event.kind == MouseEventKind::ScrollUp;
                if self.state.search_mode {
                    return false;
                }

                // Scroll lyrics if focused
                if self.state.show_lyrics && self.state.lyrics_focused {
                    if is_up {
                        self.state.lyrics_scroll_up();
                    } else {
                        let panel_h = self.state.lyrics_content_height(area_h);
                        let inner_w = self.state.lyrics_inner_w(self.area.w);
                        self.state.lyrics_scroll_down(panel_h, inner_w);
                    }
                    return true;
                }

                // Scroll station list otherwise
                if is_up {
                    self.state.select_prev();
                } else {
                    self.state.select_next();
                }
                let info_h = self.state.info_h();
                let max_visible = area_h.saturating_sub(info_h + 8) as usize;
                self.state.ensure_scroll_visible(max_visible.max(1));
                true
            }
            _ => false,
        }
    }

    fn handle_tick(&mut self) {
        let mut changed = false;
        let mut reset_requested = false;
        let tx_msg = self.tx_msg.clone();
        if let Some(ref rx) = self.rx_msg {
            while let Ok(msg) = rx.try_recv() {
                changed = true;
                match msg {
                    MpvMsg::FileLoaded(title) => {
                        let state_before = format!("{:?}", self.state.play_state);
                        match &self.state.play_state {
                            state::PlayState::Connecting(name)
                            | state::PlayState::Retrying(name) => {
                                self.state.play_state = state::PlayState::Playing(name.clone());
                                self.state.retry_deadline = None;
                                self.state.retry_attempt = 0;
                            }
                            _ => {}
                        }
                        log::info!(
                            "handle_tick: FileLoaded({:?}) state was {state_before}, now {:?}",
                            title,
                            self.state.play_state,
                        );
                        Self::apply_metadata(&mut self.state, &tx_msg, title);
                    }
                    MpvMsg::Metadata(title) => {
                        log::info!(
                            "handle_tick: Metadata({:?}) state={:?}",
                            title,
                            self.state.play_state,
                        );
                        if !matches!(self.state.play_state, state::PlayState::Playing(_)) {
                            continue;
                        }
                        Self::apply_metadata(&mut self.state, &tx_msg, title);
                    }
                    MpvMsg::TrackInfo(seq, info) => {
                        if seq != self.state.metadata_seq {
                            continue;
                        }
                        self.state.track_info = Some(info.clone());
                        if let Some(title) = &info.title {
                            self.state.song_title = title.clone();
                        }
                    }
                    MpvMsg::Lyrics(seq, data) => {
                        if seq != self.state.metadata_seq {
                            continue;
                        }
                        match data {
                            Some(lyrics) => {
                                self.state.lyrics_text = lyrics.text;
                                self.state.lyrics_source = lyrics.source;
                            }
                            None => {
                                self.state.lyrics_text = String::new();
                            }
                        }
                        self.state.lyrics_loading = false;
                        self.state.lyrics_scroll = 0;
                    }
                    MpvMsg::EndFile(reason) => {
                        let is_error = reason == player::MPV_END_FILE_REASON_EOF
                            || reason == player::MPV_END_FILE_REASON_ERROR;
                        log::info!(
                            "handle_tick: EndFile(reason={reason}) state={:?} is_error={is_error}",
                            self.state.play_state,
                        );
                        if !is_error {
                            continue;
                        }
                        let name = match &self.state.play_state {
                            state::PlayState::Playing(n) => Some(n.clone()),
                            state::PlayState::Connecting(n)
                                if reason == player::MPV_END_FILE_REASON_ERROR
                                    && self.state.retry_mode =>
                            {
                                // load_url failed while Connecting — start retry
                                // immediately instead of waiting for the 10s timeout.
                                // Only applies to automatic retries (retry_mode=true),
                                // NOT user-initiated selections (retry_mode=false).
                                // A stale EndFile from the previous stream arriving
                                // after the user switched stations must not trigger
                                // a spurious retry for the new station.
                                Some(n.clone())
                            }
                            _ => None,
                        };
                        if let Some(name) = name {
                            if self.state.retry_attempt >= state::MAX_RETRIES {
                                self.state.play_state = state::PlayState::Error(format!(
                                    "connection lost after {} attempts",
                                    state::MAX_RETRIES
                                ));
                                self.state.retry_deadline = None;
                            } else {
                                let delay_ms = state::retry_delay_ms(self.state.retry_attempt);
                                self.state.retry_deadline = Some(
                                    std::time::Instant::now()
                                        + std::time::Duration::from_millis(delay_ms),
                                );
                                self.state.play_state = state::PlayState::Retrying(name);
                                self.state.retry_attempt += 1;
                            }
                        }
                    }
                    MpvMsg::MpvReset => {
                        log::warn!("handle_tick: MpvReset — recreating mpv handle");
                        reset_requested = true;
                    }
                }
            }
        }
        if reset_requested {
            self.reset_mpv();
            // Fresh mpv handle — reset retry state so we get a full
            // MAX_RETRIES budget on the new handle.
            self.state.clear_retry();
            if let Some(idx) = self.state.current_station {
                let station = self.state.stations[idx].clone();
                self.state.play_state = state::PlayState::Connecting(station.name.clone());
                self.state.start_time = std::time::Instant::now();
                send_cmd(self, MpvCmd::LoadUrl(station.url));
            }
            changed = true;
        }
        if let state::PlayState::Connecting(name) = &self.state.play_state {
            let elapsed = self.state.start_time.elapsed().as_secs();
            if elapsed >= 10 {
                log::info!(
                    "handle_tick: Connecting timeout ({elapsed}s) for {name}, attempt {}",
                    self.state.retry_attempt,
                );
                if self.state.retry_attempt >= state::MAX_RETRIES {
                    self.state.play_state = state::PlayState::Error(format!(
                        "timed out connecting to {name} after {} attempts",
                        state::MAX_RETRIES
                    ));
                    self.state.retry_deadline = None;
                } else {
                    let delay_ms = state::retry_delay_ms(self.state.retry_attempt);
                    self.state.retry_deadline = Some(
                        std::time::Instant::now() + std::time::Duration::from_millis(delay_ms),
                    );
                    self.state.play_state = state::PlayState::Retrying(name.clone());
                    self.state.retry_attempt += 1;
                }
                changed = true;
            }
        }
        if let state::PlayState::Retrying(name) = &self.state.play_state {
            if let Some(deadline) = self.state.retry_deadline {
                if std::time::Instant::now() >= deadline {
                    let name = name.clone();
                    log::info!("handle_tick: Retrying complete for {name}, re-loading");
                    self.state.retry_mode = true;
                    self.state.last_metadata.clear();
                    self.state.song_title.clear();
                    self.state.track_info = None;
                    self.state.clear_lyrics();
                    self.state.lyrics_loading = true;
                    self.reset_mpv();
                    changed = true;
                }
            }
        }
        // ── mpv thread heartbeat ──────────────────────────────────
        // If the mpv thread stops advancing its heartbeat while we are
        // actively waiting for a response, the thread is likely stuck in
        // a blocking mpv internal call (e.g. TCP connect to a broken
        // stream).  Force a handle reset so the next LoadUrl runs on a
        // fresh mpv instance.
        //
        // The thread increments the counter once per event-loop iteration
        // (~100 ms via wait_event_raw(0.1)), so a 5 s stall = ~50 missed
        // iterations — unlikely to false-positive.
        const HEARTBEAT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
        if matches!(
            self.state.play_state,
            state::PlayState::Connecting(_) | state::PlayState::Playing(_)
        ) {
            let now = self.mpv_heartbeat.load(Ordering::Relaxed);
            if now == self.mpv_last_seen {
                let stuck = self
                    .mpv_stuck_since
                    .get_or_insert_with(std::time::Instant::now);
                if stuck.elapsed() >= HEARTBEAT_TIMEOUT {
                    log::warn!(
                        "mpv heartbeat stuck for {:?}, resetting handle",
                        stuck.elapsed(),
                    );
                    self.reset_mpv();
                    self.mpv_last_seen = self.mpv_heartbeat.load(Ordering::Relaxed);
                    self.mpv_stuck_since = None;
                }
            } else {
                self.mpv_last_seen = now;
                self.mpv_stuck_since = None;
            }
        }

        self.state.tick_counter += 1;
        if self.state.tick_scan_msg() {
            changed = true;
        }
        if self.state.search_mode && self.state.tick_counter.is_multiple_of(3) {
            changed = true;
        }
        self.dirty = changed;
    }

    fn handle_shutdown(&mut self) {
        if let Some(ref tx) = self.tx_cmd {
            let _ = tx.send(MpvCmd::Quit);
        }
        // Don't block shutdown — detach the thread and let the OS clean up.
        self.mpv_thread = None;
    }

    fn reset_mpv(&mut self) {
        // Wake up the old event loop so it can process Quit and exit promptly.
        // mpv_wakeup is safe to call from any thread; it interrupts
        // wait_event_raw and makes it return immediately.
        if let Some(ref wakeup) = self.mpv_wakeup {
            wakeup.wakeup();
        }
        // Send Quit — the thread will process it after being woken up.
        if let Some(ref tx) = self.tx_cmd {
            let _ = tx.send(MpvCmd::Quit);
        }
        // Join the old thread — it should exit quickly now.
        if let Some(handle) = self.mpv_thread.take() {
            let _ = handle.join();
        }
        self.mpv_wakeup = None;
        self.tx_cmd = None;
        self.rx_msg = None;
        self.tx_msg = None;

        // In test builds, always use mock channels — libmpv is not
        // thread-safe and parallel tests would corrupt its global state.
        let mpv_result = if cfg!(test) {
            Err::<(Mpv, Vec<String>), Box<dyn std::error::Error>>("test mode".into())
        } else {
            Mpv::new()
        };
        let (mut mpv, _warns) = match mpv_result {
            Ok(v) => v,
            Err(e) => {
                log::warn!("reset_mpv: failed to create mpv, using mock channels: {e}");
                let (tx_msg, rx_msg) = mpsc::channel::<MpvMsg>();
                let (tx_cmd, _) = mpsc::channel::<MpvCmd>();
                self.tx_cmd = Some(tx_cmd);
                self.rx_msg = Some(rx_msg);
                self.tx_msg = Some(tx_msg);
                if let Some(idx) = self.state.current_station {
                    let station = self.state.stations[idx].clone();
                    self.state.play_state = state::PlayState::Connecting(station.name.clone());
                    self.state.start_time = std::time::Instant::now();
                    send_cmd(self, MpvCmd::LoadUrl(station.url));
                }
                return;
            }
        };
        if let Err(e) = mpv.observe_property(0, "metadata") {
            log::warn!("mpv observe_property(metadata) failed: {e}");
        }
        if let Err(e) = mpv.observe_property(1, "media-title") {
            log::warn!("mpv observe_property(media-title) failed: {e}");
        }
        if let Err(e) = mpv.observe_property(2, "volume") {
            log::warn!("mpv observe_property(volume) failed: {e}");
        }
        if let Err(e) = mpv.set_volume(self.state.volume) {
            log::warn!("mpv set_volume failed: {e}");
        }

        self.mpv_wakeup = Some(mpv.make_wakeup());

        let (tx_msg, rx_msg) = mpsc::channel::<MpvMsg>();
        let (tx_cmd, rx_cmd) = mpsc::channel::<MpvCmd>();
        let tx_msg_mpv = tx_msg.clone();
        let hb = self.mpv_heartbeat.clone();

        let handle = thread::spawn(move || {
            // Duplicated from handle_init — kept in sync manually.
            loop {
                hb.fetch_add(1, Ordering::Relaxed);
                let ev = mpv.wait_event_raw(0.1);
                if let Some(ev) = ev {
                    let id = ev.event_id;
                    let name = match id {
                        player::MPV_EVENT_SHUTDOWN => "SHUTDOWN",
                        player::MPV_EVENT_FILE_LOADED => "FILE_LOADED",
                        player::MPV_EVENT_PLAYBACK_RESTART => "PLAYBACK_RESTART",
                        player::MPV_EVENT_PROPERTY_CHANGE => "PROPERTY_CHANGE",
                        player::MPV_EVENT_END_FILE => "END_FILE",
                        7 => "START_FILE",
                        _ => "OTHER",
                    };
                    log::info!("reset_mpv thread: event {name} (id={id})");
                    if id == player::MPV_EVENT_SHUTDOWN {
                        break;
                    }
                    if id == player::MPV_EVENT_FILE_LOADED {
                        let title = mpv
                            .metadata_title()
                            .ok()
                            .flatten()
                            .or_else(|| mpv.media_title().ok().flatten())
                            .unwrap_or_default();
                        let _ = tx_msg_mpv.send(MpvMsg::FileLoaded(title));
                    }
                    if id == player::MPV_EVENT_PLAYBACK_RESTART {
                        let title = mpv
                            .metadata_title()
                            .ok()
                            .flatten()
                            .or_else(|| mpv.media_title().ok().flatten())
                            .unwrap_or_default();
                        let _ = tx_msg_mpv.send(MpvMsg::FileLoaded(title));
                    }
                    if id == player::MPV_EVENT_PROPERTY_CHANGE {
                        if ev.data.is_null() {
                            continue;
                        }
                        let prop: &player::MpvEventProperty = unsafe { &*(ev.data as *const _) };
                        if prop.name.is_null() {
                            continue;
                        }
                        let name = unsafe {
                            std::ffi::CStr::from_ptr(prop.name)
                                .to_string_lossy()
                                .to_string()
                        };
                        if name == "metadata" {
                            let t = mpv.metadata_title().ok().flatten().unwrap_or_default();
                            let _ = tx_msg_mpv.send(MpvMsg::Metadata(t));
                        } else if name == "media-title" {
                            let t = mpv.media_title().ok().flatten().unwrap_or_default();
                            let _ = tx_msg_mpv.send(MpvMsg::Metadata(t));
                        }
                    }
                    if id == player::MPV_EVENT_END_FILE {
                        if ev.data.is_null() {
                            continue;
                        }
                        let ef: &player::MpvEventEndFile = unsafe { &*(ev.data as *const _) };
                        let _ = tx_msg_mpv.send(MpvMsg::EndFile(ef.reason));
                    }
                }
                while let Ok(cmd) = rx_cmd.try_recv() {
                    match cmd {
                        MpvCmd::LoadUrl(mut url) => {
                            loop {
                                match rx_cmd.try_recv() {
                                    Ok(MpvCmd::LoadUrl(new_url)) => url = new_url,
                                    Ok(MpvCmd::Stop) => {
                                        if let Err(e) = mpv.stop() {
                                            log::warn!("mpv stop failed: {e}");
                                        }
                                    }
                                    Ok(MpvCmd::SetVolume(v)) => {
                                        if let Err(e) = mpv.set_volume(v) {
                                            log::warn!("mpv set_volume failed: {e}");
                                        }
                                    }
                                    Ok(MpvCmd::Quit) => {
                                        mpv.destroy();
                                        return;
                                    }
                                    Err(_) => break,
                                }
                            }
                            let _ = mpv.stop();
                            while let Some(stale) = mpv.wait_event_raw(0.0) {
                                if stale.event_id == player::MPV_EVENT_SHUTDOWN {
                                    break;
                                }
                            }
                            log::info!("reset_mpv thread: calling load_url");
                            match mpv.load_url(&url) {
                                Ok(()) => {}
                                Err(e) => {
                                    log::warn!("reset_mpv thread: load_url failed: {e}");
                                    let _ = mpv.stop();
                                    while let Some(stale) = mpv.wait_event_raw(0.0) {
                                        if stale.event_id == player::MPV_EVENT_SHUTDOWN {
                                            break;
                                        }
                                    }
                                    match mpv.load_url(&url) {
                                        Ok(()) => {}
                                        Err(e2) => {
                                            log::warn!(
                                                "reset_mpv thread: load_url retry also failed: {e2}"
                                            );
                                            let _ = tx_msg_mpv.send(MpvMsg::MpvReset);
                                        }
                                    }
                                }
                            }
                        }
                        MpvCmd::Stop => {
                            if let Err(e) = mpv.stop() {
                                log::warn!("mpv stop failed: {e}");
                            }
                        }
                        MpvCmd::SetVolume(v) => {
                            if let Err(e) = mpv.set_volume(v) {
                                log::warn!("mpv set_volume failed: {e}");
                            }
                        }
                        MpvCmd::Quit => {
                            mpv.destroy();
                            return;
                        }
                    }
                }
            }
            mpv.destroy();
        });

        self.mpv_thread = Some(handle);
        self.tx_cmd = Some(tx_cmd);
        self.rx_msg = Some(rx_msg);
        self.tx_msg = Some(tx_msg);
        // Queue LoadUrl for the current station on the new mpv handle.
        // (Both this success path and the error path above must do this.)
        if let Some(idx) = self.state.current_station {
            let station = self.state.stations[idx].clone();
            self.state.play_state = state::PlayState::Connecting(station.name.clone());
            self.state.start_time = std::time::Instant::now();
            send_cmd(self, MpvCmd::LoadUrl(station.url));
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "favorites" {
            let favs: std::collections::HashSet<String> = match value {
                Some(json) => serde_json::from_str(&json).unwrap_or_default(),
                None => std::collections::HashSet::new(),
            };
            self.state.set_favorites(favs);
            self.dirty = true;
            self.pending_request = Some(PluginRequest::DbGet {
                key: "volume".into(),
            });
        } else if key == "volume" {
            if let Some(json) = value {
                if let Ok(vol) = serde_json::from_str::<i64>(&json) {
                    self.state.volume = vol.clamp(0, 100);
                    send_cmd(self, MpvCmd::SetVolume(self.state.volume));
                }
            }
            self.dirty = true;
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        if self.state.search_mode {
            return vec![
                ("↵".into(), "play".into()),
                ("↑↓".into(), "navigate".into()),
                ("⌫".into(), "delete".into()),
            ];
        }
        let mut hints: Vec<(String, String)> = Vec::new();
        if self.state.show_lyrics && self.state.lyrics_focused {
            hints.push(("tab".into(), "stations".into()));
            hints.push(("l".into(), "hide lyrics".into()));
        } else if self.state.show_lyrics {
            hints.push(("tab".into(), "lyrics".into()));
            hints.push(("l".into(), "hide lyrics".into()));
        } else if !self.state.lyrics_text.is_empty() || self.state.lyrics_loading {
            hints.push(("l".into(), "show lyrics".into()));
        }
        if !self.state.query.is_empty() {
            hints.push(("c".into(), "clear".into()));
        }
        hints.push(("+/-".into(), "volume".into()));
        hints
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = if let Some(ref err) = self.init_error {
                vec![RenderCmd::Text {
                    x: 0,
                    y: 0,
                    text: err.clone(),
                    fg: Some(self.theme.error),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                }]
            } else {
                ui::render_ui(&self.state, &self.theme, self.area.w, self.area.h)
            };
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn palette_commands() -> Vec<(String, String)> {
    vec![]
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

    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        if let Ok(null) = std::fs::File::open("/dev/null") {
            unsafe {
                libc::dup2(null.as_raw_fd(), libc::STDERR_FILENO);
            }
        }
    }

    let mut reader = BufReader::new(std::io::stdin().lock());

    let mut app = App::new();
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: HostMsg = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("[radio] parse error: {e}: {line}");
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
                    HostMsg::PaletteCommand { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::PluginMessage { .. } => {
                        // Plugin-to-plugin messaging — radio player does not
                        // participate in this yet, but we must handle the variant
                        // to keep the match exhaustive.
                        respond(&mut app, false);
                    }
                    HostMsg::Mouse { event } => {
                        let consumed = app.handle_mouse(event);
                        respond(&mut app, consumed);
                    }
                    HostMsg::UserUpdate { user } => {
                        app.user = user;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    HostMsg::DbValue { key, value } => {
                        app.handle_db_value(&key, value);
                        respond(&mut app, false);
                    }
                    HostMsg::Shutdown => {
                        app.handle_shutdown();
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PlayState;
    use crate::state::RadioState;
    use crate::stations::Station;
    use rusqlite::Connection;

    /// Create an in-memory SQLite db with the same `user_data` schema as
    /// santui.db.  Returns the connection.
    fn user_data_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_data (
                plugin  TEXT NOT NULL,
                user_id TEXT NOT NULL,
                key     TEXT NOT NULL,
                value   TEXT NOT NULL,
                PRIMARY KEY (plugin, user_id, key)
            )",
        )
        .unwrap();
        conn
    }

    fn make_stations(n: usize) -> Vec<Station> {
        (0..n)
            .map(|i| Station {
                name: format!("Station {i}"),
                url: format!("http://example.com/{i}"),
                country: String::new(),
                genre: String::new(),
            })
            .collect()
    }

    fn default_theme() -> ThemeData {
        ThemeData {
            text: [220, 220, 220],
            text_muted: [140, 140, 140],
            accent: [157, 124, 216],
            highlight: [250, 178, 131],
            logo: [255, 185, 0],
            background: [20, 20, 20],
            background_panel: [20, 20, 20],
            background_overlay: [10, 10, 10],
            border: [250, 178, 131],
            success: [127, 216, 143],
            error: [224, 108, 117],
            inverted_text: [20, 20, 20],
        }
    }

    fn base_app_with(n: usize, db: Option<rusqlite::Connection>) -> App {
        App {
            state: RadioState::new(make_stations(n)),
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            tx_cmd: None,
            rx_msg: None,
            tx_msg: None,
            mpv_thread: None,
            mpv_wakeup: None,
            mpv_heartbeat: Arc::new(AtomicU64::new(0)),
            mpv_last_seen: 0,
            mpv_stuck_since: None,
            init_error: None,
            dirty: true,
            cached_commands: Vec::new(),
            user: None,
            db,
            pending_request: None,
        }
    }

    fn base_app() -> App {
        base_app_with(5, None)
    }

    // ── search mode ────────────────────────────────────────────────────

    #[test]
    fn search_esc_exits_and_clears_query() {
        let mut app = base_app();
        app.state.search_mode = true;
        app.state.query = "test".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(!app.state.search_mode);
        assert!(app.state.query.is_empty());
    }

    #[test]
    fn search_enter_plays_selected() {
        let mut app = base_app();
        app.state.search_mode = true;
        assert!(app.handle_key(IpcKey::Enter));
        assert!(!app.state.search_mode);
        assert_eq!(app.state.current_station, Some(0));
        assert!(matches!(app.state.play_state, PlayState::Connecting(_)));
    }

    #[test]
    fn search_enter_with_empty_filtered_does_nothing() {
        let mut app = base_app();
        app.state.search_mode = true;
        app.state.filtered.clear();
        app.state.selected = 0;
        assert!(app.handle_key(IpcKey::Enter));
        assert!(!app.state.search_mode);
        assert_eq!(app.state.current_station, None);
    }

    #[test]
    fn search_backspace_removes_char() {
        let mut app = base_app();
        app.state.search_mode = true;
        app.state.query = "abc".into();
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.query, "ab");
    }

    #[test]
    fn search_backspace_empty_query_does_not_panic() {
        let mut app = base_app();
        app.state.search_mode = true;
        assert!(app.handle_key(IpcKey::Backspace));
        assert!(app.state.query.is_empty());
    }

    #[test]
    fn search_char_adds_to_query() {
        let mut app = base_app();
        app.state.search_mode = true;
        assert!(app.handle_key(IpcKey::Char('x')));
        assert_eq!(app.state.query, "x");
    }

    #[test]
    fn search_control_char_not_added() {
        let mut app = base_app();
        app.state.search_mode = true;
        assert!(!app.handle_key(IpcKey::Char('\n')));
        assert!(app.state.query.is_empty());
    }

    #[test]
    fn search_char_applies_filter() {
        let mut app = base_app();
        app.state.search_mode = true;
        app.handle_key(IpcKey::Char('0'));
        assert_eq!(app.state.filtered.len(), 1);
        assert_eq!(app.state.filtered[0], 0);
    }

    #[test]
    fn search_up_moves_selection_back() {
        let mut app = base_app();
        app.state.search_mode = true;
        app.state.selected = 2;
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 1);
    }

    #[test]
    fn search_up_at_top_stays() {
        let mut app = base_app();
        app.state.search_mode = true;
        app.state.selected = 0;
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 0);
    }

    #[test]
    fn search_down_moves_selection_forward() {
        let mut app = base_app();
        app.state.search_mode = true;
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 1);
    }

    #[test]
    fn search_down_at_end_stays() {
        let mut app = base_app();
        app.state.search_mode = true;
        app.state.selected = 4;
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 4);
    }

    #[test]
    fn search_unhandled_key_returns_false() {
        let mut app = base_app();
        app.state.search_mode = true;
        assert!(!app.handle_key(IpcKey::F(1)));
    }

    // ── normal mode ────────────────────────────────────────────────────

    #[test]
    fn tab_toggles_lyrics_focus_when_showing() {
        let mut app = base_app();
        app.state.show_lyrics = true;
        app.state.lyrics_focused = false;
        assert!(app.handle_key(IpcKey::Tab));
        assert!(app.state.lyrics_focused);
        assert!(app.handle_key(IpcKey::Tab));
        assert!(!app.state.lyrics_focused);
    }

    #[test]
    fn tab_noop_when_lyrics_hidden() {
        let mut app = base_app();
        app.state.show_lyrics = false;
        app.state.lyrics_focused = false;
        assert!(app.handle_key(IpcKey::Tab));
        assert!(!app.state.lyrics_focused);
    }

    #[test]
    fn up_moves_selection_back() {
        let mut app = base_app();
        app.state.selected = 3;
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 2);
    }

    #[test]
    fn up_when_lyrics_focused_scrolls_lyrics() {
        let mut app = base_app();
        app.state.show_lyrics = true;
        app.state.lyrics_focused = true;
        app.state.lyrics_text = "a\nb\nc".into();
        app.state.lyrics_scroll = 2;
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.lyrics_scroll, 1);
    }

    #[test]
    fn down_moves_selection_forward() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 1);
    }

    #[test]
    fn down_when_lyrics_focused_scrolls_lyrics() {
        let mut app = base_app();
        app.state.show_lyrics = true;
        app.state.lyrics_focused = true;
        app.state.lyrics_text = (0..30)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.lyrics_scroll, 1);
    }

    #[test]
    fn pageup_moves_selection_up() {
        let mut app = base_app_with(30, None);
        app.state.selected = 20;
        assert!(app.handle_key(IpcKey::PageUp));
        assert_eq!(app.state.selected, 6);
    }

    #[test]
    fn pageup_when_lyrics_focused_scrolls_up() {
        let mut app = base_app();
        app.state.show_lyrics = true;
        app.state.lyrics_focused = true;
        app.state.lyrics_text = "a\nb\nc".into();
        app.state.lyrics_scroll = 2;
        assert!(app.handle_key(IpcKey::PageUp));
        assert_eq!(app.state.lyrics_scroll, 0);
    }

    #[test]
    fn pagedown_moves_selection_down() {
        let mut app = base_app_with(30, None);
        app.state.selected = 0;
        assert!(app.handle_key(IpcKey::PageDown));
        assert_eq!(app.state.selected, 14);
    }

    #[test]
    fn pagedown_when_lyrics_focused_scrolls_down() {
        let mut app = base_app();
        app.state.show_lyrics = true;
        app.state.lyrics_focused = true;
        app.state.lyrics_text = (0..30)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(app.handle_key(IpcKey::PageDown));
        assert_eq!(app.state.lyrics_scroll, 10);
    }

    #[test]
    fn c_clears_filter_when_query_non_empty() {
        let mut app = base_app();
        app.state.query = "gold".into();
        app.state.apply_filter();
        assert!(app.state.filtered.len() < 5);
        assert!(app.handle_key(IpcKey::Char('c')));
        assert!(app.state.query.is_empty());
        assert_eq!(app.state.filtered.len(), 5);
        assert_eq!(app.state.selected, 0);
    }

    #[test]
    fn c_noop_when_query_empty() {
        let mut app = base_app();
        assert!(app.state.query.is_empty());
        assert!(!app.handle_key(IpcKey::Char('c')));
    }

    #[test]
    fn c_noop_when_lyrics_focused() {
        let mut app = base_app();
        app.state.query = "gold".into();
        app.state.show_lyrics = true;
        app.state.lyrics_focused = true;
        assert!(!app.handle_key(IpcKey::Char('c')));
    }

    #[test]
    fn slash_enters_search_mode() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('/')));
        assert!(app.state.search_mode);
    }

    #[test]
    fn slash_noop_when_lyrics_focused() {
        let mut app = base_app();
        app.state.show_lyrics = true;
        app.state.lyrics_focused = true;
        assert!(!app.handle_key(IpcKey::Char('/')));
        assert!(!app.state.search_mode);
    }

    #[test]
    fn enter_plays_selected_station() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.current_station, Some(0));
        assert!(matches!(app.state.play_state, PlayState::Connecting(_)));
    }

    #[test]
    fn enter_noop_when_lyrics_focused() {
        let mut app = base_app();
        app.state.show_lyrics = true;
        app.state.lyrics_focused = true;
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.current_station, None);
        assert!(matches!(app.state.play_state, PlayState::Stopped));
    }

    #[test]
    fn enter_with_empty_filtered_does_nothing() {
        let mut app = base_app();
        app.state.filtered.clear();
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.current_station, None);
    }

    #[test]
    fn r_reloads_stations_from_db() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE stations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                country TEXT NOT NULL DEFAULT '',
                genre TEXT NOT NULL DEFAULT ''
            )",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO stations (name, url) VALUES (?1, ?2)",
            rusqlite::params!["ABC", "http://abc"],
        )
        .unwrap();

        let mut app = base_app_with(3, Some(conn));
        assert!(app.handle_key(IpcKey::Char('r')));
        assert_eq!(app.state.stations.len(), 1);
        assert_eq!(app.state.stations[0].name, "ABC");
    }

    #[test]
    fn r_noop_when_lyrics_focused() {
        let mut app = base_app();
        app.state.show_lyrics = true;
        app.state.lyrics_focused = true;
        assert!(app.handle_key(IpcKey::Char('r')));
        assert_eq!(app.state.stations.len(), 5);
    }

    #[test]
    fn r_noop_when_no_db() {
        let mut app = base_app();
        assert_eq!(app.state.stations.len(), 5);
        assert!(app.handle_key(IpcKey::Char('r')));
        assert_eq!(app.state.stations.len(), 5);
    }

    #[test]
    fn s_stops_playback() {
        let mut app = base_app();
        app.state.play_state = PlayState::Playing("test".into());
        app.state.current_station = Some(0);
        app.state.song_title = "some song".into();
        assert!(app.handle_key(IpcKey::Char('s')));
        assert!(matches!(app.state.play_state, PlayState::Stopped));
        assert_eq!(app.state.current_station, None);
        assert!(app.state.song_title.is_empty());
    }

    #[test]
    fn plus_increases_volume() {
        let mut app = base_app();
        app.state.volume = 50;
        assert!(app.handle_key(IpcKey::Char('+')));
        assert_eq!(app.state.volume, 52);
    }

    #[test]
    fn equals_increases_volume() {
        let mut app = base_app();
        app.state.volume = 50;
        assert!(app.handle_key(IpcKey::Char('=')));
        assert_eq!(app.state.volume, 52);
    }

    #[test]
    fn minus_decreases_volume() {
        let mut app = base_app();
        app.state.volume = 50;
        assert!(app.handle_key(IpcKey::Char('-')));
        assert_eq!(app.state.volume, 48);
    }

    #[test]
    fn l_toggles_lyrics() {
        let mut app = base_app();
        assert!(!app.state.show_lyrics);
        assert!(app.handle_key(IpcKey::Char('l')));
        assert!(app.state.show_lyrics);
        assert!(!app.state.lyrics_focused);
        assert_eq!(app.state.lyrics_scroll, 0);
    }

    #[test]
    fn unhandled_key_returns_false() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Left));
        assert!(!app.handle_key(IpcKey::Right));
        assert!(!app.handle_key(IpcKey::Home));
        assert!(!app.handle_key(IpcKey::End));
        assert!(!app.handle_key(IpcKey::Insert));
        assert!(!app.handle_key(IpcKey::Delete));
        assert!(!app.handle_key(IpcKey::BackTab));
        assert!(!app.handle_key(IpcKey::F(2)));
    }

    #[test]
    fn handle_key_sets_dirty_and_clears_scan_msg() {
        let mut app = base_app();
        app.state.set_scan_msg("hello".into());
        app.dirty = false;
        assert!(app.handle_key(IpcKey::Char('/')));
        assert!(app.dirty);
        assert!(app.state.scan_msg.is_none());
    }

    // ── favorites persistence ───────────────────────────────────────────

    #[test]
    fn handle_db_value_loads_favorites() {
        let mut app = base_app_with(5, None);
        let favs = r#"["http://example.com/1","http://example.com/3"]"#;
        app.handle_db_value("favorites", Some(favs.to_string()));

        assert_eq!(app.state.favorites_count(), 2);
        assert!(app.state.is_favorite("http://example.com/1"));
        assert!(app.state.is_favorite("http://example.com/3"));
        assert!(!app.state.is_favorite("http://example.com/0"));
        assert!(!app.state.is_favorite("http://example.com/2"));
    }

    #[test]
    fn handle_db_value_none_clears_favorites() {
        let mut app = base_app_with(5, None);
        // Pre-populate with some favorites
        let mut favs = std::collections::HashSet::new();
        favs.insert("http://example.com/1".into());
        app.state.set_favorites(favs);
        assert_eq!(app.state.favorites_count(), 1);

        // Simulate DbValue with None (no data in DB yet)
        app.handle_db_value("favorites", None);
        assert_eq!(app.state.favorites_count(), 0);
    }

    #[test]
    fn handle_db_value_ignores_other_keys() {
        let mut app = base_app_with(5, None);
        app.handle_db_value("some_other_key", Some(r#"["http://example.com/1"]"#.into()));
        assert_eq!(app.state.favorites_count(), 0);
    }

    #[test]
    fn favorites_survive_restart_roundtrip() {
        // This test simulates the full delete+reinstall cycle:
        //   1. User saves favorites (DbSet)
        //   2. Plugin is killed and restarted
        //   3. Host reads from DB (DbGet) and sends DbValue
        //   4. Plugin loads the favorites

        // Step 1: create an in-memory DB with user_data schema (like santui.db)
        let conn = user_data_db();

        // Simulate the host's DbSet handler — store favorites for
        // (plugin="radio-stream-player", user_id="_", key="favorites")
        let stored_json = r#"["http://example.com/1","http://example.com/3"]"#;
        conn.execute(
            "INSERT INTO user_data (plugin, user_id, key, value)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(plugin, user_id, key) DO UPDATE SET value = excluded.value",
            rusqlite::params!["radio-stream-player", "_", "favorites", stored_json],
        )
        .unwrap();

        // Step 2: simulate plugin restart — create a fresh App
        let mut app = base_app_with(5, None);

        // Step 3: simulate the host's DbGet handler — read from DB
        let value_from_db: Option<String> = conn
            .query_row(
                "SELECT value FROM user_data
                 WHERE plugin = ?1 AND user_id = ?2 AND key = ?3",
                rusqlite::params!["radio-stream-player", "_", "favorites"],
                |row| row.get(0),
            )
            .ok();

        // Step 4: simulate the host sending HostMsg::DbValue
        app.handle_db_value("favorites", value_from_db);

        // Verify favorites survived
        assert_eq!(app.state.favorites_count(), 2);
        assert!(app.state.is_favorite("http://example.com/1"));
        assert!(app.state.is_favorite("http://example.com/3"));
        assert!(!app.state.is_favorite("http://example.com/0"));

        // Step 5: verify interactive operations still work
        // Space on station 0 adds it as favorite
        app.pending_request = None;
        assert!(app.handle_key(IpcKey::Char(' ')));
        assert!(app.state.is_favorite("http://example.com/0"));
        assert_eq!(app.state.favorites_count(), 3);

        // Verify the DbSet request has the correct JSON
        match &app.pending_request {
            Some(PluginRequest::DbSet { key, value }) => {
                assert_eq!(key, "favorites");
                let parsed: Vec<String> = serde_json::from_str(value).unwrap_or_default();
                assert!(parsed.contains(&"http://example.com/0".to_string()));
                assert!(parsed.contains(&"http://example.com/1".to_string()));
                assert!(parsed.contains(&"http://example.com/3".to_string()));
                assert_eq!(parsed.len(), 3);
            }
            other => panic!("expected Some(DbSet), got {other:?}"),
        }
    }

    #[test]
    fn favorites_filter_combines_with_query() {
        let mut app = base_app_with(5, None);
        let favs = r#"["http://example.com/1","http://example.com/3"]"#;
        app.handle_db_value("favorites", Some(favs.to_string()));

        // Enable favorites-only filter
        app.state.show_favorites_only = true;
        app.state.apply_filter();
        assert_eq!(app.state.filtered.len(), 2);

        // Combine with text query — station 1 has name "Station 1", station 3 is "Station 3"
        app.state.query = "3".into();
        app.state.apply_filter();
        assert_eq!(app.state.filtered.len(), 1);
        assert_eq!(app.state.filtered[0], 3);

        // Clear query, should go back to both favorites
        app.state.query.clear();
        app.state.apply_filter();
        assert_eq!(app.state.filtered.len(), 2);
    }

    // ── simulation: user scenario ────────────────────────────────────

    /// Simulate pushing an mpv event into the app's message channel so
    /// handle_tick can process it through rx.try_recv().
    fn push_msg(app: &mut App, msg: MpvMsg) {
        if let Some(ref tx) = app.tx_msg {
            let _ = tx.send(msg);
        }
    }

    /// Create an App with live mpsc channels so we can push MpvMsg events
    /// and test handle_tick.
    fn app_with_channels(n: usize) -> App {
        let (tx_msg, rx_msg) = mpsc::channel();
        let (tx_cmd, _rx_cmd) = mpsc::channel();
        App {
            state: RadioState::new(make_stations(n)),
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            tx_cmd: Some(tx_cmd),
            rx_msg: Some(rx_msg),
            tx_msg: Some(tx_msg),
            mpv_thread: None,
            mpv_wakeup: None,
            mpv_heartbeat: Arc::new(AtomicU64::new(0)),
            mpv_last_seen: 0,
            mpv_stuck_since: None,
            init_error: None,
            dirty: true,
            cached_commands: Vec::new(),
            user: None,
            db: None,
            pending_request: None,
        }
    }

    /// Simulate retry deadline arrival: set retry_deadline to past, then call handle_tick.
    fn fire_retry(app: &mut App) {
        if app.state.retry_deadline.is_some() {
            app.state.retry_deadline =
                Some(std::time::Instant::now() - std::time::Duration::from_millis(1));
        }
        app.handle_tick();
    }

    #[test]
    fn simulate_user_switching_after_bad_station() {
        let mut app = app_with_channels(3);

        // ── Step 1: play 011.FM ──────────────────────────────────────
        app.handle_key(IpcKey::Enter);
        assert!(matches!(app.state.play_state, PlayState::Connecting(_)));
        assert_eq!(app.state.current_station, Some(0));

        // 011.FM connects
        push_msg(&mut app, MpvMsg::FileLoaded("011.FM - Song".into()));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Playing(ref n) if n == "Station 0"));
        assert_eq!(app.state.song_title, "011.FM - Song");

        // ── Step 2: switch to 1.FM ───────────────────────────────────
        // 011.FM metadata event arrives BEFORE LoadUrl is processed
        // (mpv thread polls events before commands)
        app.state.selected = 1;
        push_msg(&mut app, MpvMsg::Metadata("011.FM - Extra Metadata".into()));
        app.handle_key(IpcKey::Enter);

        // Metadata should be DROPPED because state is Connecting("Station 1"), not Playing
        assert!(matches!(app.state.play_state, PlayState::Connecting(ref n) if n == "Station 1"));

        // 1.FM's loadfile replaces 011.FM → END_FILE for old stream
        push_msg(&mut app, MpvMsg::EndFile(player::MPV_END_FILE_REASON_EOF));
        app.handle_tick();
        // EndFile while Connecting → dropped, still Connecting
        assert!(matches!(app.state.play_state, PlayState::Connecting(ref n) if n == "Station 1"));

        // 1.FM loads (FILE_LOADED)
        push_msg(&mut app, MpvMsg::FileLoaded("1.FM - 80s".into()));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Playing(ref n) if n == "Station 1"));

        // ── Step 3: 1.FM has "no sound" ──────────────────────────────
        // 1.FM fails → END_FILE(ERROR)
        push_msg(&mut app, MpvMsg::EndFile(player::MPV_END_FILE_REASON_ERROR));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Retrying(ref n) if n == "Station 1"));
        assert_eq!(app.state.retry_attempt, 1);

        // ── Step 4: retry fires, queues LoadUrl ──────────────────────
        fire_retry(&mut app);
        assert!(matches!(app.state.play_state, PlayState::Connecting(ref n) if n == "Station 1"));

        // ── Step 5: user presses Enter on 011.FM BEFORE mpv thread
        // processes the retry's LoadUrl ───────────────────────────────
        // This is the critical scenario: two LoadUrl commands queued.
        // We simulate the mpv thread's event stream after processing
        // BOTH commands with the pre-drain active.

        // First, a stale METADATA arrives (from old 1.FM stream,
        // polled by mpv thread BEFORE the command loop)
        push_msg(&mut app, MpvMsg::Metadata("1.FM - Stale Metadata".into()));

        app.state.selected = 0;
        app.handle_key(IpcKey::Enter);
        assert!(matches!(app.state.play_state, PlayState::Connecting(ref n) if n == "Station 0"));

        // The stale Metadata arrived during Connecting → should be DROPPED
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Connecting(ref n) if n == "Station 0"));

        // Now simulate the mpv thread processing BOTH LoadUrls:
        // retry's LoadUrl for 1.FM, then user's LoadUrl for 011.FM,
        // with pre-drain consuming stale events from the first.

        // Events generated (in order with pre-drain):
        // Pre-drain of LoadUrl(1fm): nothing stale → loadfile 1fm replace
        // Pre-drain of LoadUrl(011): processes loadfile 1fm → END_FILE(old) → consumed
        //                           → processes FILE_LOADED(1fm) if cached → consumed by pre-drain!
        //                           → loadfile 011 replace
        // wait_event_raw(0.1): END_FILE(1fm from replacement) → FILE_LOADED(011)

        // So the events arriving at main thread after both commands are processed:
        // EndFile(EOF) for 1.FM being replaced by 011.FM
        push_msg(&mut app, MpvMsg::EndFile(player::MPV_END_FILE_REASON_EOF));
        app.handle_tick();
        // Connecting → dropped ✓
        assert!(matches!(app.state.play_state, PlayState::Connecting(ref n) if n == "Station 0"));

        // FileLoaded for 011.FM
        push_msg(&mut app, MpvMsg::FileLoaded("011.FM - Great Song".into()));
        app.handle_tick();
        // Connecting → Playing ✓
        assert!(matches!(app.state.play_state, PlayState::Playing(ref n) if n == "Station 0"));
        assert_eq!(app.state.song_title, "011.FM - Great Song");

        // Verify no spurious retry was triggered
        assert_eq!(app.state.retry_attempt, 0);
        assert!(app.state.retry_deadline.is_none());
    }

    #[test]
    fn simulate_stale_fileloaded_after_double_loadurl() {
        // Edge case: retry LoadUrl + user LoadUrl queued back-to-back.
        // mpv thread processes both; the first's FILE_LOADED event must
        // NOT confuse the second's state.
        let mut app = app_with_channels(3);

        // Start playing Station 0
        app.handle_key(IpcKey::Enter);
        push_msg(&mut app, MpvMsg::FileLoaded("Station 0 Song".into()));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Playing(_)));

        // Simulate failure → retry
        push_msg(&mut app, MpvMsg::EndFile(player::MPV_END_FILE_REASON_ERROR));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Retrying(_)));

        // Retry fires (queues LoadUrl for Station 0)
        fire_retry(&mut app);
        assert!(matches!(app.state.play_state, PlayState::Connecting(ref n) if n == "Station 0"));

        // User presses Enter on Station 1 BEFORE mpv thread processes retry
        app.state.selected = 1;
        app.handle_key(IpcKey::Enter);
        assert!(matches!(app.state.play_state, PlayState::Connecting(ref n) if n == "Station 1"));

        // Simulate the mpv thread's pre-drain consuming stale events,
        // then only the correct events arriving:

        // EndFile from loadfile replace in the SECOND (user's) LoadUrl
        push_msg(&mut app, MpvMsg::EndFile(player::MPV_END_FILE_REASON_EOF));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Connecting(ref n) if n == "Station 1"));

        // FileLoaded for Station 1 (the winning URL)
        push_msg(&mut app, MpvMsg::FileLoaded("Station 1 Rocks".into()));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Playing(ref n) if n == "Station 1"));
        assert_eq!(app.state.song_title, "Station 1 Rocks");

        // No spurious retry
        assert_eq!(app.state.retry_attempt, 0);
    }

    #[test]
    fn simulate_mpv_reset_recovers_playing() {
        // When the mpv thread sends MpvReset (load_url failed after
        // stop+retry), the main thread should recreate the mpv handle
        // and re-queue LoadUrl.  We simulate this by pushing MpvReset.
        let mut app = app_with_channels(3);

        // Start playing Station 0
        app.handle_key(IpcKey::Enter);
        push_msg(&mut app, MpvMsg::FileLoaded("Song 0".into()));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Playing(_)));

        // Simulate failure → retry
        push_msg(&mut app, MpvMsg::EndFile(player::MPV_END_FILE_REASON_ERROR));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Retrying(_)));

        // Simulate retry + connecting + mpv reset
        fire_retry(&mut app);
        assert!(matches!(app.state.play_state, PlayState::Connecting(_)));

        // MpvReset during Connecting → reset_mpv + re-queue LoadUrl
        push_msg(&mut app, MpvMsg::MpvReset);
        app.handle_tick();

        // After reset_mpv, state should be Connecting (with current_station)
        // and a new LoadUrl was queued (tx_cmd had it sent).
        assert!(matches!(app.state.play_state, PlayState::Connecting(_)));
        assert_eq!(app.state.current_station, Some(0));

        // Now simulate FileLoaded from the NEW mpv instance
        push_msg(&mut app, MpvMsg::FileLoaded("Song 0 Reborn".into()));
        app.handle_tick();
        assert!(matches!(app.state.play_state, PlayState::Playing(ref n) if n == "Station 0"));
        assert_eq!(app.state.song_title, "Song 0 Reborn");

        // No retry was incremented because MpvReset is not a retry
        assert_eq!(app.state.retry_attempt, 0);
    }

    /// Integration test that exercises a real mpv handle against the actual
    /// station URLs from the user's database.  Tests whether 1.FM's stream
    /// corrupts the mpv handle so that subsequent streams fail.
    ///
    /// This test requires network access and libmpv.  It is marked `ignore`
    /// so it does not run in the normal `cargo test` suite.  Run it with:
    ///
    ///     cargo test -p santui-radio-stream-player test_mpv_urls_integration -- --ignored
    #[test]
    #[ignore]
    fn test_mpv_urls_integration() {
        let url_011 = "https://listen.011fm.com/stream01";
        let url_1fm = "https://strmreg.1.fm/back280s_mobile_mp3";

        let (mut mpv, warns) =
            player::Mpv::new().expect("Failed to create mpv — is libmpv installed?");
        for w in &warns {
            eprintln!("mpv warn: {w}");
        }
        mpv.observe_property(0, "metadata").ok();
        mpv.observe_property(1, "media-title").ok();
        mpv.set_volume(50).ok();

        /// Load a URL and wait for either FILE_LOADED or END_FILE.
        /// Returns the event IDs that arrived.
        fn load_and_wait(mpv: &player::Mpv, label: &str, url: &str, timeout_secs: f64) -> Vec<u32> {
            eprintln!("\n--- {label}: {url} ---");
            mpv.load_url(url).expect("load_url returned error");
            let mut events = Vec::new();
            let deadline =
                std::time::Instant::now() + std::time::Duration::from_secs_f64(timeout_secs);
            while std::time::Instant::now() < deadline {
                if let Some(ev) = mpv.wait_event_raw(0.2) {
                    events.push(ev.event_id);
                    let name = match ev.event_id {
                        1 => "SHUTDOWN",
                        6 => "FILE_LOADED",
                        18 => "PLAYBACK_RESTART",
                        22 => "PROPERTY_CHANGE",
                        25 => "END_FILE",
                        _ => "OTHER",
                    };
                    eprintln!("  event {name} (id={})", ev.event_id);
                    if ev.event_id == 6 || ev.event_id == 25 || ev.event_id == 1 {
                        break;
                    }
                }
            }
            if events.is_empty() {
                eprintln!("  (no events within {timeout_secs}s timeout)");
            }
            events
        }

        // 1. Play 011.FM — must succeed
        let ev1 = load_and_wait(&mpv, "011.FM (first)", url_011, 15.0);
        assert!(
            ev1.contains(&6),
            "011.FM should FILE_LOADED on first play, got events: {ev1:?}"
        );

        // 2. Play 1.FM — expect either FILE_LOADED or END_FILE
        let ev2 = load_and_wait(&mpv, "1.FM - A List 80s Radio", url_1fm, 30.0);

        // 3. Play 011.FM again — this is the critical test
        let ev3 = load_and_wait(&mpv, "011.FM (after 1.FM)", url_011, 15.0);
        assert!(
            ev3.contains(&6),
            "mpv HANDLE CORRUPTED by 1.FM!  \
             011.FM got events {ev3:?} after 1.FM (got {ev2:?})",
        );

        eprintln!("\n✓ mpv handle survived 1.FM — no corruption detected");
        mpv.destroy();
    }
}
