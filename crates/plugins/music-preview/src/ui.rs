use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};

use crate::state::{FetchState, MusicState};

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

    let cursor = if state.tick_counter % 6 < 3 {
        '█'
    } else {
        ' '
    };
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
        FetchState::Idle => {}
    }

    cmds
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
        let duration = track
            .track_time_millis
            .map(format_duration)
            .unwrap_or_else(|| "--:--".into());
        rows.push(vec![
            santui_ipc::ui::truncate(&track.track_name, title_w),
            santui_ipc::ui::truncate(&track.artist_name, artist_w),
            santui_ipc::ui::truncate(&track.collection_name, album_w),
            santui_ipc::ui::truncate(&track.primary_genre_name, genre_w),
            duration,
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
        header: vec![
            "Title".into(),
            "Artist".into(),
            "Album".into(),
            "Genre".into(),
            "Duration".into(),
        ],
        header_style: TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![
            title_w as u16,
            artist_w as u16,
            album_w as u16,
            genre_w as u16,
            dur_w as u16,
        ],
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

fn format_duration(millis: u32) -> String {
    let total_secs = millis / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ItunesTrack;

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

    fn make_track(id: u64, name: &str) -> ItunesTrack {
        ItunesTrack {
            track_id: id,
            track_name: name.into(),
            artist_name: "Artist".into(),
            collection_name: "Album".into(),
            artwork_url_100: String::new(),
            preview_url: String::new(),
            track_time_millis: Some(180000),
            primary_genre_name: "Rock".into(),
        }
    }

    #[test]
    fn renders_search_bar_in_search_mode() {
        let state = MusicState {
            query: "eminem".into(),
            search_mode: true,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_search = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Search: eminem")),
        );
        assert!(has_search);
    }

    #[test]
    fn renders_dimmed_search_hint_when_not_searching() {
        let state = MusicState::default();
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
    fn idle_state_renders_no_instruction_text() {
        let state = MusicState::default();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_text_at_y3 = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { y: 3, .. }));
        assert!(!has_text_at_y3);
    }

    #[test]
    fn renders_fetching_spinner() {
        let state = MusicState {
            query: "test".into(),
            fetch_state: FetchState::Fetching,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_spinner = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Searching")));
        assert!(has_spinner);
    }

    #[test]
    fn renders_results_table() {
        let state = MusicState {
            query: "eminem".into(),
            results: vec![make_track(1, "Lose Yourself"), make_track(2, "Stan")],
            selected: 0,
            scroll: 0,
            fetch_state: FetchState::Done,
            ..MusicState::default()
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
        let state = MusicState {
            query: "xyzzy".into(),
            results: vec![],
            selected: 0,
            scroll: 0,
            fetch_state: FetchState::Done,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_no_results = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("No tracks found")),
        );
        assert!(has_no_results);
    }

    #[test]
    fn renders_border_title() {
        let state = MusicState::default();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
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
    fn max_visible_tracks_calculation() {
        assert_eq!(max_visible_tracks(24), 19);
        assert_eq!(max_visible_tracks(10), 5);
        assert_eq!(max_visible_tracks(5), 0);
    }
}
