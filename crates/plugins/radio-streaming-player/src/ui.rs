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

    const GAP: u16 = 0;

    let info_h = state.info_h();

    let stations_h = area_h.saturating_sub(GAP + info_h);

    // ---- Stations panel (top, fills remaining height) ----
    ui::draw_panel(&mut cmds, theme, 0, 0, area_w, stations_h, "Stations");

    let inner_w = area_w.saturating_sub(3) as usize;

    // ---- Top line: search bar, scan message, or total station count ----
    if state.search_mode {
        let cursor = if state.tick_counter % 6 < 3 {
            '█'
        } else {
            ' '
        };
        let left_text = format!("Search: {}{cursor}", state.query);
        let right_text = format!("{}/{}", state.filtered.len(), state.stations.len());
        let right_len = right_text.len();
        let max_left = inner_w.saturating_sub(right_len + 1);
        let display_left: String = left_text.chars().take(max_left).collect();
        let right_x = area_w.saturating_sub(2u16.saturating_add(right_text.len() as u16));
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: display_left,
            fg: Some(theme.accent),
            bg: None,
            bold: false,
        });
        cmds.push(RenderCmd::Text {
            x: right_x,
            y: 1,
            text: right_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
    } else {
        let top_text = match state.scan_msg {
            Some(ref msg) => {
                let max_w = area_w.saturating_sub(3) as usize;
                if msg.len() > max_w {
                    format!("{}…", &msg[..max_w.saturating_sub(1)])
                } else {
                    msg.clone()
                }
            }
            None => format!("Total stations: {}", state.stations.len()),
        };
        let top_x = area_w.saturating_sub(2u16.saturating_add(top_text.len() as u16));
        cmds.push(RenderCmd::Text {
            x: top_x,
            y: 1,
            text: top_text,
            fg: if state.scan_msg.is_some() {
                Some(theme.accent)
            } else {
                Some(theme.text_muted)
            },
            bg: None,
            bold: false,
        });
    }

    let table_top = 2u16;
    let header_h = 1u16;
    let table_avail = stations_h.saturating_sub(table_top + header_h + 1);
    let max_visible = table_avail as usize;

    let scroll = state.scroll.min(state.filtered.len().saturating_sub(1));
    let visible_count = max_visible.min(state.filtered.len().saturating_sub(scroll));

    let name_w = (inner_w * 3 / 4).max(10);
    let country_w = inner_w.saturating_sub(name_w);

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(visible_count);
    for i in 0..visible_count {
        let station_idx = state.filtered[scroll + i];
        let station = &state.stations[station_idx];
        rows.push(vec![
            ui::truncate(&station.name, name_w),
            ui::truncate(station.country_name(), country_w),
        ]);
    }

    let vis_selected = if state.selected >= scroll && state.selected < scroll + visible_count {
        Some(state.selected - scroll)
    } else {
        None
    };

    let current_row = state.current_station.and_then(|cur| {
        state.filtered[scroll..scroll + visible_count]
            .iter()
            .position(|&idx| idx == cur)
    });

    cmds.push(RenderCmd::Table {
        x: 2,
        y: table_top,
        w: inner_w as u16,
        h: (visible_count + 1).max(1) as u16,
        header: vec!["Name".into(), "Country".into()],
        header_style: TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
        },
        rows,
        column_widths: vec![name_w as u16, country_w as u16],
        selected: vis_selected,
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
        },
        current_row,
        current_style: Some(TextStyle {
            fg: Some(theme.success),
            bg: None,
            bold: false,
        }),
    });

    // ---- Now Playing panel (bottom) ----
    let np_y = stations_h + GAP;
    ui::draw_panel(&mut cmds, theme, 0, np_y, area_w, info_h, "Now Playing");

    let r_inner_w = area_w.saturating_sub(3);

    match &state.play_state {
        PlayState::Stopped => {
            ui::text_at(
                &mut cmds,
                2,
                np_y + 1,
                "No station selected",
                theme.text_muted,
                None,
                r_inner_w,
            );
        }
        PlayState::Playing(station_name) => {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: np_y + 1,
                text: ui::truncate(station_name, r_inner_w as usize),
                fg: Some(theme.success),
                bg: None,
                bold: true,
            });
            if state.song_title.is_empty() {
                ui::text_at(
                    &mut cmds,
                    2,
                    np_y + 2,
                    "(no metadata)",
                    theme.text_muted,
                    None,
                    r_inner_w,
                );
            } else {
                ui::text_at(
                    &mut cmds,
                    2,
                    np_y + 2,
                    &state.song_title,
                    theme.text,
                    None,
                    r_inner_w,
                );
                if let Some(ref info) = state.track_info {
                    if let Some(ref artist) = info.artist {
                        ui::text_at(
                            &mut cmds,
                            2,
                            np_y + 3,
                            artist,
                            theme.text_muted,
                            None,
                            r_inner_w,
                        );
                    }
                }
            }
        }
        PlayState::Error(e) => {
            ui::text_at(
                &mut cmds,
                2,
                np_y + 1,
                "⚠ Error",
                theme.error,
                None,
                r_inner_w,
            );
            ui::text_at(&mut cmds, 2, np_y + 2, e, theme.error, None, r_inner_w);
        }
    }

    cmds
}
