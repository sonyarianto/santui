use crate::state::{PlayState, RadioState};
use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData};
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

    let volume_h = 4u16;
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
    .max(4);

    let bottom_h = info_h + GAP + volume_h;
    let stations_h = area_h.saturating_sub(bottom_h);

    // ---- Stations panel (top, full width, fills remaining height) ----
    ui::draw_panel(&mut cmds, theme, 0, 0, area_w, stations_h, "Stations");

    let inner_x = 2u16;
    let inner_w = area_w.saturating_sub(3) as usize;
    let search_extra = if state.search_mode { 2 } else { 0 };
    let table_top = 2u16;
    let header_h = 1u16;
    let table_avail = stations_h.saturating_sub(table_top + header_h + search_extra);
    let max_visible = table_avail as usize;

    let scroll = state.scroll.min(state.filtered.len().saturating_sub(1));
    let visible_count = max_visible.min(state.filtered.len().saturating_sub(scroll));

    // Column widths
    let name_w = (inner_w * 3 / 4).max(10);
    let country_w = inner_w.saturating_sub(name_w);

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(visible_count);
    for i in 0..visible_count {
        let station_idx = state.filtered[scroll + i];
        let station = &state.stations[station_idx];
        let is_current = state.current_station == Some(station_idx);
        let name = if is_current {
            ui::truncate(&format!("♪ {}", station.name), name_w)
        } else {
            ui::truncate(&station.name, name_w)
        };
        rows.push(vec![name, ui::truncate(&station.country, country_w)]);
    }

    let vis_selected = if state.selected >= scroll && state.selected < scroll + visible_count {
        Some(state.selected - scroll)
    } else {
        None
    };

    cmds.push(RenderCmd::Table {
        x: inner_x,
        y: table_top,
        w: inner_w as u16,
        h: (visible_count + 1).max(1) as u16,
        header: vec!["Name".into(), "Country".into()],
        header_style: TextStyle {
            fg: Some(theme.text_muted),
            bg: Some(theme.background_panel),
            bold: true,
        },
        rows,
        column_widths: vec![name_w as u16, country_w as u16],
        selected: vis_selected,
        style: TextStyle {
            fg: Some(theme.text),
            bg: Some(theme.background_panel),
            bold: false,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
        },
    });

    // ---- Search bar overlay ----
    if state.search_mode {
        let search_y = stations_h.saturating_sub(3);
        let bar_w = area_w.saturating_sub(4) as usize;

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
        let count_y = stations_h.saturating_sub(2);
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
        let msg_y = stations_h.saturating_sub(2);
        let max_w = area_w.saturating_sub(3) as usize;
        let display = if msg.len() > max_w {
            format!("{}…", &msg[..max_w.saturating_sub(1)])
        } else {
            msg.to_string()
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

    // ---- Now Playing panel (full width, below Stations) ----
    let np_y = stations_h + GAP;
    ui::draw_panel(&mut cmds, theme, 0, np_y, area_w, info_h, "Now Playing");

    let r_inner_x = 2u16;
    let r_inner_w = area_w.saturating_sub(3);

    match &state.play_state {
        PlayState::Stopped => {
            ui::text_at(
                &mut cmds,
                r_inner_x,
                np_y + 2,
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
                np_y + 2,
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
                np_y + 3,
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
                        np_y + 4,
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
                np_y + 2,
                "Error",
                theme.error,
                theme.background_panel,
                r_inner_w,
            );
            ui::text_at(
                &mut cmds,
                r_inner_x,
                np_y + 3,
                e,
                theme.error,
                theme.background_panel,
                r_inner_w,
            );
        }
    }

    // ---- Volume panel (full width, bottom) ----
    let vol_y = np_y + info_h + GAP;
    if vol_y + volume_h <= area_h {
        ui::draw_panel(&mut cmds, theme, 0, vol_y, area_w, volume_h, "Volume");

        let g_inner_x = 2u16;
        let g_inner_w = area_w.saturating_sub(3) as usize;
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
            y: vol_y + 2,
            text: gauge_text,
            fg: Some(theme.success),
            bg: Some(theme.background_panel),
            bold: false,
        });
    }

    cmds
}
