mod itunes;
pub mod player;
mod state;
pub mod stations;
mod ui;

use crate::player::Mpv;
use crate::state::{PlayState, RadioState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use santui_core::{Plugin, PluginContext, Theme};
use std::sync::mpsc;
use std::thread;

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

pub struct RadioPlugin {
    state: RadioState,
    theme: Theme,
    tx_cmd: Option<mpsc::Sender<MpvCmd>>,
    rx_msg: Option<mpsc::Receiver<MpvMsg>>,
    tx_msg: Option<mpsc::Sender<MpvMsg>>,
    init_error: Option<String>,
}

impl Default for RadioPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl RadioPlugin {
    pub fn new() -> Self {
        let station_list = stations::load();
        RadioPlugin {
            state: RadioState::new(station_list),
            theme: Theme::default(),
            tx_cmd: None,
            rx_msg: None,
            tx_msg: None,
            init_error: None,
        }
    }

    fn send_cmd(&self, cmd: MpvCmd) {
        if let Some(ref tx) = self.tx_cmd {
            let _ = tx.send(cmd);
        }
    }
}

impl Plugin for RadioPlugin {
    fn id(&self) -> &'static str {
        "santui-radio-streaming-player"
    }

    fn name(&self) -> &str {
        "Radio Streaming Player"
    }

    fn init(&mut self, ctx: &mut PluginContext) -> Result<(), Box<dyn std::error::Error>> {
        self.theme = ctx.theme.clone();
        let (mpv, warns) = match Mpv::new() {
            Ok(v) => v,
            Err(e) => {
                self.init_error = Some(format!("{e}"));
                return Ok(());
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

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('?') => {
                self.state.show_help = !self.state.show_help;
                true
            }
            KeyCode::Char('/') => {
                self.state.filter.clear();
                self.state.apply_filter();
                true
            }
            KeyCode::Up => {
                self.state.select_prev();
                true
            }
            KeyCode::Down => {
                self.state.select_next();
                true
            }
            KeyCode::Enter => {
                if let Some(station) = self.state.selected_station().cloned() {
                    let idx = self.state.current_filtered_index();
                    self.state.current_station = Some(idx);
                    self.state.play_state = PlayState::Playing(station.name.to_string());
                    self.state.song_title.clear();
                    self.state.track_info = None;
                    self.state.start_time = std::time::Instant::now();
                    self.send_cmd(MpvCmd::Stop);
                    self.send_cmd(MpvCmd::LoadUrl(station.url));
                }
                true
            }
            KeyCode::Char('s') => {
                self.send_cmd(MpvCmd::Stop);
                self.state.play_state = PlayState::Stopped;
                self.state.current_station = None;
                self.state.song_title.clear();
                self.state.track_info = None;
                true
            }
            KeyCode::Char('=') | KeyCode::Char('+') => {
                self.state.volume_up();
                self.send_cmd(MpvCmd::SetVolume(self.state.volume));
                true
            }
            KeyCode::Char('-') => {
                self.state.volume_down();
                self.send_cmd(MpvCmd::SetVolume(self.state.volume));
                true
            }
            _ => false,
        }
    }

    fn on_theme_change(&mut self, theme: &santui_core::Theme) {
        self.theme = theme.clone();
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        if let Some(ref err) = self.init_error {
            let text = format!("Failed to load libmpv: {err}");
            let p = ratatui::widgets::Paragraph::new(text)
                .style(ratatui::style::Style::default().fg(self.theme.error));
            f.render_widget(p, area);
            return;
        }
        ui::draw_radio(f, area, &self.state, &self.theme);
    }

    fn tick(&mut self) {
        if let Some(ref rx) = self.rx_msg {
            while let Ok(msg) = rx.try_recv() {
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
                                self.send_cmd(MpvCmd::LoadUrl(station.url));
                            }
                        } else if reason == player::MPV_END_FILE_REASON_ERROR {
                            self.state.play_state = PlayState::Error("connection lost".into());
                        }
                    }
                }
            }
        }
    }

    fn status_hints(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("↑/↓", "Select"),
            ("Enter", "Play"),
            ("s", "Stop"),
            ("+/-", "Volume"),
            ("/", "Filter"),
            ("?", "Help"),
        ]
    }
}
