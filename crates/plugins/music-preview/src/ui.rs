use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};

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
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let search_prompt = "> search: ";
    let search_text = format!("{}{}", search_prompt, state.query);
    let displayed = if search_text.len() > w.saturating_sub(4) as usize {
        let available = (w as usize)
            .saturating_sub(4)
            .saturating_sub(search_prompt.len());
        format!(
            "{}{}",
            search_prompt,
            &state.query[state.query.len().saturating_sub(available)..]
        )
    } else {
        search_text
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: displayed,
        fg: Some(theme.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

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
                text: format!("Error: {}", e),
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
                render_results(state, theme, w, h, &mut cmds);
            }
        }
        FetchState::Idle => {
            if state.query.is_empty() {
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: 3,
                    text: "Type a search query and press Enter".into(),
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
            }
        }
    }

    cmds
}

fn render_results(
    state: &MusicState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    cmds: &mut Vec<RenderCmd>,
) {
    let max_visible = max_visible_tracks(h);
    let results = &state.results;
    let selected = state.selected;

    let end = (state.scroll + max_visible).min(results.len());

    for (vi, track) in results
        .iter()
        .enumerate()
        .skip(state.scroll)
        .take(end - state.scroll)
    {
        let y = 3 + vi as u16 * 3 - state.scroll as u16 * 3;
        let is_selected = vi == selected;

        let prefix = if is_selected { "\u{25B6}" } else { " " };
        let title = format!("{} {}. {}", prefix, vi + 1, track.track_name);
        let title = santui_ipc::ui::truncate(&title, w.saturating_sub(4) as usize);

        cmds.push(RenderCmd::Text {
            x: 2,
            y,
            text: title,
            fg: if is_selected {
                Some(theme.accent)
            } else {
                Some(theme.text)
            },
            bg: None,
            bold: is_selected,
            modifiers: 0,
        });

        let duration = track
            .track_time_millis
            .map(format_duration)
            .unwrap_or_else(|| "--:--".into());
        let detail = format!(
            "   {} \u{00B7} {} \u{00B7} {} \u{00B7} {}",
            track.artist_name, track.collection_name, track.primary_genre_name, duration
        );
        let detail = santui_ipc::ui::truncate(&detail, w.saturating_sub(4) as usize);

        cmds.push(RenderCmd::Text {
            x: 2,
            y: y + 1,
            text: detail,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }
}

pub fn max_visible_tracks(h: u16) -> usize {
    ((h.saturating_sub(3) - 1) / 3).max(1) as usize
}

fn format_duration(millis: u32) -> String {
    let total_secs = millis / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{}:{:02}", mins, secs)
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

    fn make_track(id: u32, name: &str) -> ItunesTrack {
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
    fn renders_search_bar() {
        let state = MusicState {
            query: "eminem".into(),
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_search = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("> search:") && text.contains("eminem")),
        );
        assert!(has_search);
    }

    #[test]
    fn renders_empty_initial_state() {
        let state = MusicState::default();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_empty = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Type a search query and press Enter")),
        );
        assert!(has_empty);
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
    fn renders_results_list() {
        let state = MusicState {
            query: "eminem".into(),
            results: vec![make_track(1, "Lose Yourself"), make_track(2, "Stan")],
            selected: 0,
            scroll: 0,
            fetch_state: FetchState::Done,
            ..MusicState::default()
        };
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_track = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Lose Yourself")),
        );
        assert!(has_track);
        let has_artist = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Artist")));
        assert!(has_artist);
        // Selected track uses accent + bold
        let selected_texts: Vec<_> = cmds
            .iter()
            .filter(|c| {
                matches!(c, RenderCmd::Text { ref text, bold, fg, .. }
                    if *bold && fg == &Some([180; 3]) && text.contains("Lose Yourself"))
            })
            .collect();
        assert!(!selected_texts.is_empty());
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
        assert_eq!(max_visible_tracks(24), 6);
        assert_eq!(max_visible_tracks(10), 2);
        assert_eq!(max_visible_tracks(5), 1);
    }
}
