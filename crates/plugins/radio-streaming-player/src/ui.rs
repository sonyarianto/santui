use crate::state::{PlayState, RadioState};
use santui_ipc::protocol::{RenderCmd, ThemeData};
use santui_ipc::ui;

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

    const GAP: u16 = 1;
    let left_w = (area_w / 3).max(12);
    let right_w = area_w.saturating_sub(left_w + GAP);
    let info_h = (match &state.play_state {
        PlayState::Stopped => 3,
        PlayState::Playing(_) => {
            if state
                .track_info
                .as_ref()
                .and_then(|i| i.artist.as_ref())
                .is_some()
            {
                6
            } else {
                5
            }
        }
        PlayState::Error(_) => 4,
    })
    .max(4)
    .min(area_h.saturating_sub(GAP + 3));

    // ---- Left panel: station list ----
    ui::draw_panel(&mut cmds, theme, 0, 0, left_w, area_h, "Stations");

    let inner_x = 2u16;
    let inner_w = left_w.saturating_sub(3);
    // Reserve bottom rows for scroll indicator + search bar
    let search_extra = if state.search_mode { 2 } else { 0 };
    let max_visible = (area_h.saturating_sub(4 + search_extra)) as usize;

    let scroll = state.scroll.min(state.filtered.len().saturating_sub(1));

    for (vis_idx, &station_idx) in state
        .filtered
        .iter()
        .enumerate()
        .skip(scroll)
        .take(max_visible)
    {
        let item_y = 2u16 + (vis_idx - scroll) as u16;
        let station = &state.stations[station_idx];
        let is_selected = vis_idx == state.selected;
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
            (Some(theme.accent), Some(theme.background_panel), true)
        } else {
            (Some(theme.text), Some(theme.background_panel), false)
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

    // More stations indicator (hidden during search)
    if !state.search_mode {
        let scroll_indicator_y = area_h.saturating_sub(2);
        if scroll + max_visible < state.filtered.len() {
            let more = state.filtered.len() - scroll - max_visible;
            let label = format!("{more} more");
            let max_w = inner_w as usize;
            let display = if label.len() > max_w {
                format!("{}…", more)
            } else {
                label
            };
            cmds.push(RenderCmd::Text {
                x: inner_x,
                y: scroll_indicator_y,
                text: display,
                fg: Some(theme.text_muted),
                bg: Some(theme.background_panel),
                bold: false,
            });
        }
    }

    // ---- Search bar overlay ----
    if state.search_mode {
        let search_y = area_h.saturating_sub(3);
        let bar_w = left_w.saturating_sub(4) as usize;

        // Draw search input line
        let cursor = if state.tick_counter % 6 < 3 {
            '█'
        } else {
            ' '
        };
        let search_text = if state.query.is_empty() {
            format!(" Search: {cursor}")
        } else {
            format!(" Search: {}{cursor}", state.query)
        };
        let display: String = search_text.chars().take(bar_w).collect();
        cmds.push(RenderCmd::Text {
            x: 2,
            y: search_y,
            text: display,
            fg: Some(theme.accent),
            bg: Some(theme.background_panel),
            bold: false,
        });

        // Draw results count
        let count_y = area_h.saturating_sub(2);
        let count_text = format!(" {} of {}", state.filtered.len(), state.stations.len());
        cmds.push(RenderCmd::Text {
            x: 2,
            y: count_y,
            text: count_text,
            fg: Some(theme.text_muted),
            bg: Some(theme.background_panel),
            bold: false,
        });
    }

    // ---- Scan message (temporary overlay in left panel) ----
    if let Some(ref msg) = state.scan_msg {
        let msg_y = area_h.saturating_sub(2);
        let max_w = left_w.saturating_sub(3) as usize;
        let display = if msg.len() > max_w {
            format!("{}…", &msg[..max_w.saturating_sub(1)])
        } else {
            format!("{:<width$}", msg, width = max_w)
        };
        cmds.push(RenderCmd::Text {
            x: 2,
            y: msg_y,
            text: display,
            fg: Some(theme.accent),
            bg: Some(theme.background_panel),
            bold: false,
        });
    }

    // ---- Right panel: Now Playing ----
    ui::draw_panel(
        &mut cmds,
        theme,
        left_w + GAP,
        0,
        right_w,
        info_h,
        "Now Playing",
    );

    let r_inner_x = left_w + GAP + 2;
    let r_inner_w = right_w.saturating_sub(3);

    match &state.play_state {
        PlayState::Stopped => {
            ui::text_at(
                &mut cmds,
                r_inner_x,
                2,
                "No station selected",
                theme.text_muted,
                theme.background_panel,
                r_inner_w,
            );
        }
        PlayState::Playing(station_name) => {
            ui::text_at(
                &mut cmds,
                r_inner_x,
                2,
                station_name,
                theme.success,
                theme.background_panel,
                r_inner_w,
            );

            let title = if state.song_title.is_empty() {
                "(no metadata)"
            } else {
                &state.song_title
            };
            ui::text_at(
                &mut cmds,
                r_inner_x,
                3,
                title,
                theme.text,
                theme.background_panel,
                r_inner_w,
            );

            if let Some(ref info) = state.track_info {
                if let Some(ref artist) = info.artist {
                    ui::text_at(
                        &mut cmds,
                        r_inner_x,
                        4,
                        artist,
                        theme.text_muted,
                        theme.background_panel,
                        r_inner_w,
                    );
                }
            }
        }
        PlayState::Error(e) => {
            ui::text_at(
                &mut cmds,
                r_inner_x,
                2,
                "Error",
                theme.error,
                theme.background_panel,
                r_inner_w,
            );
            ui::text_at(
                &mut cmds,
                r_inner_x,
                3,
                e,
                theme.error,
                theme.background_panel,
                r_inner_w,
            );
        }
    }

    // ---- Right panel bottom: Volume gauge ----
    let gauge_y = info_h + GAP;
    let gauge_h = 4u16;
    if gauge_y + gauge_h <= area_h {
        ui::draw_panel(
            &mut cmds,
            theme,
            left_w + GAP,
            gauge_y,
            right_w,
            gauge_h,
            "Volume",
        );

        let g_inner_x = left_w + GAP + 2;
        let g_inner_w = right_w.saturating_sub(3) as usize;
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
            bg: Some(theme.background_panel),
            bold: false,
        });
    }

    cmds
}
