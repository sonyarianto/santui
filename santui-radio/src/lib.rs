mod player;
mod state;
pub mod stations;
mod ui;

use crate::player::Mpv;
use crate::state::{PlayState, RadioState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use santui_core::{Plugin, PluginContext};
use std::sync::mpsc;
use std::thread;

enum MpvMsg {
    Metadata(String),
    EndFile(u32),
}

enum MpvCmd {
    LoadUrl(&'static str),
    Stop,
    SetVolume(i64),
}

pub struct RadioPlugin {
    state: RadioState,
    tx_cmd: Option<mpsc::Sender<MpvCmd>>,
    rx_msg: Option<mpsc::Receiver<MpvMsg>>,
    init_error: Option<String>,
}

impl Default for RadioPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl RadioPlugin {
    pub fn new() -> Self {
        let station_list: Vec<&'static stations::Station> = stations::STATIONS.iter().collect();
        RadioPlugin {
            state: RadioState::new(station_list),
            tx_cmd: None,
            rx_msg: None,
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
        "santui-radio"
    }

    fn name(&self) -> &str {
        "Radio Player"
    }

    fn init(&mut self, _ctx: &mut PluginContext) -> Result<(), Box<dyn std::error::Error>> {
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
        let _ = mpv.observe_property(1, "volume");
        let _ = mpv.set_volume(self.state.volume);

        let (tx_msg, rx_msg) = mpsc::channel::<MpvMsg>();
        let (tx_cmd, rx_cmd) = mpsc::channel::<MpvCmd>();

        thread::spawn(move || {
            loop {
                let ev = mpv.wait_event_raw(0.1);
                if let Some(ev) = ev {
                    let id = ev.event_id;
                    if id == player::MPV_EVENT_SHUTDOWN {
                        break;
                    }
                    if id == player::MPV_EVENT_PROPERTY_CHANGE {
                        let prop: &player::MpvEventProperty = unsafe { &*(ev.data as *const _) };
                        let name = unsafe {
                            std::ffi::CStr::from_ptr(prop.name)
                                .to_string_lossy()
                                .to_string()
                        };
                        if name == "metadata" {
                            if let Some(title) = mpv.metadata_title().ok().flatten() {
                                let _ = tx_msg.send(MpvMsg::Metadata(title));
                            }
                        }
                    }
                    if id == player::MPV_EVENT_END_FILE {
                        let ef: &player::MpvEventEndFile = unsafe { &*(ev.data as *const _) };
                        let _ = tx_msg.send(MpvMsg::EndFile(ef.reason));
                    }
                }

                while let Ok(cmd) = rx_cmd.try_recv() {
                    match cmd {
                        MpvCmd::LoadUrl(url) => {
                            let _ = mpv.load_url(url);
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

    fn render(&self, f: &mut Frame, area: Rect) {
        if let Some(ref err) = self.init_error {
            let text = format!("Failed to load libmpv: {err}");
            let p = ratatui::widgets::Paragraph::new(text)
                .style(ratatui::style::Style::default().fg(ratatui::style::Color::Red));
            f.render_widget(p, area);
            return;
        }
        ui::draw_radio(f, area, &self.state);
    }

    fn tick(&mut self) {
        if let Some(ref rx) = self.rx_msg {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    MpvMsg::Metadata(title) => {
                        self.state.song_title = title;
                    }
                    MpvMsg::EndFile(reason) => {
                        if reason == player::MPV_END_FILE_REASON_EOF {
                            if let Some(idx) = self.state.current_station {
                                let station = self.state.stations[idx];
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
}
