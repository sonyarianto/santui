use chrono::{Local, NaiveDate};
use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};

use crate::state::{FocusField, HabitState, Screen, COLOR_PRESETS};

pub fn render_ui(state: &HabitState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];

    match &state.screen {
        Screen::Overview => cmds.extend(render_overview(state, theme, w, h)),
        Screen::Detail => cmds.extend(render_detail(state, theme, w, h)),
        Screen::Editor => cmds.extend(render_editor(state, theme, w, h)),
        Screen::DayDetail => cmds.extend(render_day_detail(state, theme, w, h)),
    }

    cmds
}

fn render_overview(state: &HabitState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let title = if state.filter_mode && !state.filter_query.is_empty() {
        format!(
            "Habit Tracker \u{2014} /{}",
            santui_ipc::ui::truncate(&state.filter_query, 30)
        )
    } else {
        "Habit Tracker".into()
    };

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let habits = state.filtered_habits();
    let mini_dates = HabitState::mini_heatmap_dates();
    let today_str = Local::now().format("%Y-%m-%d").to_string();

    if habits.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: h / 2 - 1,
            text: "No habits yet.".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: h / 2,
            text: "Press n to create your first habit.".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else {
        let _max_visible = (h.saturating_sub(6)) as usize;
        for (i, habit) in habits.iter().enumerate() {
            let y = 1 + i as u16;
            if y >= h.saturating_sub(4) {
                break;
            }
            let is_selected = i == state.cursor;

            let heatmap_str = build_mini_heatmap(state, &habit.id, &mini_dates, &today_str, theme);

            let streak = state.streak(&habit.id);
            let rate = state.completion_rate(&habit.id, 30);

            let display_name =
                santui_ipc::ui::truncate(&habit.name, 14.max(w.saturating_sub(50) as usize / 2));

            let line = format!(
                "{:>2} {:<width$} {} {:>6}d {:>3.0}%",
                if is_selected { "\u{25b6}" } else { " " },
                display_name,
                heatmap_str,
                streak,
                rate * 100.0,
                width = 14.min(w as usize / 3),
            );

            cmds.push(RenderCmd::Text {
                x: 2,
                y,
                text: line,
                fg: if is_selected {
                    Some(theme.inverted_text)
                } else {
                    Some(theme.text)
                },
                bg: if is_selected {
                    Some(theme.accent)
                } else {
                    None
                },
                bold: is_selected,
                modifiers: 0,
            });
        }
    }

    let legend_y = h.saturating_sub(2);
    cmds.push(RenderCmd::Text {
        x: 2,
        y: legend_y,
        text: "\u{25c9} completed  \u{25cc} missed  - future/no data".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

fn build_mini_heatmap(
    state: &HabitState,
    habit_id: &str,
    dates: &[String],
    today_str: &str,
    _theme: &ThemeData,
) -> String {
    let mut chars = Vec::new();
    for date in dates.iter() {
        if date == today_str {
            if state.is_completed_on(habit_id, date) {
                chars.push('◉');
            } else {
                chars.push('◌');
            }
        } else {
            match NaiveDate::parse_from_str(date, "%Y-%m-%d") {
                Ok(parsed) => {
                    let today_parsed = Local::now().date_naive();
                    if parsed > today_parsed {
                        chars.push('-');
                    } else if state.is_completed_on(habit_id, date) {
                        chars.push('◉');
                    } else {
                        chars.push('◌');
                    }
                }
                Err(_) => chars.push('-'),
            }
        }
    }
    chars.into_iter().collect()
}

fn render_detail(state: &HabitState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let habit = state.data.habits.get(state.detail_habit_idx);
    let habit_name = habit.map(|h| h.name.as_str()).unwrap_or("Unknown");
    let habit_id = habit.map(|h| h.id.as_str()).unwrap_or("");
    let streak = state.streak(habit_id);

    let title = format!("{} \u{2014} {}-day streak", habit_name, streak);

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let weeks = state.heatmap_weeks(habit_id);
    let day_headers = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let today_str = Local::now().format("%Y-%m-%d").to_string();
    let yesterday_str = Local::now()
        .date_naive()
        .pred_opt()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    let start_x: u16 = 4;
    let header_y: u16 = 1;

    for (i, day_name) in day_headers.iter().enumerate() {
        cmds.push(RenderCmd::Text {
            x: start_x + i as u16 * 3,
            y: header_y,
            text: day_name.to_string(),
            fg: Some(theme.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        });
    }

    let cell_w: u16 = 3;
    let max_visible_weeks = ((w.saturating_sub(start_x)) / (7 * cell_w))
        .min(weeks.len() as u16)
        .max(1);

    for (week_idx, week) in weeks
        .iter()
        .skip(state.heatmap_scroll)
        .take(max_visible_weeks as usize)
        .enumerate()
    {
        let grid_y = header_y + 1 + week_idx as u16;
        if grid_y >= h.saturating_sub(2) {
            break;
        }
        for (day_idx, (date_str, completed)) in week.iter().enumerate() {
            let cell_x = start_x + day_idx as u16 * cell_w;
            let is_cursor = week_idx + state.heatmap_scroll == state.heatmap_row
                && day_idx == state.heatmap_col;
            let is_future = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                Ok(d) => d > Local::now().date_naive(),
                Err(_) => true,
            };

            let (symbol, fg, bold) = if is_future {
                ("\u{2591}".to_string(), theme.text_muted, false)
            } else if *completed {
                if date_str == &today_str {
                    ("\u{25c9}".to_string(), theme.accent, true)
                } else {
                    ("\u{25c9}".to_string(), theme.success, false)
                }
            } else if date_str == &today_str || date_str == &yesterday_str {
                ("\u{25cc}".to_string(), theme.error, true)
            } else {
                ("\u{25cc}".to_string(), theme.text_muted, false)
            };

            let display = if is_cursor {
                format!("[{}]", symbol)
            } else {
                format!(" {} ", symbol)
            };

            cmds.push(RenderCmd::Text {
                x: cell_x.saturating_sub(1),
                y: grid_y,
                text: display,
                fg: if is_cursor {
                    Some(theme.inverted_text)
                } else {
                    Some(fg)
                },
                bg: if is_cursor { Some(theme.accent) } else { None },
                bold: bold || is_cursor,
                modifiers: 0,
            });
        }
    }

    let date_at_cursor = weeks
        .get(state.heatmap_row)
        .and_then(|w| w.get(state.heatmap_col))
        .map(|(d, _)| d.as_str())
        .unwrap_or("--");

    let info_y = header_y + 1 + max_visible_weeks + 1;
    if info_y < h.saturating_sub(2) {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: info_y,
            text: format!(
                "Date: {}  {}",
                date_at_cursor,
                if state.is_completed_on(habit_id, date_at_cursor) {
                    "\u{25c9} completed"
                } else {
                    "\u{25cc} missed"
                }
            ),
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    let note = state.get_entry(habit_id, date_at_cursor).and_then(|e| {
        if e.note.is_empty() {
            None
        } else {
            Some(e.note.as_str())
        }
    });
    if let Some(note_text) = note {
        if info_y + 1 < h.saturating_sub(2) {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: info_y + 1,
                text: format!(
                    "Note: {}",
                    santui_ipc::ui::truncate(note_text, (w.saturating_sub(10)) as usize)
                ),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    }

    cmds
}

fn render_editor(state: &HabitState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let is_new = state.editor_habit.as_ref().is_none_or(|h| h.id.is_empty());
    let title = if is_new {
        "New Habit".into()
    } else {
        "Edit Habit".into()
    };

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let habit = state.editor_habit.clone().unwrap_or_default();
    let fields = ["Name", "Description", "Color"];
    let field_values: Vec<String> = vec![
        format!(
            "{}",
            if state.editing && state.editor_focus == FocusField::Name {
                format!("{}_", state.editor_buffer)
            } else {
                habit.name.clone()
            }
        ),
        format!(
            "{}",
            if state.editing && state.editor_focus == FocusField::Description {
                format!("{}_", state.editor_buffer)
            } else {
                habit.description.clone()
            }
        ),
        format!("{}", habit.color),
    ];

    for (i, (label, value)) in fields.iter().zip(field_values.iter()).enumerate() {
        let y = 2 + i as u16;
        let is_focused = state.editor_focus
            == match i {
                0 => FocusField::Name,
                1 => FocusField::Description,
                2 => FocusField::Color,
                _ => FocusField::Name,
            };

        let prefix = if is_focused { "\u{25b8} " } else { "  " };
        let display = format!("{:<width$} {} {}", label, prefix, value, width = 12);

        cmds.push(RenderCmd::Text {
            x: 4,
            y,
            text: display,
            fg: if is_focused {
                Some(theme.inverted_text)
            } else {
                Some(theme.text)
            },
            bg: if is_focused { Some(theme.accent) } else { None },
            bold: is_focused,
            modifiers: 0,
        });
    }

    let color_row_y = 4;
    let preset_str = COLOR_PRESETS
        .iter()
        .map(|c| {
            if *c == habit.color {
                format!("[{}]", c)
            } else {
                format!(" {} ", c)
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    cmds.push(RenderCmd::Text {
        x: 4,
        y: color_row_y,
        text: preset_str,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

fn render_day_detail(state: &HabitState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let day_name = HabitState::day_name(&state.day_detail_date);
    let title = format!("{} ({})", state.day_detail_date, day_name);

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!("{:<20} {:>8}  {}", "Habit", "Status", "Note"),
        fg: Some(theme.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let habits = state.filtered_habits();
    let max_visible = (h.saturating_sub(5)) as usize;

    for (i, habit) in habits
        .iter()
        .skip(state.day_detail_scroll)
        .take(max_visible)
        .enumerate()
    {
        let y = 2 + i as u16;
        let is_selected = i + state.day_detail_scroll == state.cursor;
        let completed = state.is_completed_on(&habit.id, &state.day_detail_date);
        let entry_note = state
            .get_entry(&habit.id, &state.day_detail_date)
            .and_then(|e| {
                if e.note.is_empty() {
                    None
                } else {
                    Some(e.note.as_str())
                }
            })
            .unwrap_or("-");

        let status = if completed {
            "\u{25c9} Done"
        } else {
            "\u{25cc} Missed"
        };
        let note_display =
            if state.note_editing && i + state.day_detail_scroll == state.note_habit_idx {
                format!("{}_", state.note_buffer)
            } else {
                entry_note.to_string()
            };

        let line = format!(
            "{:<20} {:>8}  {}",
            santui_ipc::ui::truncate(&habit.name, 20),
            status,
            santui_ipc::ui::truncate(&note_display, (w.saturating_sub(34)) as usize),
        );

        cmds.push(RenderCmd::Text {
            x: 2,
            y,
            text: line,
            fg: if is_selected {
                Some(theme.inverted_text)
            } else {
                Some(theme.text)
            },
            bg: if is_selected {
                Some(theme.accent)
            } else {
                None
            },
            bold: is_selected,
            modifiers: 0,
        });
    }

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

    fn make_state() -> crate::state::HabitState {
        let mut state = crate::state::HabitState::default();
        state.data.habits.push(crate::state::Habit {
            id: "exercise-2026".into(),
            name: "Exercise".into(),
            description: "Daily workout".into(),
            color: "green".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        });
        state.data.habits.push(crate::state::Habit {
            id: "read-2026".into(),
            name: "Read".into(),
            description: "Read books".into(),
            color: "blue".into(),
            created_at: "2026-06-02".into(),
            archived: false,
        });
        state.rebuild_sorted();
        state
    }

    #[test]
    fn renders_empty_overview_when_no_habits() {
        let state = crate::state::HabitState::default();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_empty = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("No habits yet")),
        );
        assert!(has_empty);
    }

    #[test]
    fn renders_habit_list_with_streak_and_rate() {
        let state = make_state();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_exercise = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Exercise")));
        assert!(has_exercise);
    }

    #[test]
    fn renders_mini_heatmap_inline() {
        let state = make_state();
        let cmds = render_ui(&state, &test_theme(), 120, 24);
        let has_heatmap = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Text { ref text, .. } if text.contains('\u{25c9}') || text.contains('\u{25cc}'))
        });
        assert!(has_heatmap);
    }

    #[test]
    fn renders_detail_heatmap_grid() {
        let mut state = make_state();
        state.screen = Screen::Detail;
        state.detail_habit_idx = 0;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_header = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Mon")));
        assert!(has_header);
        let has_cell = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Text { ref text, .. } if text.contains('\u{25c9}') || text.contains('\u{25cc}') || text.contains('\u{2591}'))
        });
        assert!(has_cell);
    }

    #[test]
    fn renders_editor_form() {
        let mut state = crate::state::HabitState::default();
        state.screen = Screen::Editor;
        state.editor_habit = Some(crate::state::Habit {
            id: String::new(),
            name: "Test".into(),
            description: "A test habit".into(),
            color: "green".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        });
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_name = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Name")));
        assert!(has_name);
        let has_preset = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("green")));
        assert!(has_preset);
    }

    #[test]
    fn renders_day_detail_screen() {
        let mut state = make_state();
        state.screen = Screen::DayDetail;
        state.day_detail_date = "2026-06-15".into();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_date = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { ref title, .. } if title.as_deref().map_or(false, |t| t.contains("2026-06-15")))
        });
        assert!(has_date);
        let has_status = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Done") || text.contains("Missed"))
        });
        assert!(has_status);
    }
}
