mod database;
mod itunes;
mod player;
mod state;
mod stations;
mod ui;

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::thread;

use player::Mpv;
use santui_ipc::protocol::{Area, HostMsg, IpcKey, RenderCmd, ThemeData, UserData};

enum MpvMsg {
    Metadata(String),
    TrackInfo(itunes::TrackInfo),
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
    cached_json: Option<String>,
    user: Option<UserData>,
    db: rusqlite::Connection,
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
                (db, list, None)
            }
            Err(e) => {
                let err = format!("Database error: {e}");
                log::error!("{err}");
                let fallback =
                    rusqlite::Connection::open_in_memory().expect("in-memory DB fallback");
                (fallback, Vec::new(), Some(err))
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
            cached_json: None,
            user: None,
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
                let new_stations = crate::stations::reload(&self.db);
                let count = new_stations.len();
                self.state.stations = new_stations;
                self.state.set_query(String::new());
                self.state.selected = 0;
                self.state.scroll = 0;
                self.state.scan_msg = Some(format!("Reloaded {count} stations from DB"));
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn handle_key(&mut self, key: IpcKey) {
        self.state.scan_msg = None;
        self.dirty = true;
        if self.state.search_mode {
            match key {
                IpcKey::Esc => {
                    self.state.search_mode = false;
                    self.state.set_query(String::new());
                }
                IpcKey::Enter => {
                    self.state.search_mode = false;
                }
                IpcKey::Backspace => {
                    self.state.query.pop();
                    self.state.apply_filter();
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.state.query.push(c);
                    self.state.apply_filter();
                }
                IpcKey::Up => {
                    self.state.select_prev();
                    let max_visible = self.area.h.saturating_sub(6) as usize;
                    self.state.ensure_scroll_visible(max_visible.max(1));
                }
                IpcKey::Down => {
                    self.state.select_next();
                    let max_visible = self.area.h.saturating_sub(6) as usize;
                    self.state.ensure_scroll_visible(max_visible.max(1));
                }
                _ => {}
            }
            return;
        }
        match key {
            IpcKey::Up => {
                self.state.select_prev();
                let max_visible = self.area.h.saturating_sub(4) as usize;
                self.state.ensure_scroll_visible(max_visible.max(1));
            }
            IpcKey::Down => {
                self.state.select_next();
                let max_visible = self.area.h.saturating_sub(4) as usize;
                self.state.ensure_scroll_visible(max_visible.max(1));
            }
            IpcKey::PageUp => {
                let page = self.area.h.saturating_sub(4) as usize;
                self.state.select_page_up(page.max(1));
                self.state.ensure_scroll_visible(page.max(1));
            }
            IpcKey::PageDown => {
                let page = self.area.h.saturating_sub(4) as usize;
                self.state.select_page_down(page.max(1));
                self.state.ensure_scroll_visible(page.max(1));
            }
            IpcKey::Char('/') => {
                self.state.search_mode = true;
                self.state.query.clear();
                self.state.filtered = (0..self.state.stations.len()).collect();
                self.state.selected = 0;
                self.state.scroll = 0;
            }
            IpcKey::Enter => {
                if let Some(station) = self.state.selected_station().cloned() {
                    let idx = self.state.current_filtered_index();
                    self.state.current_station = Some(idx);
                    self.state.play_state = state::PlayState::Playing(station.name.to_string());
                    self.state.song_title.clear();
                    self.state.track_info = None;
                    self.state.start_time = std::time::Instant::now();
                    send_cmd(self, MpvCmd::Stop);
                    send_cmd(self, MpvCmd::LoadUrl(station.url));
                }
            }
            IpcKey::Char('r') => {
                let new_stations = crate::stations::reload(&self.db);
                let count = new_stations.len();
                self.state.stations = new_stations;
                self.state.set_query(String::new());
                self.state.selected = 0;
                self.state.scroll = 0;
                self.state.scan_msg = Some(format!("Reloaded {count} stations from DB"));
            }
            IpcKey::Char('s') => {
                send_cmd(self, MpvCmd::Stop);
                self.state.play_state = state::PlayState::Stopped;
                self.state.current_station = None;
                self.state.song_title.clear();
                self.state.track_info = None;
            }
            IpcKey::Char('+') | IpcKey::Char('=') => {
                self.state.volume_up();
                send_cmd(self, MpvCmd::SetVolume(self.state.volume));
            }
            IpcKey::Char('-') => {
                self.state.volume_down();
                send_cmd(self, MpvCmd::SetVolume(self.state.volume));
            }
            _ => {}
        }
    }

    fn handle_tick(&mut self) {
        let mut changed = false;
        if let Some(ref rx) = self.rx_msg {
            while let Ok(msg) = rx.try_recv() {
                changed = true;
                match msg {
                    MpvMsg::Metadata(title) => {
                        self.state.song_title = title.clone();
                        self.state.track_info = None;
                        let Some(tx) = self.tx_msg.clone() else {
                            continue;
                        };
                        thread::spawn(move || {
                            if let Ok(Some(info)) = itunes::lookup(&title) {
                                let _ = tx.send(MpvMsg::TrackInfo(info));
                            }
                        });
                    }
                    MpvMsg::TrackInfo(info) => {
                        self.state.track_info = Some(info.clone());
                        if let Some(title) = &info.title {
                            self.state.song_title = title.clone();
                        }
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

    fn status_hints(&self) -> Vec<(String, String)> {
        if self.state.search_mode {
            return vec![
                ("Enter".into(), "done".into()),
                ("⌫".into(), "delete".into()),
            ];
        }
        vec![
            ("↑/↓".into(), "select".into()),
            ("PgUp/PgDn".into(), "page".into()),
            ("/".into(), "search".into()),
            ("enter".into(), "play".into()),
            ("s".into(), "stop".into()),
            ("r".into(), "reload".into()),
        ]
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

fn respond(app: &mut App) {
    if app.dirty || app.cached_json.is_none() {
        let commands_val = match serde_json::to_value(app.render()) {
            Ok(v) => v,
            Err(e) => {
                log::error!("failed to serialize render commands: {e}");
                return;
            }
        };
        let hints = app.status_hints();
        let palette = palette_commands();
        let json = serde_json::json!({
            "commands": commands_val,
            "hints": hints,
            "palette_commands": palette,
        });
        app.cached_json = match serde_json::to_string(&json) {
            Ok(s) => Some(s),
            Err(e) => {
                log::error!("failed to serialize PluginMsg: {e}");
                return;
            }
        };
        app.dirty = false;
    }
    let Some(json) = app.cached_json.as_deref() else {
        return;
    };
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json}");
    let _ = out.flush();
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
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
                        respond(&mut app);
                    }
                    HostMsg::Key { key } => {
                        app.handle_key(key);
                        respond(&mut app);
                    }
                    HostMsg::Tick => {
                        app.handle_tick();
                        respond(&mut app);
                    }
                    HostMsg::Focus | HostMsg::Blur => {
                        respond(&mut app);
                    }
                    HostMsg::ThemeChange { theme } => {
                        app.theme = theme;
                        app.dirty = true;
                        respond(&mut app);
                    }
                    HostMsg::Resize { area } => {
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app);
                    }
                    HostMsg::PaletteCommand { index } => {
                        app.handle_palette_command(index);
                        respond(&mut app);
                    }
                    HostMsg::PluginMessage { .. } => {
                        // Plugin-to-plugin messaging — radio player does not
                        // participate in this yet, but we must handle the variant
                        // to keep the match exhaustive.
                        respond(&mut app);
                    }
                    HostMsg::UserUpdate { user } => {
                        app.user = user;
                        app.dirty = true;
                        respond(&mut app);
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
