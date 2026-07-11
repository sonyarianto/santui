mod state;
mod ui;

use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginMessage, PluginRequest, RenderCmd, ThemeData,
};
use state::{Phase, PomodoroData, PomodoroState, TimerState};
use ui::render_ui;

struct App {
    state: PomodoroState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    pending_plugin_msg: Option<PluginMessage>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: PomodoroState::default(),
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
            pending_request: Some(PluginRequest::DbGet {
                key: "pomodoro".into(),
            }),
            pending_plugin_msg: None,
        }
    }
}

impl App {
    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.dirty = true;
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        if self.state.show_settings {
            return self.handle_settings_key(key);
        }
        self.handle_main_key(key)
    }

    fn handle_main_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char(' ') => {
                let was_work_phase = self.state.phase == Phase::Work;
                let was_running = self.state.timer_state == TimerState::Running;
                let was_idle_or_paused = matches!(
                    self.state.timer_state,
                    TimerState::Idle | TimerState::Paused
                );
                self.state.toggle_pause();
                self.dirty = true;
                if was_work_phase && was_idle_or_paused {
                    self.pending_plugin_msg = Some(PluginMessage {
                        to: "radio-stream-player".into(),
                        action: "pause".into(),
                        data: serde_json::Value::Null,
                    });
                } else if was_work_phase && was_running {
                    self.pending_plugin_msg = Some(PluginMessage {
                        to: "radio-stream-player".into(),
                        action: "resume".into(),
                        data: serde_json::Value::Null,
                    });
                }
                true
            }
            IpcKey::Char('s') => {
                let was_work = self.state.phase == Phase::Work;
                self.state.skip();
                self.dirty = true;
                if was_work {
                    self.pending_plugin_msg = Some(PluginMessage {
                        to: "radio-stream-player".into(),
                        action: "resume".into(),
                        data: serde_json::Value::Null,
                    });
                }
                self.schedule_db_save();
                true
            }
            IpcKey::Char('r') => {
                self.state.reset_session();
                self.dirty = true;
                true
            }
            IpcKey::Char(',') => {
                self.state.show_settings = true;
                self.dirty = true;
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_settings_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.settings_cursor = self.state.settings_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.state.settings_cursor =
                    self.state.settings_cursor.min(5).saturating_add(1).min(5);
                self.dirty = true;
                true
            }
            IpcKey::Left => {
                self.adjust_setting(-1);
                self.dirty = true;
                true
            }
            IpcKey::Right => {
                self.adjust_setting(1);
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.show_settings = false;
                self.schedule_db_save();
                self.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn adjust_setting(&mut self, delta: i64) {
        match self.state.settings_cursor {
            0 => {
                let new_val = (self.state.data.config.work_secs as i64 + delta * 60).max(60);
                self.state.data.config.work_secs = new_val as u64;
            }
            1 => {
                let new_val = (self.state.data.config.short_break_secs as i64 + delta * 60).max(60);
                self.state.data.config.short_break_secs = new_val as u64;
            }
            2 => {
                let new_val = (self.state.data.config.long_break_secs as i64 + delta * 60).max(60);
                self.state.data.config.long_break_secs = new_val as u64;
            }
            3 => {
                let new_val = (self.state.data.config.long_break_after as i64 + delta).max(1);
                self.state.data.config.long_break_after = new_val as u32;
            }
            4 => {
                self.state.data.config.auto_start_breaks = delta > 0;
            }
            5 => {
                self.state.data.config.auto_start_work = delta > 0;
            }
            _ => {}
        }
        self.schedule_db_save();
    }

    fn handle_tick(&mut self) {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now_secs == self.state.last_second {
            return;
        }
        self.state.last_second = now_secs;

        let just_finished = self.state.tick_second();
        self.dirty = true;

        if just_finished {
            if self.state.phase == Phase::Work {
                self.pending_plugin_msg = Some(PluginMessage {
                    to: "radio-stream-player".into(),
                    action: "resume".into(),
                    data: serde_json::Value::Null,
                });
            }
            let should_auto_start = (self.state.phase == Phase::Work
                && self.state.data.config.auto_start_breaks)
                || (self.state.phase != Phase::Work && self.state.data.config.auto_start_work);
            if should_auto_start {
                self.state.advance_phase();
                self.state.start();
            }
            self.schedule_db_save();
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "pomodoro" {
            let mut data: PomodoroData = value
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default();
            let today = today_date_string();
            if data.stats.date != today {
                data.stats = state::DailyStats {
                    date: today,
                    ..Default::default()
                };
            }
            self.state.data = data;
            self.dirty = true;
        }
    }

    fn schedule_db_save(&mut self) {
        let value = serde_json::to_string(&self.state.data).unwrap();
        self.pending_request = Some(PluginRequest::DbSet {
            key: "pomodoro".into(),
            value,
        });
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.state.phase = Phase::Work;
                self.state.timer_state = TimerState::Idle;
                self.state.remaining_secs = self.state.data.config.work_secs;
                self.state.sessions_done = 0;
                self.dirty = true;
            }
            1 => {
                self.state.phase = Phase::ShortBreak;
                self.state.timer_state = TimerState::Idle;
                self.state.remaining_secs = self.state.data.config.short_break_secs;
                self.dirty = true;
            }
            2 => {
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        if self.state.show_settings {
            vec![
                ("↑↓".into(), "navigate".into()),
                ("←→".into(), "adjust".into()),
                ("esc".into(), "close".into()),
            ]
        } else {
            match &self.state.timer_state {
                TimerState::Idle | TimerState::Paused => vec![
                    ("space".into(), "start".into()),
                    ("s".into(), "skip".into()),
                    ("r".into(), "reset".into()),
                    (",".into(), "settings".into()),
                ],
                TimerState::Running => vec![
                    ("space".into(), "pause".into()),
                    ("s".into(), "skip".into()),
                    ("r".into(), "reset".into()),
                    (",".into(), "settings".into()),
                ],
                TimerState::Finished => {
                    vec![("space".into(), "next".into()), ("s".into(), "skip".into())]
                }
            }
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
    vec![
        ("Pomodoro".into(), "Start focus session".into()),
        ("Pomodoro".into(), "Start break".into()),
        ("Pomodoro".into(), "View stats".into()),
    ]
}

fn today_date_string() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs / 86400;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y_adj = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y_adj, m, d)
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
    let plugin_message = app.pending_plugin_msg.take();
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": hints,
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
                        log::error!("[pomodoro] parse error: {e}: {line}");
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

    fn base_app() -> App {
        App::default()
    }

    #[test]
    fn handle_key_space_starts_timer() {
        let mut app = base_app();
        assert_eq!(app.state.timer_state, TimerState::Idle);
        assert!(app.handle_key(IpcKey::Char(' ')));
        assert_eq!(app.state.timer_state, TimerState::Running);
    }

    #[test]
    fn handle_key_space_pauses_running_timer() {
        let mut app = base_app();
        app.state.start();
        assert_eq!(app.state.timer_state, TimerState::Running);
        assert!(app.handle_key(IpcKey::Char(' ')));
        assert_eq!(app.state.timer_state, TimerState::Paused);
    }

    #[test]
    fn handle_key_space_resumes_paused_timer() {
        let mut app = base_app();
        app.state.start();
        app.state.pause();
        assert_eq!(app.state.timer_state, TimerState::Paused);
        assert!(app.handle_key(IpcKey::Char(' ')));
        assert_eq!(app.state.timer_state, TimerState::Running);
    }

    #[test]
    fn handle_key_space_on_finished_advances_phase() {
        let mut app = base_app();
        app.state.timer_state = TimerState::Finished;
        app.state.phase = Phase::Work;
        assert!(app.handle_key(IpcKey::Char(' ')));
        assert_eq!(app.state.phase, Phase::ShortBreak);
    }

    #[test]
    fn handle_key_space_sets_radio_pause_on_work_start() {
        let mut app = base_app();
        app.state.phase = Phase::Work;
        app.state.timer_state = TimerState::Idle;
        app.handle_key(IpcKey::Char(' '));
        let msg = app.pending_plugin_msg.as_ref().unwrap();
        assert_eq!(msg.to, "radio-stream-player");
        assert_eq!(msg.action, "pause");
    }

    #[test]
    fn handle_key_space_sets_radio_resume_on_work_pause() {
        let mut app = base_app();
        app.state.phase = Phase::Work;
        app.state.start();
        app.handle_key(IpcKey::Char(' '));
        let msg = app.pending_plugin_msg.as_ref().unwrap();
        assert_eq!(msg.to, "radio-stream-player");
        assert_eq!(msg.action, "resume");
    }

    #[test]
    fn handle_key_s_skips_session() {
        let mut app = base_app();
        app.state.phase = Phase::Work;
        let initial_sessions = app.state.sessions_done;
        assert!(app.handle_key(IpcKey::Char('s')));
        assert_ne!(app.state.phase, Phase::Work);
        assert_eq!(app.state.sessions_done, initial_sessions + 1);
    }

    #[test]
    fn handle_key_s_sends_resume_on_work_skip() {
        let mut app = base_app();
        app.state.phase = Phase::Work;
        app.handle_key(IpcKey::Char('s'));
        let msg = app.pending_plugin_msg.as_ref().unwrap();
        assert_eq!(msg.to, "radio-stream-player");
        assert_eq!(msg.action, "resume");
    }

    #[test]
    fn handle_key_r_resets_session() {
        let mut app = base_app();
        app.state.start();
        app.state.remaining_secs = 100;
        assert!(app.handle_key(IpcKey::Char('r')));
        assert_eq!(app.state.remaining_secs, app.state.phase_duration());
        assert_eq!(app.state.timer_state, TimerState::Idle);
    }

    #[test]
    fn handle_key_comma_opens_settings() {
        let mut app = base_app();
        assert!(!app.state.show_settings);
        assert!(app.handle_key(IpcKey::Char(',')));
        assert!(app.state.show_settings);
    }

    #[test]
    fn handle_key_esc_closes_settings() {
        let mut app = base_app();
        app.state.show_settings = true;
        assert!(app.handle_key(IpcKey::Esc));
        assert!(!app.state.show_settings);
    }

    #[test]
    fn handle_key_esc_on_main_not_consumed() {
        let mut app = base_app();
        app.state.show_settings = false;
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn settings_up_arrow_moves_cursor() {
        let mut app = base_app();
        app.state.show_settings = true;
        app.state.settings_cursor = 3;
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.settings_cursor, 2);
    }

    #[test]
    fn settings_down_arrow_moves_cursor() {
        let mut app = base_app();
        app.state.show_settings = true;
        app.state.settings_cursor = 2;
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.settings_cursor, 3);
    }

    #[test]
    fn settings_cursor_doesnt_go_below_zero() {
        let mut app = base_app();
        app.state.show_settings = true;
        app.state.settings_cursor = 0;
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.settings_cursor, 0);
    }

    #[test]
    fn settings_cursor_doesnt_exceed_max() {
        let mut app = base_app();
        app.state.show_settings = true;
        app.state.settings_cursor = 5;
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.settings_cursor, 5);
    }

    #[test]
    fn handle_tick_decrements_on_new_second() {
        let mut app = base_app();
        app.state.timer_state = TimerState::Running;
        app.state.last_second = 0;
        let before = app.state.remaining_secs;
        app.handle_tick();
        assert_eq!(app.state.remaining_secs, before - 1);
        assert!(app.dirty);
    }

    #[test]
    fn handle_tick_no_change_same_second() {
        let mut app = base_app();
        app.state.timer_state = TimerState::Running;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        app.state.last_second = now;
        let before = app.state.remaining_secs;
        app.handle_tick();
        assert_eq!(app.state.remaining_secs, before);
    }

    #[test]
    fn handle_tick_sets_radio_resume_on_work_finish() {
        let mut app = base_app();
        app.state.phase = Phase::Work;
        app.state.timer_state = TimerState::Running;
        app.state.remaining_secs = 1;
        app.state.last_second = 0;
        app.handle_tick();
        let msg = app.pending_plugin_msg.as_ref().unwrap();
        assert_eq!(msg.to, "radio-stream-player");
        assert_eq!(msg.action, "resume");
    }

    #[test]
    fn handle_tick_auto_advances_when_configured() {
        let mut app = base_app();
        app.state.data.config.auto_start_breaks = true;
        app.state.phase = Phase::Work;
        app.state.timer_state = TimerState::Running;
        app.state.remaining_secs = 1;
        app.state.last_second = 0;
        app.handle_tick();
        assert_eq!(app.state.phase, Phase::ShortBreak);
        assert_eq!(app.state.timer_state, TimerState::Running);
    }

    #[test]
    fn handle_db_value_loads_data() {
        let mut app = base_app();
        let today = today_date_string();
        let json = serde_json::json!({
            "config": {
                "work_secs": 1200,
                "short_break_secs": 180,
                "long_break_secs": 600,
                "long_break_after": 3,
                "auto_start_breaks": true,
                "auto_start_work": false
            },
            "stats": {
                "date": today,
                "sessions_completed": 10,
                "total_focus_secs": 12000,
                "total_break_secs": 3000
            }
        });
        app.handle_db_value("pomodoro", Some(json.to_string()));
        assert_eq!(app.state.data.config.work_secs, 1200);
        assert_eq!(app.state.data.config.long_break_after, 3);
        assert!(app.state.data.config.auto_start_breaks);
        assert_eq!(app.state.data.stats.sessions_completed, 10);
        assert!(app.dirty);
    }

    #[test]
    fn handle_db_value_none_uses_defaults() {
        let mut app = base_app();
        app.state.data.config.work_secs = 9999;
        app.handle_db_value("pomodoro", None);
        assert_eq!(app.state.data.config.work_secs, 25 * 60);
    }

    #[test]
    fn handle_db_value_resets_stats_on_new_day() {
        let mut app = base_app();
        let json = serde_json::json!({
            "config": state::PomodoroConfig::default(),
            "stats": {
                "date": "2020-01-01",
                "sessions_completed": 100,
                "total_focus_secs": 999,
                "total_break_secs": 888
            }
        });
        app.handle_db_value("pomodoro", Some(json.to_string()));
        assert_eq!(app.state.data.stats.sessions_completed, 0);
        assert_eq!(app.state.data.stats.total_focus_secs, 0);
        assert_eq!(app.state.data.stats.total_break_secs, 0);
        assert_ne!(app.state.data.stats.date, "2020-01-01");
    }

    #[test]
    fn handle_db_value_ignores_other_keys() {
        let mut app = base_app();
        let original_work_secs = app.state.data.config.work_secs;
        app.handle_db_value("other_key", Some(r#"{"work_secs": 999}"#.into()));
        assert_eq!(app.state.data.config.work_secs, original_work_secs);
    }

    #[test]
    fn unhandled_keys_return_false_on_main() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Left));
        assert!(!app.handle_key(IpcKey::Right));
        assert!(!app.handle_key(IpcKey::F(1)));
        assert!(!app.handle_key(IpcKey::Enter));
    }

    #[test]
    fn palette_command_0_starts_work_session() {
        let mut app = base_app();
        app.state.phase = Phase::ShortBreak;
        app.handle_palette_command(0);
        assert_eq!(app.state.phase, Phase::Work);
        assert_eq!(app.state.timer_state, TimerState::Idle);
        assert!(app.dirty);
    }

    #[test]
    fn palette_command_1_starts_break() {
        let mut app = base_app();
        app.state.phase = Phase::Work;
        app.handle_palette_command(1);
        assert_eq!(app.state.phase, Phase::ShortBreak);
        assert_eq!(app.state.timer_state, TimerState::Idle);
        assert!(app.dirty);
    }

    #[test]
    fn palette_command_2_just_sets_dirty() {
        let mut app = base_app();
        app.dirty = false;
        app.handle_palette_command(2);
        assert!(app.dirty);
    }

    #[test]
    fn respond_includes_plugin_message() {
        let mut app = base_app();
        app.pending_plugin_msg = Some(PluginMessage {
            to: "radio-stream-player".into(),
            action: "pause".into(),
            data: serde_json::Value::Null,
        });
        let hints = app.status_hints();
        let palette = palette_commands();
        let request = app.pending_request.take();
        let plugin_message = app.pending_plugin_msg.take();
        let json = serde_json::json!({
            "commands": serde_json::to_value(app.render()).unwrap(),
            "hints": hints,
            "palette_commands": palette,
            "request": request,
            "plugin_message": plugin_message,
            "consumed": false,
        });
        let json_str = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let pm = parsed["plugin_message"].as_object().unwrap();
        assert_eq!(pm["to"], "radio-stream-player");
        assert_eq!(pm["action"], "pause");
    }

    #[test]
    fn today_date_string_returns_valid_format() {
        let date = today_date_string();
        assert_eq!(date.len(), 10);
        assert_eq!(date.chars().nth(4), Some('-'));
        assert_eq!(date.chars().nth(7), Some('-'));
    }

    #[test]
    fn settings_left_decreases_work_duration() {
        let mut app = base_app();
        app.state.show_settings = true;
        app.state.settings_cursor = 0;
        let before = app.state.data.config.work_secs;
        app.handle_key(IpcKey::Left);
        assert!(app.state.data.config.work_secs < before);
    }

    #[test]
    fn settings_right_increases_work_duration() {
        let mut app = base_app();
        app.state.show_settings = true;
        app.state.settings_cursor = 0;
        let before = app.state.data.config.work_secs;
        app.handle_key(IpcKey::Right);
        assert!(app.state.data.config.work_secs > before);
    }

    #[test]
    fn settings_left_toggles_boolean() {
        let mut app = base_app();
        app.state.show_settings = true;
        app.state.settings_cursor = 4;
        app.state.data.config.auto_start_breaks = true;
        app.handle_key(IpcKey::Left);
        assert!(!app.state.data.config.auto_start_breaks);
    }

    #[test]
    fn settings_right_toggles_boolean() {
        let mut app = base_app();
        app.state.show_settings = true;
        app.state.settings_cursor = 5;
        app.state.data.config.auto_start_work = false;
        app.handle_key(IpcKey::Right);
        assert!(app.state.data.config.auto_start_work);
    }

    #[test]
    fn app_default_has_pending_db_get() {
        let app = App::default();
        match app.pending_request {
            Some(PluginRequest::DbGet { ref key }) => assert_eq!(key, "pomodoro"),
            other => panic!("expected DbGet, got {other:?}"),
        }
    }
}
