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
use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginMsg, RenderCmd, ThemeData, UserData};

enum MpvMsg {
    Metadata(String),
    TrackInfo(itunes::TrackInfo),
    EndFile(u32),
}

enum MpvCmd {
    LoadUrl(String),
    Stop,
    SetVolume(i64),
}

struct App {
    state: state::RadioState,
    theme: ThemeData,
    area: Area,
    tx_cmd: Option<mpsc::Sender<MpvCmd>>,
    rx_msg: Option<mpsc::Receiver<MpvMsg>>,
    tx_msg: Option<mpsc::Sender<MpvMsg>>,
    init_error: Option<String>,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    user: Option<UserData>,
}

fn send_cmd(app: &App, cmd: MpvCmd) {
    if let Some(ref tx) = app.tx_cmd {
        let _ = tx.send(cmd);
    }
}

impl App {
    fn new() -> Self {
        let station_list = stations::load();
        App {
            state: state::RadioState::new(station_list),
            theme: ThemeData {
                text: [220, 220, 220],
                text_muted: [140, 140, 140],
                accent: [157, 124, 216],
                highlight: [250, 178, 131],
                border: [250, 178, 131],
                success: [127, 216, 143],
                error: [224, 108, 117],
                background_panel: [20, 20, 20],
            },
            area: Area { w: 80, h: 24 },
            tx_cmd: None,
            rx_msg: None,
            tx_msg: None,
            init_error: None,
            dirty: true,
            cached_commands: Vec::new(),
            user: None,
        }
    }

    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.dirty = true;

        let (mpv, warns) = match Mpv::new() {
            Ok(v) => v,
            Err(e) => {
                self.init_error = Some(format!("{e}"));
                return;
            }
        };

        for w in &warns {
            eprintln!("  ⚠️  {w}");
        }

        let _ = mpv.observe_property(0, "metadata");
        let _ = mpv.observe_property(1, "media-title");
        let _ = mpv.observe_property(2, "volume");
        let _ = mpv.set_volume(self.state.volume);

        let (tx_msg, rx_msg) = mpsc::channel::<MpvMsg>();
        let (tx_cmd, rx_cmd) = mpsc::channel::<MpvCmd>();

        let tx_msg_mpv = tx_msg.clone();

        thread::spawn(move || {
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
                        let prop: &player::MpvEventProperty = unsafe { &*(ev.data as *const _) };
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
                        let ef: &player::MpvEventEndFile = unsafe { &*(ev.data as *const _) };
                        let _ = tx_msg_mpv.send(MpvMsg::EndFile(ef.reason));
                    }
                }

                while let Ok(cmd) = rx_cmd.try_recv() {
                    match cmd {
                        MpvCmd::LoadUrl(url) => {
                            let _ = mpv.load_url(&url);
                            if let Some(title) = mpv.metadata_title().ok().flatten() {
                                let _ = tx_msg_mpv.send(MpvMsg::Metadata(title));
                            }
                        }
                        MpvCmd::Stop => {
                            let _ = mpv.stop();
                        }
                        MpvCmd::SetVolume(v) => {
                            let _ = mpv.set_volume(v);
                        }
                    }
                }
            }
            mpv.destroy();
        });

        self.tx_cmd = Some(tx_cmd);
        self.rx_msg = Some(rx_msg);
        self.tx_msg = Some(tx_msg);
    }

    fn handle_key(&mut self, key: IpcKey) {
        self.state.scan_msg = None;
        self.dirty = true;
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
                let new_stations = crate::stations::reload();
                let count = new_stations.len();
                self.state.stations = new_stations;
                self.state.filtered = (0..count).collect();
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
                        let tx = self.tx_msg.clone().unwrap();
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
        self.dirty = changed;
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        vec![
            ("↑/↓".into(), "select".into()),
            ("PgUp/PgDn".into(), "page".into()),
            ("enter".into(), "play".into()),
            ("s".into(), "stop".into()),
            ("r".into(), "reload".into()),
            ("+/-".into(), "volume".into()),
        ]
    }

    fn render(&mut self) -> Vec<RenderCmd> {
        if let Some(ref err) = self.init_error {
            return vec![RenderCmd::Text {
                x: 0,
                y: 0,
                text: format!("Failed to load libmpv: {err}"),
                fg: Some(self.theme.error),
                bg: None,
                bold: false,
            }];
        }
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands =
                ui::render_ui(&self.state, &self.theme, self.area.w, self.area.h);
            self.dirty = false;
        }
        self.cached_commands.clone()
    }
}

fn respond(app: &mut App) {
    let msg = PluginMsg {
        commands: app.render(),
        hints: app.status_hints(),
        request: None,
    };
    let json = serde_json::to_string(&msg).expect("PluginMsg serialization");
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json}");
    let _ = out.flush();
}

fn main() {
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
                        eprintln!("[radio] parse error: {e}: {line}");
                        continue;
                    }
                };

                match msg {
                    HostMsg::Init { theme, area } => {
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
                    HostMsg::UserUpdate { user } => {
                        app.user = user;
                        app.dirty = true;
                        respond(&mut app);
                    }
                    HostMsg::Shutdown => break,
                }
            }
        }
    }
}
