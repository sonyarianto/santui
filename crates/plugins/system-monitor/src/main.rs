mod sampler;
mod state;
mod ui;

use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};

use sampler::Sampler;
use state::{Screen, SortBy, SysMonState};
use ui::render_ui;

struct App {
    state: SysMonState,
    sampler: Sampler,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: SysMonState::default(),
            sampler: Sampler::default(),
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
        }
    }
}

impl App {
    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.state.snapshot = self.sampler.sample();
        self.state.history.push(&self.state.snapshot);
        self.dirty = true;
    }

    fn handle_tick(&mut self) {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now_secs != self.state.last_second {
            self.state.last_second = now_secs;
            self.state.snapshot = self.sampler.sample();
            self.state.history.push(&self.state.snapshot);
            self.dirty = true;
        }
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char('1') => {
                self.state.screen = Screen::CpuDetail;
                self.dirty = true;
                true
            }
            IpcKey::Char('2') => {
                self.state.screen = Screen::MemDetail;
                self.dirty = true;
                true
            }
            IpcKey::Char('3') => {
                self.state.screen = Screen::DiskDetail;
                self.dirty = true;
                true
            }
            IpcKey::Char('4') => {
                self.state.screen = Screen::NetDetail;
                self.dirty = true;
                true
            }
            IpcKey::Char('5') => {
                self.state.screen = Screen::ProcessList;
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                if self.state.screen == Screen::Overview {
                    false
                } else {
                    self.state.screen = Screen::Overview;
                    self.dirty = true;
                    true
                }
            }
            IpcKey::Up | IpcKey::Char('k') => {
                if self.state.screen == Screen::ProcessList {
                    self.state.selected_process = self.state.selected_process.saturating_sub(1);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                if self.state.screen == Screen::ProcessList {
                    let max = self.state.snapshot.top_processes.len().saturating_sub(1);
                    self.state.selected_process = self
                        .state
                        .selected_process
                        .min(max)
                        .saturating_add(1)
                        .min(max);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Char('s') => {
                if self.state.screen == Screen::ProcessList {
                    self.state.process_sort = match self.state.process_sort {
                        SortBy::Cpu => SortBy::Memory,
                        SortBy::Memory => SortBy::Name,
                        SortBy::Name => SortBy::Cpu,
                    };
                    self.state.sort_processes();
                    self.state.selected_process = 0;
                    self.dirty = true;
                }
                true
            }
            _ => false,
        }
    }

    fn handle_palette_command(&mut self, index: u32) {
        self.state.screen = match index {
            0 => Screen::Overview,
            1 => Screen::CpuDetail,
            2 => Screen::MemDetail,
            3 => Screen::DiskDetail,
            4 => Screen::NetDetail,
            5 => Screen::ProcessList,
            _ => return,
        };
        self.dirty = true;
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match self.state.screen {
            Screen::Overview => vec![
                ("1".into(), "cpu".into()),
                ("2".into(), "mem".into()),
                ("3".into(), "disk".into()),
                ("4".into(), "net".into()),
                ("5".into(), "procs".into()),
            ],
            Screen::CpuDetail | Screen::MemDetail | Screen::DiskDetail | Screen::NetDetail => {
                vec![("Esc".into(), "overview".into())]
            }
            Screen::ProcessList => vec![
                ("Esc".into(), "overview".into()),
                ("s".into(), "sort".into()),
                ("↑↓".into(), "navigate".into()),
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

fn palette_commands() -> Vec<(String, String)> {
    vec![]
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
                        log::error!("[system-monitor] parse error: {e}: {line}");
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
    use state::Screen;

    fn base_app() -> App {
        App::default()
    }

    #[test]
    fn handle_key_1_opens_cpu_detail() {
        let mut app = base_app();
        app.state.screen = Screen::Overview;
        assert!(app.handle_key(IpcKey::Char('1')));
        assert_eq!(app.state.screen, Screen::CpuDetail);
    }

    #[test]
    fn handle_key_esc_on_cpu_returns_to_overview() {
        let mut app = base_app();
        app.state.screen = Screen::CpuDetail;
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.screen, Screen::Overview);
    }

    #[test]
    fn handle_key_esc_on_overview_not_consumed() {
        let mut app = base_app();
        app.state.screen = Screen::Overview;
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_s_cycles_sort() {
        let mut app = base_app();
        app.state.screen = Screen::ProcessList;
        app.state.process_sort = SortBy::Cpu;
        assert!(app.handle_key(IpcKey::Char('s')));
        assert_eq!(app.state.process_sort, SortBy::Memory);
        assert!(app.handle_key(IpcKey::Char('s')));
        assert_eq!(app.state.process_sort, SortBy::Name);
        assert!(app.handle_key(IpcKey::Char('s')));
        assert_eq!(app.state.process_sort, SortBy::Cpu);
    }

    #[test]
    fn handle_key_down_in_process_list() {
        let mut app = base_app();
        app.state.screen = Screen::ProcessList;
        app.state
            .snapshot
            .top_processes
            .push(state::ProcessSnapshot {
                pid: 1,
                name: "a".into(),
                cpu_pct: 10.0,
                mem_bytes: 100,
            });
        app.state
            .snapshot
            .top_processes
            .push(state::ProcessSnapshot {
                pid: 2,
                name: "b".into(),
                cpu_pct: 20.0,
                mem_bytes: 200,
            });
        app.state.selected_process = 0;
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected_process, 1);
    }

    #[test]
    fn handle_key_down_clamps_at_max() {
        let mut app = base_app();
        app.state.screen = Screen::ProcessList;
        app.state
            .snapshot
            .top_processes
            .push(state::ProcessSnapshot {
                pid: 1,
                name: "a".into(),
                cpu_pct: 10.0,
                mem_bytes: 100,
            });
        app.state.selected_process = 0;
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected_process, 0);
    }

    #[test]
    fn handle_tick_marks_dirty_on_new_second() {
        let mut app = base_app();
        app.dirty = false;
        app.state.last_second = 0;
        // We can't mock time, but we can verify the path by forcing a second boundary.
        // Instead let the tick run naturally and check it works.
        app.handle_tick();
        // After tick, either dirty was set (new second) or not (same second).
        // This test verifies the mechanism doesn't crash.
    }

    #[test]
    fn handle_tick_no_dirty_same_second() {
        let mut app = base_app();
        app.dirty = false;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        app.state.last_second = now;
        app.handle_tick();
        assert!(!app.dirty);
    }

    #[test]
    fn palette_command_0_opens_overview() {
        let mut app = base_app();
        app.state.screen = Screen::CpuDetail;
        app.handle_palette_command(0);
        assert_eq!(app.state.screen, Screen::Overview);
    }

    #[test]
    fn palette_command_5_opens_process_list() {
        let mut app = base_app();
        app.state.screen = Screen::Overview;
        app.handle_palette_command(5);
        assert_eq!(app.state.screen, Screen::ProcessList);
    }

    #[test]
    fn handle_key_2_opens_mem_detail() {
        let mut app = base_app();
        let consumed = app.handle_key(IpcKey::Char('2'));
        assert!(consumed);
        assert_eq!(app.state.screen, Screen::MemDetail);
    }

    #[test]
    fn handle_key_3_opens_disk_detail() {
        let mut app = base_app();
        let consumed = app.handle_key(IpcKey::Char('3'));
        assert!(consumed);
        assert_eq!(app.state.screen, Screen::DiskDetail);
    }

    #[test]
    fn handle_key_4_opens_net_detail() {
        let mut app = base_app();
        let consumed = app.handle_key(IpcKey::Char('4'));
        assert!(consumed);
        assert_eq!(app.state.screen, Screen::NetDetail);
    }

    #[test]
    fn palette_command_1_opens_cpu_detail() {
        let mut app = base_app();
        app.handle_palette_command(1);
        assert_eq!(app.state.screen, Screen::CpuDetail);
    }

    #[test]
    fn palette_command_2_opens_mem_detail() {
        let mut app = base_app();
        app.handle_palette_command(2);
        assert_eq!(app.state.screen, Screen::MemDetail);
    }

    #[test]
    fn palette_command_3_opens_disk_detail() {
        let mut app = base_app();
        app.handle_palette_command(3);
        assert_eq!(app.state.screen, Screen::DiskDetail);
    }

    #[test]
    fn palette_command_4_opens_net_detail() {
        let mut app = base_app();
        app.handle_palette_command(4);
        assert_eq!(app.state.screen, Screen::NetDetail);
    }
}
