use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};

use crate::state::{FetchState, LyricsState};

pub fn render_ui(state: &LyricsState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
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
        title: Some("Music Lyrics".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    if state.show_lyrics {
        render_lyrics_view(state, theme, w, h, &mut cmds);
    } else {
        render_search_view(state, theme, w, h, &mut cmds);
    }

    cmds
}

fn render_lyrics_view(
    state: &LyricsState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    cmds: &mut Vec<RenderCmd>,
) {
    let inner_w = w.saturating_sub(4) as usize;

    let header = if state.lyrics_title.is_empty() && state.lyrics_artist.is_empty() {
        String::new()
    } else {
        format!("{} - {}", state.lyrics_title, state.lyrics_artist)
    };
    if !header.is_empty() {
        let display: String = header.chars().take(inner_w).collect();
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: display,
            fg: Some(theme.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        });
    }

    let source_text = format!("Source: {}", state.lyrics_source);
    let display_src: String = source_text.chars().take(inner_w).collect();
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: display_src,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let max_lines = h.saturating_sub(5) as usize;
    let line_w = w.saturating_sub(4) as usize;

    if state.lyrics_loading {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 4,
            text: "Searching lyrics...".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        return;
    }

    if state.lyrics_text.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 4,
            text: "No lyrics found".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        return;
    }

    let lines: Vec<&str> = state.lyrics_text.lines().collect();
    let scroll = state.lyrics_scroll.min(lines.len().saturating_sub(1));
    for i in 0..max_lines {
        let idx = scroll + i;
        if idx >= lines.len() {
            break;
        }
        let display: String = lines[idx].chars().take(line_w).collect();
        cmds.push(RenderCmd::Text {
            x: 2,
            y: (4 + i) as u16,
            text: display,
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }
}

fn render_search_view(
    state: &LyricsState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    cmds: &mut Vec<RenderCmd>,
) {
    let inner_w = w.saturating_sub(4) as usize;

    let cursor = if state.tick_counter % 6 < 3 {
        '\u{2588}'
    } else {
        ' '
    };

    let count_label = if state.results.is_empty() {
        String::new()
    } else {
        let n = state.results.len();
        if n == 1 {
            "1 result".into()
        } else {
            format!("{n} results")
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
            let right_x = w.saturating_sub(2u16.saturating_add(count_label.len() as u16));
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
        let left_text = if state.query.is_empty() {
            "Search: ".to_string()
        } else {
            format!("Search: {}", state.query)
        };
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
        if !count_label.is_empty() {
            let right_x = w.saturating_sub(2u16.saturating_add(count_label.len() as u16));
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
                    text: format!("No results for '{}'", state.query),
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
            } else {
                render_table(state, theme, w, h, cmds);
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
}

fn render_table(state: &LyricsState, theme: &ThemeData, w: u16, h: u16, cmds: &mut Vec<RenderCmd>) {
    let inner_w = w.saturating_sub(4) as usize;
    let table_top = 3u16;

    let remaining = inner_w;
    let title_w = remaining * 40 / 100;
    let artist_w = remaining * 35 / 100;
    let album_w = remaining.saturating_sub(title_w + artist_w);

    let max_visible = (h.saturating_sub(table_top + 2)) as usize;
    let scroll = state.scroll;
    let visible_count = max_visible.min(state.results.len().saturating_sub(scroll));

    if visible_count == 0 {
        return;
    }

    let mut rows = Vec::with_capacity(visible_count);
    for i in 0..visible_count {
        let track = &state.results[scroll + i];
        rows.push(vec![
            santui_ipc::ui::truncate(&track.track_name, title_w),
            santui_ipc::ui::truncate(&track.artist_name, artist_w),
            santui_ipc::ui::truncate(&track.album_name, album_w),
        ]);
    }

    let vis_selected = if state.selected >= scroll && state.selected < scroll + visible_count {
        Some(state.selected - scroll)
    } else {
        None
    };

    cmds.push(RenderCmd::Table {
        x: 2,
        y: table_top,
        w: inner_w as u16,
        h: (visible_count + 1) as u16,
        header: vec!["Title".into(), "Artist".into(), "Album".into()],
        header_style: TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![title_w as u16, artist_w as u16, album_w as u16],
        selected: vis_selected,
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
        current_row: None,
        current_style: None,
        cell_styles: None,
    });
}

pub fn max_visible_tracks(h: u16) -> usize {
    h.saturating_sub(5) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lrclib::LRCLibTrack;

    fn test_theme() -> ThemeData {
        ThemeData {
            text: [200; 3],
            text_muted: [100; 3],
            accent: [180; 3],
            highlight: [220; 3],
            logo: [255; 3],
            background: [0; 3],
            background_panel: [20; 3],
            background_overlay: [10; 3],
            border: [150; 3],
            success: [80; 3],
            error: [255; 3],
            inverted_text: [255; 3],
        }
    }

    fn make_track(id: u64, name: &str) -> LRCLibTrack {
        LRCLibTrack {
            id,
            track_name: name.into(),
            artist_name: "Artist".into(),
            album_name: "Album".into(),
            duration: 200.0,
            instrumental: false,
            plain_lyrics: None,
            synced_lyrics: None,
        }
    }

    #[test]
    fn renders_search_bar_in_search_mode() {
        let state = LyricsState {
            query: "eminem".into(),
            search_mode: true,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_search = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Search: eminem")),
        );
        assert!(has_search);
    }

    #[test]
    fn renders_dimmed_search_hint_when_not_searching() {
        let state = LyricsState::default();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let hint = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Text { y: 1, .. }));
        assert!(hint.is_some());
        if let Some(RenderCmd::Text { text, fg, .. }) = hint {
            assert_eq!(text, "Search: ");
            assert_eq!(*fg, Some(test_theme().text_muted));
        }
    }

    #[test]
    fn idle_state_shows_centered_hint() {
        let state = LyricsState::default();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let hint = cmds.iter().find(
            |c| matches!(c, RenderCmd::Text { text, .. } if text == "press / to start search"),
        );
        assert!(hint.is_some());
        if let Some(RenderCmd::Text { x, y, fg, .. }) = hint {
            assert_eq!(*x, (80 - 23) / 2);
            assert_eq!(*y, 12);
            assert_eq!(*fg, Some(test_theme().text_muted));
        }
    }

    #[test]
    fn renders_fetching_spinner() {
        let state = LyricsState {
            query: "test".into(),
            fetch_state: FetchState::Fetching,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_spinner = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Searching")));
        assert!(has_spinner);
    }

    #[test]
    fn renders_results_table() {
        let state = LyricsState {
            query: "eminem".into(),
            results: vec![make_track(1, "Lose Yourself"), make_track(2, "Stan")],
            selected: 0,
            scroll: 0,
            fetch_state: FetchState::Done,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_table = cmds.iter().any(|c| matches!(c, RenderCmd::Table { .. }));
        assert!(has_table);
        let has_track = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Table { ref rows, .. } if rows.iter().any(|r| r.iter().any(|cell| cell.contains("Lose Yourself"))))
        });
        assert!(has_track);
    }

    #[test]
    fn renders_no_results_message() {
        let state = LyricsState {
            query: "xyzzy".into(),
            results: vec![],
            fetch_state: FetchState::Done,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_no_results = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("No results")));
        assert!(has_no_results);
    }

    #[test]
    fn renders_border_title() {
        let state = LyricsState::default();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_title = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { ref title, .. } if title.as_deref() == Some("Music Lyrics"))
        });
        assert!(has_title);
    }

    #[test]
    fn lyrics_view_shows_header() {
        let state = LyricsState {
            show_lyrics: true,
            lyrics_title: "Lose Yourself".into(),
            lyrics_artist: "Eminem".into(),
            lyrics_source: "LRCLib".into(),
            lyrics_text: "line one\nline two".into(),
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_header = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Lose Yourself - Eminem")),
        );
        assert!(has_header);
        let has_source = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Source: LRCLib")),
        );
        assert!(has_source);
        let has_lyric = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text == "line one"));
        assert!(has_lyric);
    }

    #[test]
    fn lyrics_view_shows_no_lyrics_message() {
        let state = LyricsState {
            show_lyrics: true,
            lyrics_text: String::new(),
            lyrics_loading: false,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_msg = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text == "No lyrics found"));
        assert!(has_msg);
    }

    #[test]
    fn lyrics_view_shows_loading_message() {
        let state = LyricsState {
            show_lyrics: true,
            lyrics_loading: true,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_msg = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text == "Searching lyrics..."),
        );
        assert!(has_msg);
    }

    #[test]
    fn max_visible_tracks_calculation() {
        assert_eq!(max_visible_tracks(24), 19);
        assert_eq!(max_visible_tracks(10), 5);
        assert_eq!(max_visible_tracks(5), 0);
    }
}
