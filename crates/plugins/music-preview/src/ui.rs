use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};
use santui_ipc::ui;

use crate::state::{detail_lines, FetchState, MusicState};

pub fn render_ui(state: &MusicState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Music Preview".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let cursor = ui::blink_cursor(state.tick_counter);
    let inner_w = w.saturating_sub(4) as usize;

    let count_label = if state.results.is_empty() {
        String::new()
    } else {
        let n = state.results.len();
        if n == 1 {
            "1 track".into()
        } else {
            format!("{n} tracks")
        }
    };
    let right_len = count_label.len();

    if state.search_mode {
        let left_text = format!("Search: {}{cursor}", state.query);
        let max_left = inner_w.saturating_sub(right_len + 1);
        let display_left: String = left_text.chars().take(max_left).collect();
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: display_left,
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        if !count_label.is_empty() {
            let right_x = ui::right_align_x(w, &count_label);
            cmds.push(RenderCmd::Text {
                x: right_x,
                y: 1,
                text: count_label,
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    } else {
        if !state.query.is_empty() {
            let left_text = format!("Search: {}", state.query);
            let max_left = inner_w.saturating_sub(right_len + 1);
            let display_left: String = left_text.chars().take(max_left).collect();
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 1,
                text: display_left,
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
        if !count_label.is_empty() {
            let right_x = ui::right_align_x(w, &count_label);
            cmds.push(RenderCmd::Text {
                x: right_x,
                y: 1,
                text: count_label,
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    }

    match &state.fetch_state {
        FetchState::Fetching => {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 3,
                text: "\u{27F3} Searching...".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
        FetchState::Error(e) => {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 3,
                text: format!("Error: {e}"),
                fg: Some(theme.error),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
        FetchState::Done => {
            if state.results.is_empty() {
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: 3,
                    text: format!("No tracks found for '{}'", state.query),
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
            } else {
                render_table(state, theme, w, h, &mut cmds);
            }
        }
        FetchState::Idle => {
            if !state.search_mode && state.query.is_empty() && state.results.is_empty() {
                let text = "press / to start search";
                let text_x = (w.saturating_sub(text.len() as u16)) / 2;
                let text_y = h / 2;
                cmds.push(RenderCmd::Text {
                    x: text_x,
                    y: text_y,
                    text: text.into(),
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
            }
        }
    }

    // ---- Track Details side panel (snapped right, dim behind) ----
    if state.show_details {
        if let Some(track) = state.selected_track() {
            let popup_w = (w * 2 / 5).max(20);
            let popup_x = w.saturating_sub(popup_w);
            let popup_h = h;
            if popup_x >= 4 && h >= 10 {
                ui::popup_backdrop(&mut cmds, theme, popup_x, 0, popup_w, popup_h);

                ui::draw_panel(
                    &mut cmds,
                    theme,
                    popup_x,
                    0,
                    popup_w,
                    popup_h,
                    Some("Track Details"),
                    ui::PanelOpts {
                        focused: true,
                        footer: Some(&[("↑↓", "scroll"), ("d", "hide details")]),
                        dim_unfocused: false,
                        ..Default::default()
                    },
                );

                let inner_w = popup_w.saturating_sub(4) as usize;
                let elapsed = if state.now_playing == Some(state.selected) {
                    state.track_elapsed
                } else {
                    None
                };
                let lines = detail_lines(track, elapsed, inner_w);
                let panel_h = popup_h.saturating_sub(4) as usize;
                let start = state.details_scroll.min(lines.len().saturating_sub(1));
                for (y, (i, line)) in
                    (1u16..).zip(lines.iter().enumerate().skip(start).take(panel_h))
                {
                    let playing = i == 0 && state.now_playing == Some(state.selected);
                    let base_x = popup_x + 2;
                    let (label, value) = split_key_value(line);
                    if let Some(label) = label {
                        cmds.push(RenderCmd::Text {
                            x: base_x,
                            y,
                            text: format!("{label} "),
                            fg: Some(theme.text_muted),
                            bg: None,
                            bold: playing,
                            modifiers: 0,
                        });
                        cmds.push(RenderCmd::Text {
                            x: base_x + label.len() as u16 + 1,
                            y,
                            text: value.to_string(),
                            fg: Some(if playing { theme.accent } else { theme.text }),
                            bg: None,
                            bold: playing,
                            modifiers: 0,
                        });
                    } else {
                        cmds.push(RenderCmd::Text {
                            x: base_x,
                            y,
                            text: value.to_string(),
                            fg: Some(if playing {
                                theme.accent
                            } else {
                                theme.text_muted
                            }),
                            bg: None,
                            bold: playing,
                            modifiers: 0,
                        });
                    }
                }
            }
        }
    }

    cmds
}

/// Split a `Key: Value` detail line into label and value. Returns `(None, line)`
/// for lines without a label (playing status, wrapped continuation).
fn split_key_value(line: &str) -> (Option<&str>, &str) {
    match line.split_once(": ") {
        Some((k, v)) if !k.contains([' ', '(', '[', '▶']) => (Some(k), v),
        _ => (None, line),
    }
}

fn render_table(state: &MusicState, theme: &ThemeData, w: u16, h: u16, cmds: &mut Vec<RenderCmd>) {
    let inner_w = w.saturating_sub(4) as usize;
    let table_top = 3u16;

    let dur_w = 9usize;
    let remaining = inner_w.saturating_sub(dur_w);
    let title_w = remaining * 40 / 100;
    let artist_w = remaining * 25 / 100;
    let album_w = remaining * 25 / 100;
    let genre_w = remaining.saturating_sub(title_w + artist_w + album_w);

    let max_visible = (h.saturating_sub(table_top + 2)) as usize;
    let scroll = state.scroll;
    let visible_count = max_visible.min(state.results.len().saturating_sub(scroll));

    if visible_count == 0 {
        return;
    }

    let mut rows = Vec::with_capacity(visible_count);
    for i in 0..visible_count {
        let track = &state.results[scroll + i];
        let is_now_playing = state.now_playing == Some(scroll + i);
        let duration = if is_now_playing {
            if let Some(elapsed) = state.track_elapsed {
                format_duration((elapsed * 1000) as u32)
            } else {
                track
                    .track_time_millis
                    .map(format_duration)
                    .unwrap_or_else(|| "--:--".into())
            }
        } else {
            track
                .track_time_millis
                .map(format_duration)
                .unwrap_or_else(|| "--:--".into())
        };
        rows.push(vec![
            ui::truncate(&track.track_name, title_w),
            ui::truncate(&track.artist_name, artist_w),
            ui::truncate(&track.collection_name, album_w),
            ui::truncate(&track.primary_genre_name, genre_w),
            duration,
        ]);
    }

    let vis_selected = ui::vis_selected(state.selected, scroll, visible_count);

    let vis_now_playing = state.now_playing.and_then(|np| {
        if np >= scroll && np < scroll + visible_count {
            Some(np - scroll)
        } else {
            None
        }
    });

    let ts = ui::table_styles(theme);
    cmds.push(RenderCmd::Table {
        x: 2,
        y: table_top,
        w: inner_w as u16,
        h: (visible_count + 1) as u16,
        header: vec![
            "Title".into(),
            "Artist".into(),
            "Album".into(),
            "Genre".into(),
            "Duration".into(),
        ],
        header_style: ts.header,
        rows,
        column_widths: vec![
            title_w as u16,
            artist_w as u16,
            album_w as u16,
            genre_w as u16,
            dur_w as u16,
        ],
        selected: vis_selected,
        style: ts.body,
        highlight_style: ts.highlight,
        current_row: vis_now_playing,
        current_style: Some(TextStyle {
            fg: Some(theme.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        }),
        cell_styles: None,
    });
}

fn format_duration(millis: u32) -> String {
    let total_secs = millis / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::make_track;

    #[test]
    fn renders_search_bar_in_search_mode() {
        let state = MusicState {
            query: "eminem".into(),
            search_mode: true,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_search = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Search: eminem")),
        );
        assert!(has_search);
    }

    #[test]
    fn no_search_hint_when_idle() {
        let state = MusicState::default();
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let hint = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Search: ")));
        assert!(hint.is_none());
    }

    #[test]
    fn idle_state_shows_centered_hint() {
        let state = MusicState::default();
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let hint = cmds.iter().find(
            |c| matches!(c, RenderCmd::Text { text, .. } if text == "press / to start search"),
        );
        assert!(hint.is_some());
        if let Some(RenderCmd::Text { x, y, fg, .. }) = hint {
            assert_eq!(*x, (80 - 23) / 2);
            assert_eq!(*y, 12);
            assert_eq!(*fg, Some(santui_ipc::test::theme().text_muted));
        }
    }

    #[test]
    fn renders_fetching_spinner() {
        let state = MusicState {
            query: "test".into(),
            fetch_state: FetchState::Fetching,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_spinner = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Searching")));
        assert!(has_spinner);
    }

    #[test]
    fn renders_results_table() {
        let state = MusicState {
            query: "eminem".into(),
            results: vec![
                make_track(1, "Lose Yourself", ""),
                make_track(2, "Stan", ""),
            ],
            selected: 0,
            scroll: 0,
            fetch_state: FetchState::Done,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_table = cmds.iter().any(|c| matches!(c, RenderCmd::Table { .. }));
        assert!(has_table);
        let has_track = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Table { ref rows, .. } if rows.iter().any(|r| r.iter().any(|cell| cell.contains("Lose Yourself"))))
        });
        assert!(has_track);
    }

    #[test]
    fn shows_countdown_on_now_playing_row() {
        let state = MusicState {
            query: "test".into(),
            results: vec![make_track(1, "Track A", ""), make_track(2, "Track B", "")],
            selected: 0,
            scroll: 0,
            now_playing: Some(0),
            track_elapsed: Some(12),
            fetch_state: FetchState::Done,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_countdown = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Table { ref rows, .. } if rows[0].last().map(|s| s.as_str()) == Some("0:12"))
        });
        assert!(has_countdown, "expected countdown in now-playing row");
    }

    #[test]
    fn renders_no_results_message() {
        let state = MusicState {
            query: "xyzzy".into(),
            results: vec![],
            selected: 0,
            scroll: 0,
            fetch_state: FetchState::Done,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_no_results = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("No tracks found")),
        );
        assert!(has_no_results);
    }

    #[test]
    fn renders_border_title() {
        let state = MusicState::default();
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_title = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { ref title, .. } if title.as_deref() == Some("Music Preview"))
        });
        assert!(has_title);
    }

    #[test]
    fn format_duration_converts_millis() {
        assert_eq!(format_duration(60000), "1:00");
        assert_eq!(format_duration(125000), "2:05");
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(3599000), "59:59");
    }

    #[test]
    fn details_panel_renders_when_open() {
        let state = MusicState {
            results: vec![make_track(1, "Lose Yourself", "http://x")],
            fetch_state: FetchState::Done,
            show_details: true,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_title = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Border { title: Some(t), .. } if t == "Track Details"));
        assert!(has_title);
        let has_track = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text == "Lose Yourself"));
        assert!(has_track);
    }

    #[test]
    fn details_panel_not_rendered_when_closed() {
        let state = MusicState {
            results: vec![make_track(1, "Lose Yourself", "http://x")],
            fetch_state: FetchState::Done,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_title = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Border { title: Some(t), .. } if t == "Track Details"));
        assert!(!has_title);
    }

    #[test]
    fn details_panel_respects_scroll() {
        let mut track = make_track(1, "Lose Yourself", "http://x");
        track.artist_name = "Eminem".into();
        track.collection_name = "8 Mile Soundtrack".into();
        track.primary_genre_name = "Hip-Hop/Rap".into();
        let state = MusicState {
            results: vec![track],
            fetch_state: FetchState::Done,
            show_details: true,
            details_scroll: 2,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&String> = cmds
            .iter()
            .filter_map(|c| match c {
                RenderCmd::Text {
                    x: 50, y: 1, text, ..
                } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["Album "]);
        let values: Vec<&String> = cmds
            .iter()
            .filter_map(|c| match c {
                RenderCmd::Text {
                    x: 56, y: 1, text, ..
                } => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(values, vec!["8 Mile Soundtrack"]);
    }
}
