use chrono::{Datelike, Offset, Timelike};
use chrono_tz::Tz;
use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};

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
        Screen::Detail(idx) => render_detail(state, theme, w, h, *idx),
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
        cmds.push(RenderCmd::Text {
            x: 2,
            y: h / 2,
            text: "Add a timezone (press 'a')".into(),
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
        let hour = dt.hour();
        let dot_fg = match hour {
            9..=16 => theme.success,
            7..=8 | 17..=18 => theme.highlight,
            _ => theme.text_muted,
        };

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
        cmds.push(RenderCmd::Text {
            x: cx + cw - 3,
            y: cy + 5,
            text: "●".into(),
            fg: Some(dot_fg),
            bg: None,
            bold: false,
        });
    }

    cmds
}

fn render_detail(
    state: &WorldTimeState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    idx: usize,
) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let clock = &state.clocks[idx];
    let dt = chrono::Utc::now().with_timezone(&clock.tz);
    let offset_str = fmt_offset(clock.tz);
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(clock.label.clone()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    let mut row = 1;
    let header = format!(
        "{}, {} {} {}    {:02}:{:02}:{:02} {}    UTC {}",
        dt.format("%A"),
        dt.day(),
        dt.format("%B"),
        dt.year(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        dt.format("%Z"),
        offset_str,
    );
    cmds.push(RenderCmd::Text {
        x: 2,
        y: row,
        text: header,
        fg: Some(theme.text),
        bg: None,
        bold: false,
    });
    row += 1;

    let dst = dt.format("%Z").to_string();
    let std_name = clock.tz.name().split('/').next_back().unwrap_or("");
    let is_dst = dst != std_name;
    let dst_str = if is_dst { "Yes" } else { "No" };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: row,
        text: format!("DST: {}", dst_str),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });
    row += 2;

    let bar_y = row;
    for hour in 0..24 {
        let col = 2 + hour * 2;
        let ch = match hour {
            9..=16 => '▓',
            _ => '░',
        };
        cmds.push(RenderCmd::Text {
            x: col,
            y: bar_y,
            text: ch.to_string(),
            fg: if (9..=16).contains(&hour) {
                Some(theme.accent)
            } else {
                Some(theme.text_muted)
            },
            bg: None,
            bold: false,
        });
    }
    let local_h = dt.hour();
    let now_col = 2 + (local_h as u16).min(23) * 2;
    if local_h < 24 {
        cmds.push(RenderCmd::Text {
            x: now_col,
            y: bar_y,
            text: "▲".into(),
            fg: Some(theme.success),
            bg: None,
            bold: true,
        });
    }
    row += 2;

    let hour_labels: String = (0..24).map(|h| format!("{:02} ", h)).collect();
    cmds.push(RenderCmd::Text {
        x: 2,
        y: row,
        text: hour_labels,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    cmds
}

fn render_search(state: &WorldTimeState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let popup_w = 50.min(w.saturating_sub(4));
    let result_count = state.search_results.len();
    let popup_h = (12u16)
        .min(result_count.saturating_add(4) as u16)
        .min(h.saturating_sub(2));
    let popup_x = (w - popup_w) / 2;
    let popup_y = (h - popup_h) / 2;

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
        title: Some("Add Timezone".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    let input_y = popup_y + 1;
    let input_text = format!("> {}", state.search_query);
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: input_y,
        text: santui_ipc::ui::truncate(&input_text, popup_w.saturating_sub(4) as usize),
        fg: Some(theme.text),
        bg: None,
        bold: false,
    });

    let list_y = input_y + 2;
    let max_visible = popup_h.saturating_sub(4) as usize;
    for (i, tz) in state.search_results.iter().enumerate().take(max_visible) {
        let is_selected_result = i == state.search_cursor;
        let name = timezones::city_name(*tz);
        let line = format!(" {}  {}", santui_ipc::ui::truncate(&name, 16), tz.name(),);
        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y: list_y + i as u16,
            text: line,
            fg: if is_selected_result {
                Some(theme.accent)
            } else {
                Some(theme.text)
            },
            bg: None,
            bold: is_selected_result,
        });
    }

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
