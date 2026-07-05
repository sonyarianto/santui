mod database;
mod itunes;
mod lrclib;
mod player;
mod state;
mod stations;
mod ui;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::thread;

use ui::{HEADER_H, TABLE_TOP};

const LIST_OVERHEAD: u16 = TABLE_TOP + HEADER_H + 1 + 2; // top + search + sep + header + bottom + footer (blank + hints)

use player::Mpv;
use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData, UserData};

enum MpvMsg {
    Metadata(String),
    TrackInfo(u64, itunes::TrackInfo),
    Lyrics(u64, Option<lrclib::LyricsData>),
    EndFile(u32),
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
            init_error,
            dirty: true,
            cached_commands: Vec::new(),
            user: None,
            pending_request: Some(PluginRequest::DbGet {
                key: "favorites".into(),
            }),
        }
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

        let (tx_msg, rx_msg) = mpsc::channel::<MpvMsg>();
        let (tx_cmd, rx_cmd) = mpsc::channel::<MpvCmd>();

        let tx_msg_mpv = tx_msg.clone();

        let handle = thread::spawn(move || {
            loop {
                let ev = mpv.wait_event_raw(0.1);
                if let Some(ev) = ev {
                    let id = ev.event_id;
                    if id == player::MPV_EVENT_SHUTDOWN {
                        break;
                    }
                    if id == player::MPV_EVENT_FILE_LOADED {
                        if let Ok(Some(t)) = mpv.metadata_title() {
                            let _ = tx_msg_mpv.send(MpvMsg::Metadata(t));
                        } else if let Ok(Some(t)) = mpv.media_title() {
                            let _ = tx_msg_mpv.send(MpvMsg::Metadata(t));
                        }
                    }
                    if id == player::MPV_EVENT_PLAYBACK_RESTART {
                        let title = mpv
                            .metadata_title()
                            .ok()
                            .flatten()
                            .or_else(|| mpv.media_title().ok().flatten());
                        if let Some(title) = title {
                            let _ = tx_msg_mpv.send(MpvMsg::Metadata(title));
                        }
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
                            if let Ok(Some(t)) = mpv.metadata_title() {
                                let _ = tx_msg_mpv.send(MpvMsg::Metadata(t));
                            }
                        } else if name == "media-title" {
                            if let Ok(Some(t)) = mpv.media_title() {
                                let _ = tx_msg_mpv.send(MpvMsg::Metadata(t));
                            }
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
                        MpvCmd::LoadUrl(url) => {
                            if let Err(e) = mpv.load_url(&url) {
                                log::warn!("mpv load_url failed: {e}");
                            }
                            if let Some(title) = mpv.metadata_title().ok().flatten() {
                                let _ = tx_msg_mpv.send(MpvMsg::Metadata(title));
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

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                // "Search stations" — enter search mode
                self.state.search_mode = true;
                self.state.query.clear();
                self.state.filtered = (0..self.state.stations.len()).collect();
                self.state.selected = 0;
                self.state.scroll = 0;
                self.dirty = true;
            }
            1 => {
                // "Reload stations" — reload from DB
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
                self.dirty = true;
            }
            _ => {}
        }
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
                    if let Some(station) = self.state.selected_station().cloned() {
                        let idx = self.state.current_filtered_index();
                        self.state.current_station = Some(idx);
                        self.state.play_state = state::PlayState::Playing(station.name.to_string());
                        self.state.last_metadata.clear();
                        self.state.song_title.clear();
                        self.state.track_info = None;
                        self.state.clear_lyrics();
                        self.state.lyrics_loading = true;
                        self.state.start_time = std::time::Instant::now();
                        send_cmd(self, MpvCmd::Stop);
                        send_cmd(self, MpvCmd::LoadUrl(station.url));
                    }
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
                    self.state.lyrics_scroll_down(panel_h);
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
                    self.state.lyrics_scroll_up();
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
                    self.state.lyrics_scroll_down(panel_h);
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
                } else if let Some(station) = self.state.selected_station().cloned() {
                    let idx = self.state.current_filtered_index();
                    self.state.current_station = Some(idx);
                    self.state.play_state = state::PlayState::Playing(station.name.to_string());
                    self.state.last_metadata.clear();
                    self.state.song_title.clear();
                    self.state.track_info = None;
                    self.state.clear_lyrics();
                    self.state.lyrics_loading = true;
                    self.state.start_time = std::time::Instant::now();
                    send_cmd(self, MpvCmd::Stop);
                    send_cmd(self, MpvCmd::LoadUrl(station.url));
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
                true
            }
            IpcKey::Char('+') | IpcKey::Char('=') => {
                self.state.volume_up();
                send_cmd(self, MpvCmd::SetVolume(self.state.volume));
                true
            }
            IpcKey::Char('-') => {
                self.state.volume_down();
                send_cmd(self, MpvCmd::SetVolume(self.state.volume));
                true
            }
            IpcKey::Char('l') => {
                self.state.show_lyrics = !self.state.show_lyrics;
                self.state.lyrics_scroll = 0;
                self.state.lyrics_focused = self.state.show_lyrics;
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

    fn handle_tick(&mut self) {
        let mut changed = false;
        if let Some(ref rx) = self.rx_msg {
            while let Ok(msg) = rx.try_recv() {
                changed = true;
                match msg {
                    MpvMsg::Metadata(title) => {
                        // Mpv can fire redundant PLAYBACK_RESTART or
                        // PROPERTY_CHANGE events for the same radio stream
                        // title (rebuffer, reconnect).  Skip to avoid
                        // clearing lyrics and re-fetching unnecessarily — a
                        // failed redundant fetch would overwrite good lyrics
                        // with empty via MpvMsg::Lyrics(None).
                        //
                        // Use last_metadata (not song_title) as the dedup key
                        // because TrackInfo may update song_title mid-stream.
                        if title == self.state.last_metadata {
                            continue;
                        }
                        self.state.last_metadata = title.clone();
                        self.state.song_title = title.clone();
                        self.state.track_info = None;
                        self.state.clear_lyrics();
                        self.state.lyrics_loading = true;
                        self.state.metadata_seq += 1;
                        let seq = self.state.metadata_seq;
                        let Some(tx) = self.tx_msg.clone() else {
                            continue;
                        };
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
                        if reason == player::MPV_END_FILE_REASON_EOF {
                            if let Some(idx) = self.state.current_station {
                                let station = self.state.stations[idx].clone();
                                send_cmd(self, MpvCmd::LoadUrl(station.url));
                            }
                        } else if reason == player::MPV_END_FILE_REASON_ERROR {
                            self.state.play_state =
                                state::PlayState::Error("connection lost".into());
                        }
                    }
                }
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
        if let Some(handle) = self.mpv_thread.take() {
            let _ = handle.join();
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
            hints.push(("l".into(), "hide".into()));
        } else if self.state.show_lyrics {
            hints.push(("tab".into(), "lyrics".into()));
            hints.push(("l".into(), "hide".into()));
        } else if !self.state.lyrics_text.is_empty() || self.state.lyrics_loading {
            hints.push(("l".into(), "lyrics".into()));
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
    vec![
        ("Radio".into(), "Search stations".into()),
        ("Radio".into(), "Reload stations".into()),
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
                    HostMsg::PaletteCommand { index } => {
                        app.handle_palette_command(index);
                        respond(&mut app, false);
                    }
                    HostMsg::PluginMessage { .. } => {
                        // Plugin-to-plugin messaging — radio player does not
                        // participate in this yet, but we must handle the variant
                        // to keep the match exhaustive.
                        respond(&mut app, false);
                    }
                    HostMsg::Mouse { .. } => {
                        respond(&mut app, false);
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
        assert!(matches!(app.state.play_state, PlayState::Playing(_)));
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
        assert_eq!(app.state.lyrics_scroll, 1);
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
        assert_eq!(app.state.lyrics_scroll, 1);
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
        assert!(matches!(app.state.play_state, PlayState::Playing(_)));
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
        assert!(app.state.lyrics_focused);
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
}
