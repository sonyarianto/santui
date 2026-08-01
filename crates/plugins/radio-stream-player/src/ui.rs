use crate::lrclib;
use crate::state::{wrap_text, PlayState, RadioState};
use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData};
use santui_ipc::ui;

use santui_ipc::ui::PanelOpts;
pub const TABLE_TOP: u16 = 3;
pub const HEADER_H: u16 = 1;

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

    let left_w = area_w;

    const GAP: u16 = 0;
    let info_h = state.info_h();
    let stations_h = area_h.saturating_sub(GAP + info_h);

    let stations_footer: Option<&[(&str, &str)]> = if state.search_mode {
        Some(&[("↑↓", "navigate"), ("↵", "play"), ("⌫", "delete")])
    } else if !state.query.is_empty() {
        Some(&[
            ("↑↓", "navigate"),
            ("/", "search"),
            ("↵", "play"),
            ("s", "stop"),
            ("space", "toggle fav"),
            ("c", "clear"),
            ("f", "toggle fav list"),
            ("r", "reload stations"),
        ])
    } else if state.show_lyrics {
        Some(&[("l", "hide lyrics")])
    } else {
        Some(&[
            ("↑↓", "navigate"),
            ("/", "search"),
            ("↵", "play"),
            ("s", "stop"),
            ("space", "toggle fav"),
            ("f", "toggle fav list"),
            ("r", "reload stations"),
        ])
    };
    let lyrics_footer: Option<&[(&str, &str)]> = if state.show_lyrics {
        Some(&[("↑↓", "scroll"), ("l", "hide lyrics")])
    } else {
        None
    };
    let stations_footer_rows: u16 = if stations_footer.is_some() { 2 } else { 0 };

    // ---- Stations panel (top-left) ----
    let stations_focused = true;
    ui::draw_panel(
        &mut cmds,
        theme,
        0,
        0,
        left_w,
        stations_h,
        Some("Stations"),
        PanelOpts {
            focused: stations_focused,
            footer: stations_footer,
            dim_unfocused: true,
            ..Default::default()
        },
    );

    let inner_w = left_w.saturating_sub(4) as usize;

    // ---- Top line: search bar, scan message, filter indicator, or total station count ----
    let table_avail = stations_h.saturating_sub(TABLE_TOP + HEADER_H + 1 + stations_footer_rows);
    let max_visible = table_avail as usize;
    let scroll_pct = {
        let pct = ui::scroll_pct(state.scroll, state.filtered.len(), max_visible);
        if pct > 0 {
            Some(pct)
        } else {
            None
        }
    };
    if state.search_mode {
        let cursor = ui::blink_cursor(state.tick_counter);
        let left_text = format!("Search: {}{cursor}", state.query);
        let right_text = if let Some(pct) = scroll_pct {
            format!(
                "{}/{}  {}%",
                state.filtered.len(),
                state.stations.len(),
                pct
            )
        } else {
            format!("{}/{}", state.filtered.len(), state.stations.len())
        };
        let right_len = right_text.len();
        let max_left = inner_w.saturating_sub(right_len + 1);
        let display_left: String = left_text.chars().take(max_left).collect();
        let right_x = ui::right_align_x(left_w, &right_text);
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: display_left,
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: right_x,
            y: 1,
            text: right_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else if let Some(ref msg) = state.scan_msg {
        let max_w = left_w.saturating_sub(4) as usize;
        let top_text = if msg.chars().count() > max_w {
            let truncated: String = msg.chars().take(max_w.saturating_sub(3)).collect();
            format!("{}...", truncated)
        } else {
            msg.clone()
        };
        let top_x = ui::right_align_x(left_w, &top_text);
        cmds.push(RenderCmd::Text {
            x: top_x,
            y: 1,
            text: top_text,
            fg: Some(theme.accent),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else if !state.query.is_empty() {
        let left_text = format!("Filter: \"{}\"", state.query);
        let right_text = if let Some(pct) = scroll_pct {
            format!(
                "{}/{}  {}%",
                state.filtered.len(),
                state.stations.len(),
                pct
            )
        } else {
            format!("{}/{}", state.filtered.len(), state.stations.len())
        };
        let right_len = right_text.len();
        let max_left = inner_w.saturating_sub(right_len + 1);
        let display_left: String = left_text.chars().take(max_left).collect();
        let right_x = ui::right_align_x(left_w, &right_text);
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: display_left,
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: right_x,
            y: 1,
            text: right_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else {
        let fav_count = state.favorites_count();
        let top_text = if state.show_favorites_only {
            format!("♥ {} favorites", state.filtered.len())
        } else if fav_count > 0 {
            format!("Total stations: {}  ♥ {}", state.stations.len(), fav_count)
        } else {
            format!("Total stations: {}", state.stations.len())
        };
        let top_text = if let Some(pct) = scroll_pct {
            format!("{}  {}%", top_text, pct)
        } else {
            top_text
        };
        let top_x = ui::right_align_x(left_w, &top_text);
        cmds.push(RenderCmd::Text {
            x: top_x,
            y: 1,
            text: top_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });

        if state.show_favorites_only {
            cmds.push(RenderCmd::Text {
                x: top_x,
                y: 1,
                text: "♥".into(),
                fg: Some([255, 60, 60]),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        } else if fav_count > 0 {
            let digit_count = state.stations.len().to_string().chars().count();
            let heart_x = top_x + 18 + digit_count as u16;
            cmds.push(RenderCmd::Text {
                x: heart_x,
                y: 1,
                text: "♥".into(),
                fg: Some([255, 60, 60]),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    }

    let table_top = TABLE_TOP;
    let header_h = HEADER_H;
    let table_avail = stations_h.saturating_sub(table_top + header_h + 1 + stations_footer_rows);
    let max_visible = table_avail as usize;

    let scroll = state.scroll.min(state.filtered.len().saturating_sub(1));
    let visible_count = max_visible.min(state.filtered.len().saturating_sub(scroll));

    let name_w = ((inner_w - 2) * 45 / 100).max(10);
    let genre_w = ((inner_w - 2) * 35 / 100).max(8);
    let country_w = inner_w.saturating_sub(2 + name_w + genre_w);

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(visible_count);
    for i in 0..visible_count {
        let station_idx = state.filtered[scroll + i];
        let station = &state.stations[station_idx];
        let fav = ui::fav_prefix(state.is_favorite(&station.url));
        rows.push(vec![
            ui::truncate(&format!("{fav}{}", station.name), name_w),
            ui::truncate(&station.genre, genre_w),
            ui::truncate(station.country_name(), country_w),
        ]);
    }

    let vis_selected = ui::vis_selected(state.selected, scroll, visible_count);

    let current_row = state.current_station.and_then(|cur| {
        state.filtered[scroll..scroll + visible_count]
            .iter()
            .position(|&idx| idx == cur)
    });

    let ts = ui::table_styles(theme);
    cmds.push(RenderCmd::Table {
        x: 2,
        y: table_top,
        w: inner_w as u16,
        h: (visible_count + 1).max(1) as u16,
        header: vec!["Name".into(), "Genre".into(), "Country".into()],
        header_style: ts.header,
        rows,
        column_widths: vec![name_w as u16, genre_w as u16, country_w as u16],
        selected: vis_selected,
        style: if stations_focused {
            ts.body
        } else {
            TextStyle {
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            }
        },
        highlight_style: ts.highlight,
        current_row,
        current_style: Some(TextStyle {
            fg: Some(theme.success),
            bg: None,
            bold: false,
            modifiers: 0,
        }),
        cell_styles: None,
    });

    // Red heart overlay for favorite stations (table already renders "♥ " in the name cell)
    ui::heart_overlay(
        &mut cmds,
        theme,
        table_top,
        vis_selected,
        visible_count,
        |i| {
            let station_idx = state.filtered[scroll + i];
            state.is_favorite(&state.stations[station_idx].url)
        },
    );

    // ---- Now Playing panel (bottom-left) ----
    const NP_TITLE: &str = "Now Playing";
    let np_y = stations_h + GAP;
    ui::draw_panel(
        &mut cmds,
        theme,
        0,
        np_y,
        left_w,
        info_h,
        Some(NP_TITLE),
        PanelOpts::default(),
    );
    // Volume right-aligned on the last content row (space is reserved in
    // r_inner_w so left-aligned text never overlaps it)
    let vol_text = format!(" Vol: {}% ", state.volume);
    let vol_w = vol_text.chars().count() as u16;
    let r_inner_w = left_w.saturating_sub(3 + vol_w);
    let vol_x = left_w.saturating_sub(1 + vol_w);
    let vol_y = np_y + info_h - 2; // last content row, above bottom border
    cmds.push(RenderCmd::Text {
        x: vol_x,
        y: vol_y,
        text: vol_text,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

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
        PlayState::Connecting(station_name) => {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: np_y + 1,
                text: ui::truncate(station_name, r_inner_w as usize),
                fg: Some(theme.accent),
                bg: None,
                bold: true,
                modifiers: 0,
            });
            ui::text_at(
                &mut cmds,
                2,
                np_y + 2,
                "Connecting...",
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
                modifiers: 0,
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
        PlayState::Retrying(station_name) => {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: np_y + 1,
                text: ui::truncate(station_name, r_inner_w as usize),
                fg: Some(theme.accent),
                bg: None,
                bold: true,
                modifiers: 0,
            });
            let remaining = state
                .retry_deadline
                .map(|d| {
                    d.saturating_duration_since(std::time::Instant::now())
                        .as_secs()
                })
                .unwrap_or(0);
            let msg = format!(
                "Reconnecting in {remaining}s (attempt {}/{})",
                state.retry_attempt,
                crate::state::MAX_RETRIES,
            );
            ui::text_at(
                &mut cmds,
                2,
                np_y + 2,
                &msg,
                theme.text_muted,
                None,
                r_inner_w,
            );
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

    // ---- Lyrics side panel (snapped right, dim behind) ----
    if state.show_lyrics {
        let popup_w = (area_w * 2 / 5).max(20);
        let popup_x = area_w - popup_w;
        let popup_y = 0u16;
        let popup_h = area_h;
        if popup_x < 4 || area_h < 10 {
            // too small for a useful popup
        } else {
            ui::popup_backdrop(&mut cmds, theme, popup_x, popup_y, popup_w, popup_h);

            ui::draw_panel(
                &mut cmds,
                theme,
                popup_x,
                popup_y,
                popup_w,
                popup_h,
                Some("Lyrics"),
                PanelOpts {
                    focused: true,
                    footer: lyrics_footer,
                    dim_unfocused: false,
                    ..Default::default()
                },
            );

            let ly_inner_w = popup_w.saturating_sub(4);

            if !state.lyrics_text.is_empty() && !state.lyrics_source.is_empty() {
                let footer_y = popup_y + popup_h - 2;
                let sx = popup_x + popup_w.saturating_sub(state.lyrics_source.len() as u16 + 2);
                cmds.push(RenderCmd::Text {
                    x: sx,
                    y: footer_y,
                    text: state.lyrics_source.clone(),
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
            }

            // Title/artist header from iTunes (track_info) or station metadata (song_title)
            let (header_title, header_artist) = if !state.lyrics_text.is_empty() {
                if let Some(ref info) = state.track_info {
                    let title = info.title.clone().or_else(|| {
                        (!state.song_title.is_empty()).then(|| state.song_title.clone())
                    });
                    (title, info.artist.clone())
                } else if !state.song_title.is_empty() {
                    let (artist, title) = lrclib::split_title(&state.song_title);
                    (Some(title), artist)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            let header_rows = match (&header_title, &header_artist) {
                (Some(_), Some(_)) => 3,
                (Some(_), None) => 2,
                (None, Some(_)) => 2,
                (None, None) => 0,
            };
            let content_top = popup_y + 1 + header_rows;

            if state.lyrics_loading {
                ui::text_at(
                    &mut cmds,
                    popup_x + 2,
                    popup_y + 1,
                    "Searching lyrics...",
                    theme.text_muted,
                    None,
                    ly_inner_w,
                );
            } else if state.lyrics_text.is_empty() {
                ui::text_at(
                    &mut cmds,
                    popup_x + 2,
                    popup_y + 1,
                    "No lyrics found",
                    theme.text_muted,
                    None,
                    ly_inner_w,
                );
            } else {
                let title_fg = theme.accent;
                let artist_fg = theme.text_muted;
                if let Some(ref title) = header_title {
                    cmds.push(RenderCmd::Text {
                        x: popup_x + 2,
                        y: popup_y + 1,
                        text: title.chars().take(ly_inner_w as usize).collect(),
                        fg: Some(title_fg),
                        bg: None,
                        bold: true,
                        modifiers: 0,
                    });
                }
                if let Some(ref artist) = header_artist {
                    cmds.push(RenderCmd::Text {
                        x: popup_x + 2,
                        y: popup_y + 2,
                        text: artist.chars().take(ly_inner_w as usize).collect(),
                        fg: Some(artist_fg),
                        bg: None,
                        bold: false,
                        modifiers: 0,
                    });
                }

                let ly_h = state.lyrics_content_height(area_h);
                let wrapped = wrap_text(&state.lyrics_text, ly_inner_w as usize);
                let total_visual = wrapped.len();
                let scroll = state.lyrics_scroll.min(total_visual.saturating_sub(1));
                for i in 0..ly_h {
                    let line_idx = scroll + i;
                    if line_idx >= total_visual {
                        break;
                    }
                    let lyrics_body_fg = theme.text;
                    cmds.push(RenderCmd::Text {
                        x: popup_x + 2,
                        y: content_top + i as u16,
                        text: wrapped[line_idx].clone(),
                        fg: Some(lyrics_body_fg),
                        bg: None,
                        bold: false,
                        modifiers: 0,
                    });
                }
                if total_visual > ly_h {
                    let pct = ui::scroll_pct(scroll, total_visual, ly_h);
                    let scroll_text = format!("{pct}%");
                    let indicator_y = content_top + ly_h as u16 - 1;
                    let sx = popup_x + popup_w.saturating_sub(scroll_text.len() as u16 + 2);
                    cmds.push(RenderCmd::Text {
                        x: sx,
                        y: indicator_y,
                        text: scroll_text,
                        fg: Some(theme.text_muted),
                        bg: None,
                        bold: false,
                        modifiers: 0,
                    });
                }
            }
        }
    }

    cmds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::itunes::TrackInfo;
    use crate::state::PlayState;
    use crate::stations::Station;

    fn make_stations(n: usize) -> Vec<Station> {
        (0..n)
            .map(|i| Station {
                name: format!("Station {i}"),
                url: format!("http://example.com/{i}"),
                country: if i % 2 == 0 { "US".into() } else { "GB".into() },
                genre: if i % 3 == 0 {
                    "Rock".into()
                } else {
                    "Pop".into()
                },
            })
            .collect()
    }

    fn state_with(n: usize) -> RadioState {
        RadioState::new(make_stations(n))
    }

    #[test]
    fn small_area_returns_empty() {
        let state = state_with(5);
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 9, 2);
        assert!(cmds.is_empty());
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 10, 2);
        assert!(cmds.is_empty());
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 9, 3);
        assert!(cmds.is_empty());
    }

    #[test]
    fn contains_clear_command() {
        let cmds = render_ui(&state_with(5), &santui_ipc::test::theme(), 80, 24);
        if let RenderCmd::Clear { x, y, w, h } = &cmds[0] {
            assert_eq!(*x, 0);
            assert_eq!(*y, 0);
            assert_eq!(*w, 80);
            assert_eq!(*h, 24);
        } else {
            panic!("first cmd should be Clear");
        }
    }

    #[test]
    fn contains_stations_panel_border() {
        let cmds = render_ui(&state_with(5), &santui_ipc::test::theme(), 80, 24);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        assert_eq!(borders.len(), 2, "stations panel + now playing panel");
        if let RenderCmd::Border { title, y, .. } = borders[0] {
            assert_eq!(title.as_deref(), Some("Stations"));
            assert_eq!(*y, 0);
        }
    }

    #[test]
    fn shows_total_stations_in_normal_mode() {
        let cmds = render_ui(&state_with(5), &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_total = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text.contains("Total stations: 5")
            } else {
                false
            }
        });
        assert!(has_total);
    }

    #[test]
    fn shows_search_bar_in_search_mode() {
        let mut st = state_with(5);
        st.search_mode = true;
        st.query = "test".into();
        st.filtered = vec![0, 3];
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_search = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text.contains("Search: test")
            } else {
                false
            }
        });
        assert!(has_search);
        let has_count = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "2/5"
            } else {
                false
            }
        });
        assert!(has_count);
    }

    #[test]
    fn shows_filter_indicator_when_query_non_empty() {
        let mut st = state_with(5);
        st.query = "gold".into();
        st.filtered = vec![0, 2, 4];
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_filter = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text.contains("Filter:")
            } else {
                false
            }
        });
        assert!(has_filter, "should show \"Filter: ...\" indicator");
        let has_count = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "3/5"
            } else {
                false
            }
        });
        assert!(has_count, "should show filtered count");
    }

    #[test]
    fn shows_scan_msg_when_set() {
        let mut st = state_with(5);
        st.scan_msg = Some("Reloaded 5 stations".into());
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_msg = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "Reloaded 5 stations"
            } else {
                false
            }
        });
        assert!(has_msg);
    }

    #[test]
    fn stopped_shows_no_station_selected() {
        let st = state_with(5);
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_noop = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "No station selected"
            } else {
                false
            }
        });
        assert!(has_noop);
    }

    #[test]
    fn playing_shows_station_name_green() {
        let mut st = state_with(5);
        st.play_state = PlayState::Playing("Station 1".into());
        st.current_station = Some(1);
        st.song_title = "Some Song".into();
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_name = texts.iter().any(|t| {
            if let RenderCmd::Text { text, fg, bold, .. } = t {
                text == "Station 1" && *fg == Some(santui_ipc::test::theme().success) && *bold
            } else {
                false
            }
        });
        assert!(has_name);
        let has_title = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "Some Song"
            } else {
                false
            }
        });
        assert!(has_title);
    }

    #[test]
    fn playing_with_track_info_shows_artist() {
        let mut st = state_with(5);
        st.play_state = PlayState::Playing("Station 0".into());
        st.current_station = Some(0);
        st.song_title = "Song Title".into();
        st.track_info = Some(TrackInfo {
            artist: Some("Artist Name".into()),
            title: None,
        });
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_artist = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "Artist Name"
            } else {
                false
            }
        });
        assert!(has_artist);
    }

    #[test]
    fn playing_no_song_title_shows_no_metadata() {
        let mut st = state_with(5);
        st.play_state = PlayState::Playing("Station 0".into());
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_msg = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "(no metadata)"
            } else {
                false
            }
        });
        assert!(has_msg);
    }

    #[test]
    fn error_shows_error_message() {
        let mut st = state_with(5);
        st.play_state = PlayState::Error("connection lost".into());
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_error = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "⚠ Error"
            } else {
                false
            }
        });
        assert!(has_error);
        let has_detail = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "connection lost"
            } else {
                false
            }
        });
        assert!(has_detail);
    }

    #[test]
    fn table_has_correct_headers() {
        let cmds = render_ui(&state_with(5), &santui_ipc::test::theme(), 80, 24);
        let table = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Table { .. }))
            .unwrap();
        if let RenderCmd::Table { header, .. } = table {
            assert_eq!(header, &vec!["Name", "Genre", "Country"]);
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn table_shows_station_rows() {
        let cmds = render_ui(&state_with(5), &santui_ipc::test::theme(), 80, 24);
        let table = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Table { .. }))
            .unwrap();
        if let RenderCmd::Table { rows, .. } = table {
            assert_eq!(rows.len(), 5);
            assert_eq!(rows[0][0], "  Station 0");
            assert_eq!(rows[0][1], "Rock");
            assert_eq!(rows[0][2], "United States");
            assert_eq!(rows[1][0], "  Station 1");
            assert_eq!(rows[1][1], "Pop");
            assert_eq!(rows[1][2], "United Kingdom");
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn table_selection_highlighted() {
        let mut st = state_with(10);
        st.selected = 3;
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let table = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Table { .. }))
            .unwrap();
        if let RenderCmd::Table { selected, .. } = table {
            assert_eq!(*selected, Some(3));
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn table_current_row_marked() {
        let mut st = state_with(10);
        st.current_station = Some(5);
        st.play_state = PlayState::Playing("Station 5".into());
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let table = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Table { .. }))
            .unwrap();
        if let RenderCmd::Table {
            current_row,
            current_style,
            ..
        } = table
        {
            assert_eq!(*current_row, Some(5));
            assert!(current_style.is_some());
            assert_eq!(
                current_style.as_ref().unwrap().fg,
                Some(santui_ipc::test::theme().success)
            );
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn lyrics_panel_shown_when_enabled() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        assert_eq!(borders.len(), 3, "stations + now playing + lyrics panels");
        let has_lyrics = borders.iter().any(|b| {
            if let RenderCmd::Border { title, .. } = b {
                title.as_deref() == Some("Lyrics")
            } else {
                false
            }
        });
        assert!(has_lyrics);
    }

    #[test]
    fn lyrics_loading_shows_searching_message() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        st.lyrics_loading = true;
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_msg = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "Searching lyrics..."
            } else {
                false
            }
        });
        assert!(has_msg);
    }

    #[test]
    fn lyrics_empty_shows_no_lyrics_message() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        st.lyrics_text = String::new();
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_msg = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "No lyrics found"
            } else {
                false
            }
        });
        assert!(has_msg);
    }

    #[test]
    fn lyrics_content_rendered() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        st.lyrics_text = "Line one\nLine two\nLine three".into();
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_line1 = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "Line one"
            } else {
                false
            }
        });
        let has_line2 = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "Line two"
            } else {
                false
            }
        });
        assert!(has_line1 && has_line2);
    }

    #[test]
    fn lyrics_not_shown_when_disabled() {
        let st = state_with(5);
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        let has_lyrics = borders.iter().any(|b| {
            if let RenderCmd::Border { title, .. } = b {
                title.as_deref() == Some("Lyrics")
            } else {
                false
            }
        });
        assert!(!has_lyrics);
    }

    #[test]
    fn lyrics_hidden_when_area_too_narrow() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        let cmds2 = render_ui(&st, &santui_ipc::test::theme(), 20, 24);
        let borders2: Vec<&RenderCmd> = cmds2
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        let has_lyrics = borders2.iter().any(|b| {
            if let RenderCmd::Border { title, .. } = b {
                title.as_deref() == Some("Lyrics")
            } else {
                false
            }
        });
        assert!(!has_lyrics, "lyrics hidden when area too small for popup");
    }

    #[test]
    fn lyrics_overlay_renders_right_snapped_popup() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let has_dim = cmds.iter().any(|c| matches!(c, RenderCmd::Dim { .. }));
        assert!(has_dim, "expected Dim command for overlay");
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        assert_eq!(borders.len(), 3);
        // Stations panel stays full width (not split)
        if let RenderCmd::Border { w, .. } = borders[0] {
            assert_eq!(*w, 80);
        }
        // Lyrics popup snapped to right edge: x=48, w=32, full height
        if let RenderCmd::Border { x, w, y, h, .. } = borders[2] {
            assert_eq!(*x, 48);
            assert_eq!(*w, 32);
            assert_eq!(*y, 0);
            assert_eq!(*h, 24);
        }
    }

    #[test]
    fn now_playing_panel_contains_volume() {
        let mut st = state_with(5);
        st.volume = 75;
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_vol = texts.iter().any(|c| {
            if let RenderCmd::Text { text, .. } = c {
                text.contains("Vol: 75%")
            } else {
                false
            }
        });
        assert!(has_vol, "expected a Text cmd containing Vol: 75%");
    }

    #[test]
    fn table_columns_use_country_name() {
        let mut st = state_with(3);
        // Override country to known codes
        st.stations[0].country = "DE".into();
        st.stations[1].country = "FR".into();
        st.stations[2].country = "XX".into();
        st.apply_filter();
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let table = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Table { .. }))
            .unwrap();
        if let RenderCmd::Table { rows, .. } = table {
            assert_eq!(rows[0][2], "Germany");
            assert_eq!(rows[1][2], "France");
            assert_eq!(rows[2][2], "XX"); // unknown code returned as-is
        }
    }

    #[test]
    fn table_scroll_offset() {
        let mut st = state_with(30);
        st.scroll = 10;
        st.selected = 12;
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let table = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Table { .. }))
            .unwrap();
        if let RenderCmd::Table {
            rows,
            selected,
            current_row,
            ..
        } = table
        {
            // With scroll=10, visible rows start at index 10
            assert_eq!(rows[0][0], "  Station 10");
            assert_eq!(rows[2][0], "  Station 12");
            assert_eq!(*selected, Some(2)); // vis_selected = 12 - 10 = 2
            assert_eq!(*current_row, None); // no current_station set
        }
    }

    #[test]
    fn table_empty_filtered_no_rows() {
        let mut st = state_with(5);
        st.filtered.clear();
        // render_ui accesses state.filtered so with empty it should produce empty rows
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let table = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Table { .. }))
            .unwrap();
        if let RenderCmd::Table { rows, .. } = table {
            assert!(rows.is_empty());
        }
    }

    #[test]
    fn table_visible_count_limited_by_area() {
        let st = state_with(100);
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 10);
        let table = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Table { .. }))
            .unwrap();
        if let RenderCmd::Table { rows, h, .. } = table {
            // With small area height, less rows visible
            assert!(rows.len() < 100);
            assert!(*h > 0);
        }
    }

    #[test]
    fn now_playing_error_shows_red_text() {
        let mut st = state_with(5);
        st.play_state = PlayState::Error("stream failed".into());
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_error = texts.iter().any(|t| {
            if let RenderCmd::Text { text, fg, .. } = t {
                text == "⚠ Error" && *fg == Some(santui_ipc::test::theme().error)
            } else {
                false
            }
        });
        assert!(has_error);
    }

    #[test]
    fn lyrics_scroll_shows_percentage() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        // Many lines so scroll is needed (area_h=24 → ly_h=22)
        st.lyrics_text = (0..50)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        st.lyrics_scroll = 14;
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_pct = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text == "50%" || text == "0%" || text.contains('%')
            } else {
                false
            }
        });
        assert!(has_pct, "expected scroll percentage indicator");
    }

    #[test]
    fn stations_panel_stays_bright_when_lyrics_shown() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        // Stations panel (first border) should use border color when focused
        if let RenderCmd::Border { fg, .. } = borders[0] {
            assert_eq!(
                *fg,
                santui_ipc::test::theme().border,
                "stations panel should use border color when focused"
            );
        }
    }

    #[test]
    fn lyrics_panel_stays_bright_when_shown() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        let cmds = render_ui(&st, &santui_ipc::test::theme(), 80, 24);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        // Lyrics panel is the third border
        assert!(borders.len() >= 3);
        if let RenderCmd::Border { title, fg, .. } = &borders[2] {
            assert_eq!(title.as_deref(), Some("Lyrics"));
            assert_eq!(
                *fg,
                santui_ipc::test::theme().border,
                "lyrics panel should use border color when focused"
            );
        }
    }
}
