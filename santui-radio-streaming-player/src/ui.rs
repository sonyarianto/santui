use crate::state::{PlayState, RadioState};
use santui_ipc::protocol::{RenderCmd, ThemeData};

fn panel_top(w: u16, title: &str) -> String {
    if w < 2 {
        return String::new();
    }
    let inner = w as usize - 2;
    let title_len = title.len();
    if title_len <= inner.saturating_sub(2) {
        let right = inner - 1 - title_len;
        format!("┌─{}{}┐", title, "─".repeat(right))
    } else {
        let n = inner.min(title_len);
        format!("┌{}┐", &title[..n])
    }
}

fn panel_bottom(w: u16) -> String {
    if w < 2 {
        return String::new();
    }
    format!("└{}┘", "─".repeat(w as usize - 2))
}

fn panel_mid(w: u16) -> String {
    if w < 2 {
        return String::new();
    }
    format!("│{}│", " ".repeat(w as usize - 2))
}

fn draw_panel(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    title: &str,
) {
    if w < 4 || h < 3 {
        return;
    }

    cmds.push(RenderCmd::Text {
        x,
        y,
        text: panel_top(w, title),
        fg: Some(theme.border),
        bg: None,
        bold: false,
    });

    for row in (y + 1)..(y + h - 1) {
        cmds.push(RenderCmd::Text {
            x,
            y: row,
            text: panel_mid(w),
            fg: Some(theme.border),
            bg: None,
            bold: false,
        });
    }

    cmds.push(RenderCmd::Text {
        x,
        y: y + h - 1,
        text: panel_bottom(w),
        fg: Some(theme.border),
        bg: None,
        bold: false,
    });
}

fn text_at(cmds: &mut Vec<RenderCmd>, x: u16, y: u16, text: &str, fg: [u8; 3], max_w: u16) {
    let max = max_w as usize;
    let display = if text.len() > max && max > 1 {
        let mut t = text.chars().take(max.saturating_sub(1)).collect::<String>();
        t.push('…');
        t
    } else {
        format!("{:<width$}", text, width = max)
    };
    cmds.push(RenderCmd::Text {
        x,
        y,
        text: display,
        fg: Some(fg),
        bg: None,
        bold: false,
    });
}

pub fn render_ui(
    state: &RadioState,
    theme: &ThemeData,
    area_w: u16,
    area_h: u16,
) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    if area_w < 10 || area_h < 3 {
        return cmds;
    }

    cmds.push(RenderCmd::Clear {
        x: 0,
        y: 0,
        w: area_w,
        h: area_h,
    });

    let left_w = (area_w / 3).max(12);
    let right_w = area_w.saturating_sub(left_w);
    let info_h = 6u16.min(area_h);

    // ---- Left panel: station list ----
    draw_panel(&mut cmds, theme, 0, 0, left_w, area_h, " Stations ");

    let inner_x = 2u16;
    let inner_w = left_w.saturating_sub(4);

    for (i, &station_idx) in state.filtered.iter().enumerate() {
        let item_y = 1u16 + i as u16;
        if item_y >= area_h.saturating_sub(1) {
            break;
        }

        let station = &state.stations[station_idx];
        let is_selected = i == state.selected;
        let is_current = state.current_station == Some(station_idx);

        let icon = if is_current { " ♫ " } else { "   " };
        let text = format!("{}{}", icon, station.name);
        let max_len = inner_w as usize;
        let display = if text.len() > max_len && max_len > 0 {
            let mut t = text
                .chars()
                .take(max_len.saturating_sub(1))
                .collect::<String>();
            t.push('…');
            t
        } else {
            format!("{:<width$}", text, width = max_len)
        };

        let (fg, bg, bold) = if is_selected {
            (Some([0u8, 0, 0]), Some(theme.highlight), false)
        } else if is_current {
            (Some(theme.accent), None, true)
        } else {
            (Some(theme.text), None, false)
        };

        cmds.push(RenderCmd::Text {
            x: inner_x,
            y: item_y,
            text: display,
            fg,
            bg,
            bold,
        });
    }

    // ---- Right panel: Now Playing ----
    draw_panel(
        &mut cmds,
        theme,
        left_w,
        0,
        right_w,
        info_h,
        " Now Playing ",
    );

    let r_inner_x = left_w + 2;
    let r_inner_w = right_w.saturating_sub(4);

    match &state.play_state {
        PlayState::Stopped => {
            text_at(
                &mut cmds,
                r_inner_x,
                2,
                "No station selected",
                theme.text_muted,
                r_inner_w,
            );
            text_at(
                &mut cmds,
                r_inner_x,
                3,
                "⏹  Stopped",
                theme.error,
                r_inner_w,
            );
        }
        PlayState::Playing(station_name) => {
            text_at(
                &mut cmds,
                r_inner_x,
                2,
                station_name,
                theme.success,
                r_inner_w,
            );

            let title = if state.song_title.is_empty() {
                "(no metadata)"
            } else {
                &state.song_title
            };
            text_at(&mut cmds, r_inner_x, 3, title, theme.text, r_inner_w);

            if let Some(ref info) = state.track_info {
                if let Some(ref artist) = info.artist {
                    text_at(&mut cmds, r_inner_x, 4, artist, theme.text_muted, r_inner_w);
                }
            }
        }
        PlayState::Error(e) => {
            text_at(&mut cmds, r_inner_x, 2, "Error", theme.error, r_inner_w);
            text_at(&mut cmds, r_inner_x, 3, e, theme.error, r_inner_w);
        }
    }

    // ---- Right panel bottom: Volume gauge ----
    let gauge_y = info_h;
    let gauge_h = area_h.saturating_sub(gauge_y);
    if gauge_h >= 3 {
        draw_panel(
            &mut cmds, theme, left_w, gauge_y, right_w, gauge_h, " Volume ",
        );

        let g_inner_x = left_w + 2;
        let g_inner_w = right_w.saturating_sub(4) as usize;
        let bar_w = g_inner_w.saturating_sub(5); // room for " 100%"
        let filled = (bar_w as u64 * state.volume as u64 / 100) as usize;
        let empty = bar_w.saturating_sub(filled);
        let gauge_text = format!(
            "{}{} {:>3}%",
            "█".repeat(filled),
            "░".repeat(empty),
            state.volume
        );
        cmds.push(RenderCmd::Text {
            x: g_inner_x,
            y: gauge_y + 2,
            text: gauge_text,
            fg: Some(theme.success),
            bg: None,
            bold: false,
        });
    }

    // ---- Help popup ----
    if state.show_help && area_w >= 40 && area_h >= 16 {
        let pw = (area_w / 2).min(50);
        let ph = 13u16;
        let px = (area_w - pw) / 2;
        let py = (area_h - ph) / 2;

        draw_panel(&mut cmds, theme, px, py, pw, ph, " Help ");

        let hx = px + 2;
        let hw = pw.saturating_sub(4);

        let help_lines: &[(&str, [u8; 3])] = &[
            ("↑/↓      Navigate station list", theme.text),
            ("Enter    Play selected station", theme.text),
            ("s         Stop playback", theme.text),
            ("+/-       Adjust volume", theme.text),
            ("/         Filter stations by name", theme.text),
            ("?         Toggle this help", theme.text),
            ("Esc       Back to Santui menu", theme.text),
            ("", theme.text_muted),
            ("Press any key to close", theme.text_muted),
        ];

        for (li, (line, color)) in help_lines.iter().enumerate() {
            let ly = py + 2 + li as u16;
            if ly >= py + ph - 1 {
                break;
            }
            cmds.push(RenderCmd::Text {
                x: hx,
                y: ly,
                text: format!("{:<width$}", line, width = hw as usize),
                fg: Some(*color),
                bg: None,
                bold: false,
            });
        }
    }

    cmds
}
