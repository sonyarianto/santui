use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Phase {
    Work,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn label(&self) -> &str {
        match self {
            Phase::Work => "FOCUS",
            Phase::ShortBreak => "SHORT BREAK",
            Phase::LongBreak => "LONG BREAK",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimerState {
    Idle,
    Running,
    Paused,
    Finished,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailyStats {
    pub date: String,
    pub sessions_completed: u32,
    pub total_focus_secs: u64,
    pub total_break_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PomodoroConfig {
    pub work_secs: u64,
    pub short_break_secs: u64,
    pub long_break_secs: u64,
    pub long_break_after: u32,
    pub auto_start_breaks: bool,
    pub auto_start_work: bool,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            work_secs: 25 * 60,
            short_break_secs: 5 * 60,
            long_break_secs: 15 * 60,
            long_break_after: 4,
            auto_start_breaks: false,
            auto_start_work: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PomodoroData {
    pub config: PomodoroConfig,
    pub stats: DailyStats,
}

pub struct PomodoroState {
    pub phase: Phase,
    pub timer_state: TimerState,
    pub remaining_secs: u64,
    pub sessions_done: u32,
    pub data: PomodoroData,
    pub last_second: u64,
    pub show_settings: bool,
    pub settings_cursor: usize,
}

impl Default for PomodoroState {
    fn default() -> Self {
        Self {
            phase: Phase::Work,
            timer_state: TimerState::Idle,
            remaining_secs: 25 * 60,
            data: PomodoroData::default(),
            sessions_done: 0,
            last_second: 0,
            show_settings: false,
            settings_cursor: 0,
        }
    }
}

impl PomodoroState {
    pub fn phase_duration(&self) -> u64 {
        match self.phase {
            Phase::Work => self.data.config.work_secs,
            Phase::ShortBreak => self.data.config.short_break_secs,
            Phase::LongBreak => self.data.config.long_break_secs,
        }
    }

    pub fn start(&mut self) {
        if self.timer_state == TimerState::Idle || self.timer_state == TimerState::Finished {
            self.remaining_secs = self.phase_duration();
        }
        self.timer_state = TimerState::Running;
    }

    pub fn pause(&mut self) {
        if self.timer_state == TimerState::Running {
            self.timer_state = TimerState::Paused;
        }
    }

    pub fn toggle_pause(&mut self) {
        match self.timer_state {
            TimerState::Running => self.pause(),
            TimerState::Paused | TimerState::Idle => self.start(),
            TimerState::Finished => {
                self.advance_phase();
            }
        }
    }

    pub fn advance_phase(&mut self) {
        match self.phase {
            Phase::Work => {
                self.sessions_done += 1;
                self.data.stats.sessions_completed += 1;
                if self
                    .sessions_done
                    .is_multiple_of(self.data.config.long_break_after)
                {
                    self.phase = Phase::LongBreak;
                } else {
                    self.phase = Phase::ShortBreak;
                }
            }
            Phase::ShortBreak | Phase::LongBreak => {
                self.phase = Phase::Work;
            }
        }
        self.remaining_secs = self.phase_duration();
        self.timer_state = TimerState::Idle;
    }

    pub fn skip(&mut self) {
        self.advance_phase();
    }

    pub fn reset_session(&mut self) {
        self.remaining_secs = self.phase_duration();
        self.timer_state = TimerState::Idle;
    }

    pub fn tick_second(&mut self) -> bool {
        if self.timer_state != TimerState::Running {
            return false;
        }
        if self.remaining_secs > 0 {
            self.remaining_secs -= 1;
            match self.phase {
                Phase::Work => self.data.stats.total_focus_secs += 1,
                _ => self.data.stats.total_break_secs += 1,
            }
        }
        if self.remaining_secs == 0 {
            self.timer_state = TimerState::Finished;
            return true;
        }
        false
    }

    pub fn progress_pct(&self) -> f32 {
        let total = self.phase_duration() as f32;
        if total == 0.0 {
            return 0.0;
        }
        let elapsed = total - self.remaining_secs as f32;
        (elapsed / total * 100.0).clamp(0.0, 100.0)
    }

    pub fn fmt_remaining(&self) -> String {
        let m = self.remaining_secs / 60;
        let s = self.remaining_secs % 60;
        format!("{:02}:{:02}", m, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_second_decrements_remaining() {
        let mut state = PomodoroState::default();
        state.timer_state = TimerState::Running;
        let before = state.remaining_secs;
        assert!(!state.tick_second());
        assert_eq!(state.remaining_secs, before - 1);
    }

    #[test]
    fn tick_second_returns_true_when_finished() {
        let mut state = PomodoroState::default();
        state.remaining_secs = 1;
        state.timer_state = TimerState::Running;
        assert!(state.tick_second());
        assert_eq!(state.remaining_secs, 0);
        assert_eq!(state.timer_state, TimerState::Finished);
    }

    #[test]
    fn tick_second_does_nothing_when_paused() {
        let mut state = PomodoroState::default();
        state.timer_state = TimerState::Paused;
        let before = state.remaining_secs;
        assert!(!state.tick_second());
        assert_eq!(state.remaining_secs, before);
    }

    #[test]
    fn tick_second_does_nothing_when_idle() {
        let mut state = PomodoroState::default();
        state.timer_state = TimerState::Idle;
        let before = state.remaining_secs;
        assert!(!state.tick_second());
        assert_eq!(state.remaining_secs, before);
    }

    #[test]
    fn tick_second_accumulates_focus_stats() {
        let mut state = PomodoroState::default();
        state.phase = Phase::Work;
        state.timer_state = TimerState::Running;
        let before = state.data.stats.total_focus_secs;
        let _ = state.tick_second();
        assert_eq!(state.data.stats.total_focus_secs, before + 1);
    }

    #[test]
    fn tick_second_accumulates_break_stats() {
        let mut state = PomodoroState::default();
        state.phase = Phase::ShortBreak;
        state.timer_state = TimerState::Running;
        let before = state.data.stats.total_break_secs;
        let _ = state.tick_second();
        assert_eq!(state.data.stats.total_break_secs, before + 1);
    }

    #[test]
    fn advance_phase_work_to_short_break() {
        let mut state = PomodoroState::default();
        state.phase = Phase::Work;
        state.sessions_done = 1;
        state.advance_phase();
        assert_eq!(state.phase, Phase::ShortBreak);
        assert_eq!(state.sessions_done, 2);
        assert_eq!(state.data.stats.sessions_completed, 1);
        assert_eq!(state.timer_state, TimerState::Idle);
    }

    #[test]
    fn advance_phase_work_to_long_break_after_n_sessions() {
        let mut state = PomodoroState::default();
        state.phase = Phase::Work;
        state.sessions_done = 3;
        state.data.config.long_break_after = 4;
        state.advance_phase();
        assert_eq!(state.phase, Phase::LongBreak);
        assert_eq!(state.sessions_done, 4);
    }

    #[test]
    fn advance_phase_break_to_work() {
        let mut state = PomodoroState::default();
        state.phase = Phase::ShortBreak;
        state.advance_phase();
        assert_eq!(state.phase, Phase::Work);
        assert_eq!(state.timer_state, TimerState::Idle);
        state.phase = Phase::LongBreak;
        state.advance_phase();
        assert_eq!(state.phase, Phase::Work);
        assert_eq!(state.timer_state, TimerState::Idle);
    }

    #[test]
    fn progress_pct_at_half() {
        let mut state = PomodoroState::default();
        state.phase = Phase::Work;
        state.data.config.work_secs = 100;
        state.remaining_secs = 50;
        assert_eq!(state.progress_pct(), 50.0);
    }

    #[test]
    fn progress_pct_at_zero() {
        let mut state = PomodoroState::default();
        state.phase = Phase::Work;
        state.data.config.work_secs = 100;
        state.remaining_secs = 100;
        assert_eq!(state.progress_pct(), 0.0);
    }

    #[test]
    fn progress_pct_at_full() {
        let mut state = PomodoroState::default();
        state.phase = Phase::Work;
        state.data.config.work_secs = 100;
        state.remaining_secs = 0;
        assert_eq!(state.progress_pct(), 100.0);
    }

    #[test]
    fn progress_pct_zero_duration() {
        let mut state = PomodoroState::default();
        state.data.config.work_secs = 0;
        state.remaining_secs = 0;
        assert_eq!(state.progress_pct(), 0.0);
    }

    #[test]
    fn fmt_remaining_formats_correctly() {
        let mut state = PomodoroState::default();
        state.remaining_secs = 25 * 60;
        assert_eq!(state.fmt_remaining(), "25:00");
        state.remaining_secs = 90;
        assert_eq!(state.fmt_remaining(), "01:30");
    }

    #[test]
    fn toggle_pause_starts_when_idle() {
        let mut state = PomodoroState::default();
        state.timer_state = TimerState::Idle;
        state.toggle_pause();
        assert_eq!(state.timer_state, TimerState::Running);
    }

    #[test]
    fn toggle_pause_pauses_when_running() {
        let mut state = PomodoroState::default();
        state.timer_state = TimerState::Running;
        state.toggle_pause();
        assert_eq!(state.timer_state, TimerState::Paused);
    }

    #[test]
    fn toggle_pause_resumes_when_paused() {
        let mut state = PomodoroState::default();
        state.timer_state = TimerState::Paused;
        state.toggle_pause();
        assert_eq!(state.timer_state, TimerState::Running);
    }

    #[test]
    fn toggle_pause_advances_when_finished() {
        let mut state = PomodoroState::default();
        state.phase = Phase::Work;
        state.timer_state = TimerState::Finished;
        state.toggle_pause();
        assert_eq!(state.phase, Phase::ShortBreak);
        assert_eq!(state.timer_state, TimerState::Idle);
    }

    #[test]
    fn reset_session_restores_duration() {
        let mut state = PomodoroState::default();
        state.start();
        state.remaining_secs = 100;
        state.reset_session();
        assert_eq!(state.remaining_secs, state.phase_duration());
        assert_eq!(state.timer_state, TimerState::Idle);
    }

    #[test]
    fn skip_advances_phase() {
        let mut state = PomodoroState::default();
        state.phase = Phase::Work;
        state.skip();
        assert_eq!(state.phase, Phase::ShortBreak);
    }

    #[test]
    fn config_default_values() {
        let config = PomodoroConfig::default();
        assert_eq!(config.work_secs, 25 * 60);
        assert_eq!(config.short_break_secs, 5 * 60);
        assert_eq!(config.long_break_secs, 15 * 60);
        assert_eq!(config.long_break_after, 4);
        assert!(!config.auto_start_breaks);
        assert!(!config.auto_start_work);
    }

    #[test]
    fn daily_stats_reset_on_new_date() {
        let mut data = PomodoroData::default();
        data.stats.date = "2025-01-01".into();
        data.stats.sessions_completed = 5;
        let today = "2025-01-02";
        assert_ne!(data.stats.date, today);
        // simulate reset
        let today_owned = today.to_string();
        let new_stats = DailyStats {
            date: today_owned,
            ..Default::default()
        };
        data.stats = new_stats;
        assert_eq!(data.stats.sessions_completed, 0);
        assert_eq!(data.stats.total_focus_secs, 0);
        assert_eq!(data.stats.total_break_secs, 0);
        assert_eq!(data.stats.date, "2025-01-02");
    }

    #[test]
    fn start_sets_remaining_when_idle() {
        let mut state = PomodoroState::default();
        state.remaining_secs = 0;
        state.timer_state = TimerState::Idle;
        state.start();
        assert_eq!(state.timer_state, TimerState::Running);
        assert_eq!(state.remaining_secs, state.phase_duration());
    }

    #[test]
    fn start_sets_remaining_when_finished() {
        let mut state = PomodoroState::default();
        state.remaining_secs = 0;
        state.timer_state = TimerState::Finished;
        state.start();
        assert_eq!(state.timer_state, TimerState::Running);
        assert_eq!(state.remaining_secs, state.phase_duration());
    }

    #[test]
    fn start_does_not_reset_remaining_when_paused() {
        let mut state = PomodoroState::default();
        state.timer_state = TimerState::Paused;
        state.remaining_secs = 500;
        state.start();
        assert_eq!(state.timer_state, TimerState::Running);
        assert_eq!(state.remaining_secs, 500);
    }

    #[test]
    fn phase_label_returns_correct_labels() {
        assert_eq!(Phase::Work.label(), "FOCUS");
        assert_eq!(Phase::ShortBreak.label(), "SHORT BREAK");
        assert_eq!(Phase::LongBreak.label(), "LONG BREAK");
    }
}
