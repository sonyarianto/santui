use chrono::Offset;
use chrono_tz::{OffsetComponents, Tz};
use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};
use santui_ipc::ui;

use crate::state::{DateFormat, HourFormat, Screen, WorldTimeState};

fn fmt_offset(tz: Tz) -> String {
    let dt = chrono::Utc::now().with_timezone(&tz);
    let offset_secs = dt.offset().fix().local_minus_utc();
    let hours = offset_secs / 3600;
    let mins = (offset_secs.abs() / 60) % 60;
    let sign = if hours >= 0 { '+' } else { '-' };
    format!("{}{:02}:{:02}", sign, hours.abs(), mins)
}

fn card_w() -> u16 {
    26
}

fn card_h() -> u16 {
    7
}

pub(crate) fn grid_cols(area_w: u16) -> u16 {
    ((area_w.saturating_sub(4)) / (card_w() + 1)).max(1)
}

pub fn render_ui(state: &WorldTimeState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    match &state.screen {
        Screen::Grid => render_grid(state, theme, w, h),
        Screen::Search => render_search(state, theme, w, h),
        Screen::Rename(_) => render_rename(state, theme, w, h),
    }
}

fn render_grid(state: &WorldTimeState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let cols = grid_cols(w);
    let gap: u16 = 1;
    let mx: u16 = 2;
    let my: u16 = 1;
    let cw = card_w();
    let ch = card_h();

    cmds.push(RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("World Clock".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    if state.clocks.is_empty() {
        let text = "Add a timezone (press 'a')";
        let x = mx + (w.saturating_sub(4).saturating_sub(text.len() as u16)) / 2;
        ui::push_text(&mut cmds, x, my + h / 2, text, theme.text_muted, false);
        return cmds;
    }

    for (i, clock) in state.clocks.iter().enumerate() {
        let col = i as u16 % cols;
        let row = i as u16 / cols;
        let cx = mx + col * (cw + gap);
        let cy = my + row * ch;

        let is_selected = i == state.selected;

        ui::draw_panel(
            &mut cmds,
            theme,
            cx,
            cy,
            cw,
            ch,
            None,
            ui::PanelOpts {
                focused: false,
                dim_unfocused: true,
                footer: None,
                selected: is_selected,
                bg: None,
            },
        );

        let dt = chrono::Utc::now().with_timezone(&clock.tz);
        let offset_str = fmt_offset(clock.tz);
        let dst_active = dt.offset().dst_offset().num_seconds() != 0;

        ui::text_at(
            &mut cmds,
            cx + 2,
            cy + 1,
            &clock.label,
            theme.text,
            None,
            12,
        );
        ui::push_text(
            &mut cmds,
            cx + ui::right_align_x(cw, &offset_str),
            cy + 1,
            offset_str,
            theme.text_muted,
            false,
        );

        let time_str = match state.hour_format {
            HourFormat::TwentyFour => dt.format("%H:%M:%S").to_string(),
            HourFormat::Twelve => dt.format("%-I:%M:%S %p").to_string(),
        };
        ui::push_text(&mut cmds, cx + 2, cy + 3, time_str, theme.accent, true);

        let date_str = match state.date_format {
            DateFormat::MonthFirst => dt.format("%a, %b %-d %Y").to_string(),
            DateFormat::DayFirst => dt.format("%a, %-d %b %Y").to_string(),
        };
        ui::push_text(&mut cmds, cx + 2, cy + 5, date_str, theme.text_muted, false);
        if dst_active {
            ui::push_text(
                &mut cmds,
                cx + ui::right_align_x(cw, "DST"),
                cy + 5,
                "DST",
                theme.text_muted,
                false,
            );
        }
    }

    cmds
}

fn render_search(state: &WorldTimeState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = render_grid(state, theme, w, h);
    let title_h = 5u16;

    const MAX_ITEMS: usize = 12;
    let item_count = state.search_results.len().min(MAX_ITEMS);
    let popup_h = (title_h + item_count as u16 + 3).min(h).max(title_h + 1);
    let r = ui::palette_rect(w, h, popup_h);
    ui::palette_bg(&mut cmds, theme, &r);

    ui::palette_title(&mut cmds, theme, &r, 1, "Add Timezone");

    let input_y = r.y + 3;
    if state.search_query.is_empty() {
        ui::text_at(
            &mut cmds,
            r.ix,
            input_y,
            "Search...",
            theme.text_muted,
            Some(theme.background_panel),
            r.iw,
        );
    } else {
        ui::text_at(
            &mut cmds,
            r.ix,
            input_y,
            &state.search_query,
            theme.text,
            Some(theme.background_panel),
            r.iw,
        );
        let cursor = ui::blink_cursor(state.tick_counter);
        cmds.push(RenderCmd::Text {
            x: r.ix + state.search_query.len() as u16,
            y: input_y,
            text: cursor.to_string(),
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: false,
            modifiers: 0,
        });
    }

    let scroll = state.search_scroll;
    let end = (scroll + item_count).min(state.search_results.len());
    for (y_off, i) in (title_h..).zip(scroll..end) {
        let tz = state.search_results[i];
        let selected = i == state.search_cursor;
        ui::palette_item(
            &mut cmds,
            theme,
            &r,
            y_off,
            &timezones::city_name(tz),
            selected,
        );
    }

    let hint_y = r.y + title_h + item_count as u16 + 1;
    ui::hints_row(
        &mut cmds,
        theme,
        r.ix,
        hint_y,
        &[("↑↓", "navigate"), ("pgup/pgdn", "jump"), ("↵", "add")],
        r.iw as usize,
    );

    cmds
}

fn render_rename(state: &WorldTimeState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = render_grid(state, theme, w, h);

    const TITLE_H: u16 = 5;
    let popup_h = TITLE_H + 2;
    let r = ui::palette_rect(w, h, popup_h);
    ui::palette_bg(&mut cmds, theme, &r);

    ui::palette_title(&mut cmds, theme, &r, 1, "Rename Clock");

    let input_y = r.y + 3;
    ui::text_at(
        &mut cmds,
        r.ix,
        input_y,
        &state.rename_buf,
        theme.text,
        Some(theme.background_panel),
        r.iw,
    );
    let cursor = ui::blink_cursor(state.tick_counter);
    cmds.push(RenderCmd::Text {
        x: (r.ix + state.rename_buf.len() as u16).min(r.ix + r.iw.saturating_sub(1)),
        y: input_y,
        text: cursor.to_string(),
        fg: Some(theme.inverted_text),
        bg: Some(theme.highlight),
        bold: false,
        modifiers: 0,
    });

    let hint_y = r.y + TITLE_H;
    ui::hints_row(
        &mut cmds,
        theme,
        r.ix,
        hint_y,
        &[("↵", "save"), ("ctrl+r", "default")],
        r.iw as usize,
    );

    cmds
}

use crate::timezones;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{DateFormat, HourFormat, Screen, WorldTimeState};

    fn test_theme() -> ThemeData {
        ThemeData {
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
        }
    }

    fn state_with_clocks() -> WorldTimeState {
        let mut s = WorldTimeState::default();
        s.add_clock(chrono_tz::Tz::Asia__Tokyo);
        s.add_clock(chrono_tz::Tz::Europe__London);
        s
    }

    #[test]
    fn grid_renders_clock_cards() {
        let s = state_with_clocks();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        assert!(!cmds.is_empty());
        let has_tokyo = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Tokyo")));
        assert!(has_tokyo);
    }

    #[test]
    fn grid_renders_london() {
        let s = state_with_clocks();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let has_london = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("London")));
        assert!(has_london);
    }

    #[test]
    fn search_overlay_renders_on_search_screen() {
        let mut s = state_with_clocks();
        s.screen = Screen::Search;
        s.search_query = "tok".into();
        s.apply_search();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        assert!(!cmds.is_empty());
    }

    #[test]
    fn renders_full_window_panel_with_title() {
        let s = state_with_clocks();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let panel = cmds.iter().find_map(|c| match c {
            RenderCmd::Border {
                title: Some(t),
                x: 0,
                y: 0,
                ..
            } => Some(t.clone()),
            _ => None,
        });
        assert_eq!(panel.as_deref(), Some("World Clock"));
    }

    #[test]
    fn selected_card_gets_highlight_border() {
        let s = state_with_clocks();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        assert_eq!(borders.len(), 3, "panel + 2 cards");
        if let RenderCmd::Border { fg, x, .. } = borders[1] {
            assert_eq!(*fg, test_theme().highlight);
            assert_eq!(*x, 2);
        } else {
            panic!("expected border");
        }
    }

    #[test]
    fn unselected_card_gets_muted_border() {
        let s = state_with_clocks();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        if let RenderCmd::Border { fg, y, .. } = borders[2] {
            assert_eq!(*fg, test_theme().text_muted);
            assert_eq!(*y, 1);
        } else {
            panic!("expected border");
        }
    }

    #[test]
    fn cards_have_no_title_marker() {
        let s = state_with_clocks();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        for b in borders.iter().skip(1) {
            if let RenderCmd::Border { title, .. } = b {
                assert_eq!(title.as_deref(), None);
            }
        }
    }

    #[test]
    fn grid_date_uses_month_first_by_default() {
        let s = state_with_clocks();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let now = chrono::Utc::now().with_timezone(&s.clocks[0].tz);
        let expect = now.format("%a, %b %-d %Y").to_string();
        assert!(
            cmds.iter()
                .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == &expect)),
            "expected month-first date {expect:?}"
        );
    }

    #[test]
    fn grid_date_uses_day_first_when_toggled() {
        let mut s = state_with_clocks();
        s.date_format = DateFormat::DayFirst;
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let now = chrono::Utc::now().with_timezone(&s.clocks[0].tz);
        let expect = now.format("%a, %-d %b %Y").to_string();
        assert!(
            cmds.iter()
                .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == &expect)),
            "expected day-first date {expect:?}"
        );
    }

    #[test]
    fn grid_time_uses_twenty_four_hour_by_default() {
        let s = state_with_clocks();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let now = chrono::Utc::now().with_timezone(&s.clocks[0].tz);
        let expect = now.format("%H:%M:%S").to_string();
        assert!(
            cmds.iter()
                .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == &expect)),
            "expected 24h time {expect:?}"
        );
    }

    #[test]
    fn grid_time_uses_twelve_hour_when_toggled() {
        let mut s = state_with_clocks();
        s.hour_format = HourFormat::Twelve;
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let now = chrono::Utc::now().with_timezone(&s.clocks[0].tz);
        let expect = now.format("%-I:%M:%S %p").to_string();
        assert!(
            cmds.iter()
                .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == &expect)),
            "expected 12h time {expect:?}"
        );
    }

    #[test]
    fn empty_state_prompt_centered_inside_panel() {
        let s = WorldTimeState::default();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let prompt = cmds.iter().find(|c| {
            matches!(c, RenderCmd::Text { ref text, .. } if text == "Add a timezone (press 'a')")
        });
        assert!(prompt.is_some());
    }

    #[test]
    fn empty_state_does_not_panic() {
        let s = WorldTimeState::default();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let _ = cmds;
    }

    #[test]
    fn rename_popup_matches_palette_layout() {
        let mut s = state_with_clocks();
        s.screen = Screen::Rename(0);
        s.rename_buf = "My City".into();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let title = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Rename Clock")));
        assert!(title);
        let has_buf = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("My City")));
        assert!(has_buf);
        let has_hint = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("default")));
        assert!(has_hint);
        let has_cursor = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Text { fg, bg, .. }
                if *fg == Some(test_theme().inverted_text) && *bg == Some(test_theme().highlight))
        });
        assert!(has_cursor);
        let has_grid = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { title, .. } if title.as_deref() == Some("World Clock"))
        });
        assert!(has_grid);
    }
}
