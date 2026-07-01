use crate::lrclib;
use crate::state::{PlayState, RadioState};
use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};
use santui_ipc::ui;

pub const TABLE_TOP: u16 = 3;
pub const HEADER_H: u16 = 1;

#[allow(clippy::too_many_arguments)]
fn draw_panel(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    title: &str,
    focused: bool,
    footer: Option<&[(&str, &str)]>,
) {
    if w < 3 || h < 2 {
        return;
    }
    if focused {
        cmds.push(RenderCmd::Border {
            x,
            y,
            w,
            h,
            fg: theme.accent,
            bg: None,
            borders: BORDER_ALL,
            title: Some(title.trim().into()),
            title_fg: Some(theme.text),
            title_dash_fg: Some(theme.accent),
        });
    } else {
        ui::draw_panel(cmds, theme, x, y, w, h, title);
    }

    if let Some(hints) = footer {
        let max_chars = w.saturating_sub(3) as usize;
        let mut cx = x + 2;
        let footer_y = y + h - 2;
        let mut remaining = max_chars;
        for (i, (key, desc)) in hints.iter().enumerate() {
            if remaining == 0 {
                break;
            }
            if i > 0 {
                const SEP: &str = " • ";
                let sep_w = SEP.chars().count();
                if sep_w <= remaining {
                    cmds.push(RenderCmd::Text {
                        x: cx,
                        y: footer_y,
                        text: SEP.into(),
                        fg: Some(theme.text_muted),
                        bg: None,
                        bold: false,
                    });
                    cx += sep_w as u16;
                    remaining -= sep_w;
                }
            }
            if remaining == 0 {
                break;
            }
            let k: String = key.chars().take(remaining).collect();
            if !k.is_empty() {
                let kw = k.chars().count();
                cmds.push(RenderCmd::Text {
                    x: cx,
                    y: footer_y,
                    text: k,
                    fg: Some(theme.text),
                    bg: None,
                    bold: false,
                });
                cx += kw as u16;
                remaining -= kw;
            }
            if remaining == 0 {
                break;
            }
            if !desc.is_empty() {
                let desc_w = desc.chars().count();
                let space_needed = 1 + desc_w;
                if space_needed <= remaining {
                    let d: String = desc.chars().take(remaining - 1).collect();
                    let dw = d.chars().count();
                    let display = format!(" {d}");
                    cmds.push(RenderCmd::Text {
                        x: cx,
                        y: footer_y,
                        text: display,
                        fg: Some(theme.text_muted),
                        bg: None,
                        bold: false,
                    });
                    cx += (1 + dw) as u16;
                    remaining -= 1 + dw;
                }
            }
        }
    }
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

    let left_w = if state.show_lyrics {
        (area_w * 3 / 5).max(20)
    } else {
        area_w
    };
    let right_w = area_w.saturating_sub(left_w);

    const GAP: u16 = 0;
    let info_h = state.info_h();
    let stations_h = area_h.saturating_sub(GAP + info_h);

    let stations_footer: Option<&[(&str, &str)]> = if state.search_mode {
        Some(&[("↑↓", "navigate"), ("↵", "play"), ("⌫", "delete")])
    } else if !state.query.is_empty() {
        Some(&[
            ("↑↓", "navigate"),
            ("↵", "play"),
            ("space", "fav"),
            ("c", "clear"),
            ("/", "search"),
            ("s", "stop"),
            ("f", "fav only"),
            ("r", "reload"),
        ])
    } else if state.lyrics_focused {
        Some(&[("↑↓", "scroll"), ("l", "hide")])
    } else {
        Some(&[
            ("↑↓", "navigate"),
            ("↵", "play"),
            ("space", "fav"),
            ("/", "search"),
            ("s", "stop"),
            ("f", "fav only"),
            ("r", "reload"),
        ])
    };
    let lyrics_footer: Option<&[(&str, &str)]> = if state.show_lyrics {
        Some(&[("↑↓", "scroll"), ("l", "hide")])
    } else {
        None
    };
    let stations_footer_rows: u16 = if stations_footer.is_some() { 2 } else { 0 };

    // ---- Stations panel (top-left) ----
    let stations_focused = state.show_lyrics && !state.lyrics_focused;
    draw_panel(
        &mut cmds,
        theme,
        0,
        0,
        left_w,
        stations_h,
        "Stations",
        stations_focused,
        stations_footer,
    );

    let inner_w = left_w.saturating_sub(3) as usize;

    // ---- Top line: search bar, scan message, filter indicator, or total station count ----
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
        let right_x = left_w.saturating_sub(2u16.saturating_add(right_text.len() as u16));
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
    } else if let Some(ref msg) = state.scan_msg {
        let max_w = left_w.saturating_sub(3) as usize;
        let top_text = if msg.len() > max_w {
            format!("{}…", &msg[..max_w.saturating_sub(1)])
        } else {
            msg.clone()
        };
        let top_x = left_w.saturating_sub(2u16.saturating_add(top_text.len() as u16));
        cmds.push(RenderCmd::Text {
            x: top_x,
            y: 1,
            text: top_text,
            fg: Some(theme.accent),
            bg: None,
            bold: false,
        });
    } else if !state.query.is_empty() {
        let left_text = format!("Filter: \"{}\"", state.query);
        let right_text = format!("{}/{}", state.filtered.len(), state.stations.len());
        let right_len = right_text.len();
        let max_left = inner_w.saturating_sub(right_len + 1);
        let display_left: String = left_text.chars().take(max_left).collect();
        let right_x = left_w.saturating_sub(2u16.saturating_add(right_text.len() as u16));
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
        let top_text = {
            let fav_count = state.favorites_count();
            if state.show_favorites_only {
                format!("♥ {} favorites", state.filtered.len())
            } else if fav_count > 0 {
                format!("Total stations: {}  ♥ {}", state.stations.len(), fav_count)
            } else {
                format!("Total stations: {}", state.stations.len())
            }
        };
        let top_x = left_w.saturating_sub(2u16.saturating_add(top_text.len() as u16));
        cmds.push(RenderCmd::Text {
            x: top_x,
            y: 1,
            text: top_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
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
        let fav = if state.is_favorite(&station.url) {
            "♥ "
        } else {
            "  "
        };
        rows.push(vec![
            ui::truncate(&format!("{fav}{}", station.name), name_w),
            ui::truncate(&station.genre, genre_w),
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
        header: vec!["Name".into(), "Genre".into(), "Country".into()],
        header_style: TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
        },
        rows,
        column_widths: vec![name_w as u16, genre_w as u16, country_w as u16],
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

    // ---- Now Playing panel (bottom-left) ----
    let np_y = stations_h + GAP;
    draw_panel(
        &mut cmds,
        theme,
        0,
        np_y,
        left_w,
        info_h,
        &format!("Now Playing │ Vol: {}%", state.volume),
        false,
        None,
    );

    let r_inner_w = left_w.saturating_sub(3);

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

    // ---- Lyrics panel (right side) ----
    if state.show_lyrics && right_w >= 15 {
        let ly_x = left_w;
        let ly_panel_w = right_w;
        draw_panel(
            &mut cmds,
            theme,
            ly_x,
            0,
            ly_panel_w,
            area_h,
            "Lyrics",
            state.lyrics_focused,
            lyrics_footer,
        );

        let ly_inner_w = ly_panel_w.saturating_sub(3);

        // Title/artist header from iTunes (track_info) or station metadata (song_title)
        let (header_title, header_artist) = if !state.lyrics_text.is_empty() {
            if let Some(ref info) = state.track_info {
                let title = info
                    .title
                    .clone()
                    .or_else(|| (!state.song_title.is_empty()).then(|| state.song_title.clone()));
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
        let content_top = 1 + header_rows;

        if state.lyrics_loading {
            ui::text_at(
                &mut cmds,
                ly_x + 2,
                1,
                "Searching lyrics...",
                theme.text_muted,
                None,
                ly_inner_w,
            );
        } else if state.lyrics_text.is_empty() {
            ui::text_at(
                &mut cmds,
                ly_x + 2,
                1,
                "No lyrics found",
                theme.text_muted,
                None,
                ly_inner_w,
            );
        } else {
            // Render title header
            if let Some(ref title) = header_title {
                cmds.push(RenderCmd::Text {
                    x: ly_x + 2,
                    y: 1,
                    text: title.chars().take(ly_inner_w as usize).collect(),
                    fg: Some(theme.accent),
                    bg: None,
                    bold: true,
                });
            }
            if let Some(ref artist) = header_artist {
                cmds.push(RenderCmd::Text {
                    x: ly_x + 2,
                    y: 2,
                    text: artist.chars().take(ly_inner_w as usize).collect(),
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                });
            }
            // Blank line at y=3 (both) or y=2 (title only) is implicit

            let ly_h = state.lyrics_content_height(area_h);
            let lines: Vec<&str> = state.lyrics_text.lines().collect();
            let scroll = state.lyrics_scroll.min(lines.len().saturating_sub(1));
            for i in 0..ly_h {
                let line_idx = scroll + i;
                if line_idx >= lines.len() {
                    break;
                }
                let line = lines[line_idx];
                ui::text_at(
                    &mut cmds,
                    ly_x + 2,
                    content_top + i as u16,
                    line,
                    theme.text,
                    None,
                    ly_inner_w,
                );
            }
            if lines.len() > ly_h {
                let total = lines.len();
                let max_scroll = total.saturating_sub(ly_h);
                let pct = (scroll * 100)
                    .checked_div(max_scroll)
                    .map(|v| v.min(100))
                    .unwrap_or(0);
                let scroll_text = format!("{pct}%");
                let indicator_y = content_top + ly_h as u16 - 1;
                let sx = ly_x + ly_panel_w.saturating_sub(scroll_text.len() as u16 + 2);
                cmds.push(RenderCmd::Text {
                    x: sx,
                    y: indicator_y,
                    text: scroll_text,
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                });
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

    fn default_theme() -> ThemeData {
        ThemeData {
            text: [220, 220, 220],
            text_muted: [140, 140, 140],
            accent: [157, 124, 216],
            highlight: [250, 178, 131],
            logo: [255, 185, 0],
            background: [20, 20, 20],
            background_panel: [20, 20, 20],
            background_overlay: [10, 10, 10],
            border: [250, 178, 131],
            success: [127, 216, 143],
            error: [224, 108, 117],
            inverted_text: [20, 20, 20],
        }
    }

    fn state_with(n: usize) -> RadioState {
        RadioState::new(make_stations(n))
    }

    #[test]
    fn small_area_returns_empty() {
        let state = state_with(5);
        let cmds = render_ui(&state, &default_theme(), 9, 2);
        assert!(cmds.is_empty());
        let cmds = render_ui(&state, &default_theme(), 10, 2);
        assert!(cmds.is_empty());
        let cmds = render_ui(&state, &default_theme(), 9, 3);
        assert!(cmds.is_empty());
    }

    #[test]
    fn contains_clear_command() {
        let cmds = render_ui(&state_with(5), &default_theme(), 80, 24);
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
        let cmds = render_ui(&state_with(5), &default_theme(), 80, 24);
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
        let cmds = render_ui(&state_with(5), &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        assert!(has_filter, "should show \"Filter: …\" indicator");
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_name = texts.iter().any(|t| {
            if let RenderCmd::Text { text, fg, bold, .. } = t {
                text == "Station 1" && *fg == Some(default_theme().success) && *bold
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&state_with(5), &default_theme(), 80, 24);
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
        let cmds = render_ui(&state_with(5), &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
                Some(default_theme().success)
            );
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn lyrics_panel_shown_when_enabled() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds2 = render_ui(&st, &default_theme(), 20, 24);
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
        assert!(!has_lyrics, "lyrics hidden when right panel < 15 wide");
    }

    #[test]
    fn split_layout_when_lyrics_shown() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        let cmds = render_ui(&st, &default_theme(), 80, 24);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        assert_eq!(borders.len(), 3);
        // First border (Stations) should have width = 80*3/5 = 48
        if let RenderCmd::Border { w, .. } = borders[0] {
            assert_eq!(*w, 48);
        }
        // Third border (Lyrics) should start at x = 48
        if let RenderCmd::Border { x, .. } = borders[2] {
            assert_eq!(*x, 48);
        }
    }

    #[test]
    fn now_playing_panel_contains_volume() {
        let mut st = state_with(5);
        st.volume = 75;
        let cmds = render_ui(&st, &default_theme(), 80, 24);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        assert!(borders.len() >= 2);
        if let RenderCmd::Border { title, .. } = borders[1] {
            let t = title.as_deref().unwrap_or("");
            assert!(
                t.contains("Vol: 75%"),
                "expected Vol: 75% in title, got: {t}"
            );
        }
    }

    #[test]
    fn table_columns_use_country_name() {
        let mut st = state_with(3);
        // Override country to known codes
        st.stations[0].country = "DE".into();
        st.stations[1].country = "FR".into();
        st.stations[2].country = "XX".into();
        st.apply_filter();
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
        let cmds = render_ui(&st, &default_theme(), 80, 10);
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_error = texts.iter().any(|t| {
            if let RenderCmd::Text { text, fg, .. } = t {
                text == "⚠ Error" && *fg == Some(default_theme().error)
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
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
    fn stations_panel_focused_when_lyrics_shown_and_not_lyrics_focused() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        st.lyrics_focused = false;
        let cmds = render_ui(&st, &default_theme(), 80, 24);
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        // Stations panel (first border) should use accent when focused
        if let RenderCmd::Border { fg, .. } = borders[0] {
            assert_eq!(
                *fg,
                default_theme().accent,
                "stations panel should use accent when focused"
            );
        }
    }

    #[test]
    fn lyrics_panel_focused_when_lyrics_focused() {
        let mut st = state_with(5);
        st.show_lyrics = true;
        st.lyrics_focused = true;
        let cmds = render_ui(&st, &default_theme(), 80, 24);
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
                default_theme().accent,
                "lyrics panel should use accent when focused"
            );
        }
    }
}
