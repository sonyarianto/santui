use crate::state::TimerState;
use crate::state::{Phase, PomodoroState};
use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};

pub fn render_ui(state: &PomodoroState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];

    cmds.extend(render_main(state, theme, w, h));

    if state.show_settings {
        cmds.extend(render_settings(state, theme, w, h));
    }

    cmds
}

fn phase_color(phase: &Phase, theme: &ThemeData) -> [u8; 3] {
    match phase {
        Phase::Work => theme.accent,
        Phase::ShortBreak => theme.success,
        Phase::LongBreak => theme.highlight,
    }
}

fn render_main(state: &PomodoroState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let color = phase_color(&state.phase, theme);

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Pomodoro".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let content_h: u16 = 9;
    let top = if h >= content_h + 6 {
        ((h - content_h) / 2).max(2)
    } else {
        2
    };

    let phase_label = state.phase.label();
    let phase_x = (w / 2).saturating_sub(phase_label.len() as u16 / 2);
    cmds.push(RenderCmd::Text {
        x: phase_x,
        y: top,
        text: phase_label.into(),
        fg: Some(color),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let time_text = match &state.timer_state {
        TimerState::Finished => "DONE!".to_string(),
        TimerState::Paused => format!("{} [PAUSED]", state.fmt_remaining()),
        _ => state.fmt_remaining(),
    };
    let time_fg = match &state.timer_state {
        TimerState::Finished => Some(theme.success),
        TimerState::Paused => Some(theme.text_muted),
        TimerState::Idle => Some(theme.text_muted),
        TimerState::Running => Some(color),
    };
    let time_bold = matches!(
        state.timer_state,
        TimerState::Running | TimerState::Finished
    );
    let time_x = (w / 2).saturating_sub(time_text.len() as u16 / 2);
    cmds.push(RenderCmd::Text {
        x: time_x,
        y: top + 2,
        text: time_text,
        fg: time_fg,
        bg: None,
        bold: time_bold,
        modifiers: 0,
    });

    let bar_w = w.saturating_sub(12).max(8);
    let bar_x = 4;
    let bar_y = top + 4;
    let pct = state.progress_pct() as usize;
    let filled = (bar_w as f32 * (pct as f32 / 100.0)) as usize;
    let empty = bar_w as usize - filled;
    let bar_text = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));
    cmds.push(RenderCmd::Text {
        x: bar_x,
        y: bar_y,
        text: bar_text,
        fg: Some(color),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    let pct_text = format!("{:>3}%", pct);
    if bar_x + bar_w + 2 + pct_text.len() as u16 <= w {
        cmds.push(RenderCmd::Text {
            x: bar_x + bar_w + 2,
            y: bar_y,
            text: pct_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    let dots_y = top + 6;
    if dots_y + 2 < h {
        let total = state.data.config.long_break_after;
        let done = state.sessions_done.min(total);
        if total > 12 {
            let dots_text = format!("{done} / {total} sessions");
            let dots_x = (w / 2).saturating_sub(dots_text.len() as u16 / 2);
            cmds.push(RenderCmd::Text {
                x: dots_x,
                y: dots_y,
                text: dots_text,
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        } else {
            let filled_dots = "● ".repeat(done as usize);
            let hollow_dots = "○ ".repeat((total - done) as usize);
            let dots_len = filled_dots.trim_end().chars().count() as u16
                + if total - done > 0 { 1 } else { 0 }
                + hollow_dots.trim_end().chars().count() as u16;
            let dots_x = (w / 2).saturating_sub(dots_len / 2);
            if done > 0 {
                cmds.push(RenderCmd::Text {
                    x: dots_x,
                    y: dots_y,
                    text: filled_dots.trim_end().to_string(),
                    fg: Some(color),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
            }
            if total - done > 0 {
                cmds.push(RenderCmd::Text {
                    x: dots_x + filled_dots.chars().count() as u16,
                    y: dots_y,
                    text: hollow_dots.trim_end().to_string(),
                    fg: Some(theme.border),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
            }
        }
    }

    let stats_y = top + 8;
    if stats_y + 1 < h {
        let stats_text = format!(
            "Today: {} sessions · {} focused · {} break",
            state.data.stats.sessions_completed,
            fmt_minutes(state.data.stats.total_focus_secs),
            fmt_minutes(state.data.stats.total_break_secs)
        );
        let stats_x = (w / 2).saturating_sub(stats_text.len() as u16 / 2);
        cmds.push(RenderCmd::Text {
            x: stats_x,
            y: stats_y,
            text: stats_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    if h >= 14 {
        let cfg_text = format!(
            "{}m work · {}m break · {}m long",
            state.data.config.work_secs / 60,
            state.data.config.short_break_secs / 60,
            state.data.config.long_break_secs / 60
        );
        cmds.push(RenderCmd::Text {
            x: 2,
            y: h - 3,
            text: cfg_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds
}

fn fmt_minutes(secs: u64) -> String {
    let total_min = secs / 60;
    let hours = total_min / 60;
    let mins = total_min % 60;
    if hours > 0 {
        format!("{hours}h {mins:02}m")
    } else {
        format!("{mins}m")
    }
}

fn render_settings(state: &PomodoroState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let popup_w = 46;
    let popup_h = 12;
    let popup_x = (w.saturating_sub(popup_w)) / 2;
    let popup_y = (h.saturating_sub(popup_h)) / 2;

    santui_ipc::ui::dim_overlay(&mut cmds, theme);

    cmds.push(RenderCmd::Border {
        x: popup_x,
        y: popup_y,
        w: popup_w,
        h: popup_h,
        fg: theme.border,
        bg: Some(theme.background_overlay),
        borders: BORDER_ALL,
        title: Some("Settings".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let fields: [(&str, String); 6] = [
        (
            "Work duration",
            format!("{} min", state.data.config.work_secs / 60),
        ),
        (
            "Short break",
            format!("{} min", state.data.config.short_break_secs / 60),
        ),
        (
            "Long break",
            format!("{} min", state.data.config.long_break_secs / 60),
        ),
        (
            "Long break after",
            format!("{} sessions", state.data.config.long_break_after),
        ),
        (
            "Auto-start breaks",
            if state.data.config.auto_start_breaks {
                "Yes".into()
            } else {
                "No".into()
            },
        ),
        (
            "Auto-start work",
            if state.data.config.auto_start_work {
                "Yes".into()
            } else {
                "No".into()
            },
        ),
    ];

    let descs: [&str; 6] = [
        "Focus session length",
        "Rest between focus sessions",
        "Longer rest after every cycle",
        "Sessions before the long break",
        "Start breaks when work ends",
        "Start work when a break ends",
    ];

    for (i, (label, value)) in fields.iter().enumerate() {
        let y = popup_y + 1 + i as u16;
        let is_selected = i == state.settings_cursor;

        let label_fg = if is_selected {
            theme.inverted_text
        } else {
            theme.text
        };
        let label_bg = if is_selected {
            Some(theme.accent)
        } else {
            None
        };
        let value_fg = if is_selected {
            theme.inverted_text
        } else {
            theme.accent
        };
        let value_bg = if is_selected {
            Some(theme.accent)
        } else {
            None
        };
        let bold = is_selected;

        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y,
            text: label.to_string(),
            fg: Some(label_fg),
            bg: label_bg,
            bold,
            modifiers: 0,
        });

        let value_x = popup_x + popup_w - 2 - value.len() as u16;
        cmds.push(RenderCmd::Text {
            x: value_x,
            y,
            text: value.clone(),
            fg: Some(value_fg),
            bg: value_bg,
            bold,
            modifiers: 0,
        });
    }

    let desc = descs[state.settings_cursor.min(5)];
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: popup_y + 8,
        text: desc.to_string(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> ThemeData {
        ThemeData {
            text: [200; 3],
            text_muted: [100; 3],
            accent: [180; 3],
            highlight: [220; 3],
            logo: [255; 3],
            background: [0; 3],
            background_panel: [20; 3],
            background_overlay: [10; 3],
            border: [150; 3],
            success: [80; 3],
            error: [255; 3],
            inverted_text: [255; 3],
        }
    }

    fn test_state() -> PomodoroState {
        let mut state = PomodoroState::default();
        state.sessions_done = 2;
        state.data.stats.sessions_completed = 3;
        state.data.stats.total_focus_secs = 25200;
        state.data.stats.total_break_secs = 1800;
        state.timer_state = TimerState::Running;
        state.remaining_secs = 23 * 60 + 45;
        state
    }

    #[test]
    fn renders_phase_label() {
        let state = test_state();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_label = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("WORK")));
        assert!(has_label);
    }

    #[test]
    fn renders_time_remaining() {
        let state = test_state();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_time = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("23:45")));
        assert!(has_time);
    }

    #[test]
    fn renders_progress_bar_and_percentage() {
        let state = test_state();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_bar = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("█")));
        let has_pct = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("%")));
        assert!(has_bar);
        assert!(has_pct);
    }

    #[test]
    fn renders_session_count() {
        let state = test_state();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_count = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Today:")
                && text.contains("3 sessions"))
        });
        assert!(has_count);
    }

    #[test]
    fn renders_break_time() {
        let state = test_state();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_break = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("30m break")));
        assert!(has_break);
    }

    #[test]
    fn renders_config_summary() {
        let state = test_state();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_cfg = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("25m work")));
        assert!(has_cfg);
    }

    #[test]
    fn renders_session_dots() {
        let state = test_state();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_dots = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("●")));
        assert!(has_dots);
    }

    #[test]
    fn renders_paused_indicator_when_paused() {
        let mut state = test_state();
        state.timer_state = TimerState::Paused;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_paused = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("[PAUSED]")));
        assert!(has_paused);
    }

    #[test]
    fn renders_finished_indicator() {
        let mut state = test_state();
        state.timer_state = TimerState::Finished;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_done = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("DONE!")));
        assert!(has_done);
    }

    #[test]
    fn renders_settings_overlay_when_show_settings() {
        let mut state = test_state();
        state.show_settings = true;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_overlay = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { ref title, .. } if title.as_deref() == Some("Settings"))
        });
        assert!(has_overlay);
    }

    #[test]
    fn settings_overlay_highlights_selected_field() {
        let mut state = test_state();
        state.show_settings = true;
        state.settings_cursor = 0;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_work_duration = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Work duration")),
        );
        assert!(has_work_duration);
    }

    #[test]
    fn renders_idle_time_dimmed() {
        let mut state = test_state();
        state.timer_state = TimerState::Idle;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let time_cmd = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("23:45")));
        if let Some(RenderCmd::Text { fg, .. }) = time_cmd {
            assert_eq!(*fg, Some(test_theme().text_muted));
        } else {
            panic!("time text not found");
        }
    }

    #[test]
    fn renders_focus_time_correctly() {
        let state = test_state();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_focus = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("7h 00m focused")),
        );
        assert!(has_focus);
    }
}
