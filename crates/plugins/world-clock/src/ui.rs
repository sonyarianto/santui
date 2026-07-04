use chrono::{Offset, Timelike};
use chrono_tz::{OffsetComponents, Tz};
use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};
use santui_ipc::ui;

use crate::state::{Screen, WorldTimeState};

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

fn grid_cols(area_w: u16) -> u16 {
    ((area_w.saturating_sub(2)) / (card_w() + 1)).max(1)
}

pub fn render_ui(state: &WorldTimeState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    match &state.screen {
        Screen::Grid => render_grid(state, theme, w, h),
        Screen::Search => render_search(state, theme, w, h),
        Screen::Rename(idx) => render_rename(state, theme, w, h, *idx),
    }
}

fn render_grid(state: &WorldTimeState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let cols = grid_cols(w);
    let gap: u16 = 1;
    let mx: u16 = 1;
    let cw = card_w();
    let ch = card_h();

    if state.clocks.is_empty() {
        let text = "Add a timezone (press 'a')";
        let x = (w.saturating_sub(text.len() as u16)) / 2;
        cmds.push(RenderCmd::Text {
            x,
            y: h / 2,
            text: text.into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
        return cmds;
    }

    for (i, clock) in state.clocks.iter().enumerate() {
        let col = i as u16 % cols;
        let row = i as u16 / cols;
        let cx = mx + col * (cw + gap);
        let cy = row * ch;

        let is_selected = i == state.selected;

        cmds.push(RenderCmd::Border {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
            fg: theme.border,
            bg: None,
            borders: BORDER_ALL,
            title: if is_selected {
                Some("●".into())
            } else {
                None
            },
            title_fg: if is_selected { Some(theme.text) } else { None },
            title_dash_fg: Some(theme.border),
        });

        let dt = chrono::Utc::now().with_timezone(&clock.tz);
        let offset_str = fmt_offset(clock.tz);
        let dst_active = dt.offset().dst_offset().num_seconds() != 0;

        cmds.push(RenderCmd::Text {
            x: cx + 2,
            y: cy + 1,
            text: santui_ipc::ui::truncate(&clock.label, 12),
            fg: Some(theme.text),
            bg: None,
            bold: false,
        });
        cmds.push(RenderCmd::Text {
            x: cx + cw.saturating_sub(2 + offset_str.len() as u16),
            y: cy + 1,
            text: offset_str,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });

        let time_str = format!("{:02}:{:02}:{:02}", dt.hour(), dt.minute(), dt.second());
        cmds.push(RenderCmd::Text {
            x: cx + 2,
            y: cy + 3,
            text: time_str,
            fg: Some(theme.accent),
            bg: None,
            bold: true,
        });

        let date_str = dt.format("%a, %-d %b %Y").to_string();
        cmds.push(RenderCmd::Text {
            x: cx + 2,
            y: cy + 5,
            text: date_str,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
        if dst_active {
            cmds.push(RenderCmd::Text {
                x: cx + cw - 3,
                y: cy + 5,
                text: "D".into(),
                fg: Some(theme.highlight),
                bg: None,
                bold: true,
            });
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
        cmds.push(RenderCmd::Text {
            x: r.ix,
            y: input_y,
            text: "Search...".into(),
            fg: Some(theme.text_muted),
            bg: Some(theme.background_panel),
            bold: false,
        });
    } else {
        cmds.push(RenderCmd::Text {
            x: r.ix,
            y: input_y,
            text: state.search_query.clone(),
            fg: Some(theme.text),
            bg: Some(theme.background_panel),
            bold: false,
        });
        if state.search_cursor_visible {
            cmds.push(RenderCmd::Text {
                x: r.ix + state.search_query.len() as u16,
                y: input_y,
                text: " ".into(),
                fg: Some(theme.inverted_text),
                bg: Some(theme.highlight),
                bold: false,
            });
        }
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
    cmds.push(RenderCmd::Text {
        x: r.ix,
        y: hint_y,
        text: "↑↓ pgup pgdn  ↵ add".into(),
        fg: Some(theme.text_muted),
        bg: Some(theme.background_panel),
        bold: false,
    });

    cmds
}

fn render_rename(
    state: &WorldTimeState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    idx: usize,
) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let popup_w = 30u16;
    let popup_h = 4u16;

    let cols = grid_cols(w);
    let col = idx as u16 % cols;
    let row = idx as u16 / cols;
    let card_x = 1 + col * (card_w() + 1);
    let card_y = row * card_h();
    let popup_x = card_x.min(w.saturating_sub(popup_w));
    let popup_y = card_y;

    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w,
        h,
        bg: theme.background_overlay,
    });

    cmds.push(RenderCmd::Border {
        x: popup_x,
        y: popup_y,
        w: popup_w,
        h: popup_h,
        fg: theme.border,
        bg: Some(theme.background_overlay),
        borders: BORDER_ALL,
        title: Some("Rename".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    let input_text = format!("> {}", state.rename_buf);
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: popup_y + 1,
        text: santui_ipc::ui::truncate(&input_text, popup_w.saturating_sub(4) as usize),
        fg: Some(theme.text),
        bg: None,
        bold: false,
    });

    cmds
}

use crate::timezones;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Screen, WorldTimeState};

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
    fn empty_state_does_not_panic() {
        let s = WorldTimeState::default();
        let cmds = render_ui(&s, &test_theme(), 120, 30);
        let _ = cmds;
    }
}
