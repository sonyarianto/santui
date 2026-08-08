use santui_ipc::protocol::{BORDER_ALL, RenderCmd, ThemeData};
use santui_ipc::ui;

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

    render_search_view(state, theme, w, h, &mut cmds);

    if state.show_lyrics {
        render_lyrics_panel(state, theme, w, h, &mut cmds);
    }

    cmds
}

fn render_lyrics_panel(
    state: &LyricsState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    cmds: &mut Vec<RenderCmd>,
) {
    let popup_w = (w * 2 / 5).max(20);
    let popup_x = w.saturating_sub(popup_w);
    if popup_x < 4 || h < 10 {
        return;
    }

    ui::popup_backdrop(cmds, theme, popup_x, 0, popup_w, h);

    let footer: &[(&str, &str)] = &[("\u{2191}\u{2193}", "scroll"), ("esc", "close")];
    ui::draw_panel(
        cmds,
        theme,
        popup_x,
        0,
        popup_w,
        h,
        Some("Lyrics"),
        ui::PanelOpts {
            focused: true,
            footer: Some(footer),
            dim_unfocused: false,
            ..Default::default()
        },
    );

    let inner_w = popup_w.saturating_sub(4) as usize;
    let mut y = 1u16;

    if !state.lyrics_title.is_empty() {
        let display: String = state.lyrics_title.chars().take(inner_w).collect();
        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y,
            text: display,
            fg: Some(theme.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        });
        y += 1;
    }
    if !state.lyrics_artist.is_empty() {
        let display: String = state.lyrics_artist.chars().take(inner_w).collect();
        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y,
            text: display,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        y += 1;
    }

    if !state.lyrics_source.is_empty() {
        let source_text = format!("Source: {}", state.lyrics_source);
        let display_src: String = source_text.chars().take(inner_w).collect();
        let sx = popup_x + popup_w.saturating_sub(display_src.len() as u16 + 2);
        cmds.push(RenderCmd::Text {
            x: sx,
            y: h.saturating_sub(2),
            text: display_src,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    let max_lines = h.saturating_sub(y + 3) as usize;

    if state.lyrics_loading {
        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y,
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
            x: popup_x + 2,
            y,
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
    let mut last_y = y;
    for i in 0..max_lines {
        let idx = scroll + i;
        if idx >= lines.len() {
            break;
        }
        let display: String = lines[idx].chars().take(inner_w).collect();
        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y: y + i as u16,
            text: display,
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        last_y = y + i as u16;
    }

    let total = lines.len();
    if total > max_lines {
        let pct = ui::scroll_pct(scroll, total, max_lines);
        let scroll_text = format!("{pct}%");
        let sx = (popup_x + 2 + inner_w as u16).saturating_sub(scroll_text.len() as u16 + 1);
        cmds.push(RenderCmd::Text {
            x: sx,
            y: last_y,
            text: scroll_text,
            fg: Some(theme.text_muted),
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

    let cursor = ui::blink_cursor(state.tick_counter);

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

    let remaining = inner_w.saturating_sub(2);
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
            ui::truncate(&track.track_name, title_w),
            ui::truncate(&track.artist_name, artist_w),
            ui::truncate(&track.album_name, album_w),
        ]);
    }

    let vis_selected = ui::vis_selected(state.selected, scroll, visible_count);

    let ts = ui::table_styles(theme);
    cmds.push(RenderCmd::Table {
        x: 2,
        y: table_top,
        w: inner_w as u16,
        h: (visible_count + 1) as u16,
        header: vec!["Title".into(), "Artist".into(), "Album".into()],
        header_style: ts.header,
        rows,
        column_widths: vec![title_w as u16, artist_w as u16, album_w as u16],
        selected: vis_selected,
        style: ts.body,
        highlight_style: ts.highlight,
        current_row: None,
        current_style: None,
        cell_styles: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::make_track;

    #[test]
    fn renders_search_bar_in_search_mode() {
        let state = LyricsState {
            query: "eminem".into(),
            search_mode: true,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_search = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Search: eminem")));
        assert!(has_search);
    }

    #[test]
    fn no_search_hint_when_idle() {
        let state = LyricsState::default();
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let hint = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Search: ")));
        assert!(hint.is_none());
    }

    #[test]
    fn idle_state_shows_centered_hint() {
        let state = LyricsState::default();
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
        let state = LyricsState {
            query: "test".into(),
            fetch_state: FetchState::Fetching,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_spinner = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Searching")));
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
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_table = cmds.iter().any(|c| matches!(c, RenderCmd::Table { .. }));
        assert!(has_table);
        let has_track = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Table { rows, .. } if rows.iter().any(|r| r.iter().any(|cell| cell.contains("Lose Yourself"))))
        });
        assert!(has_track);
    }

    #[test]
    fn long_title_keeps_ellipsis_after_truncation() {
        let state = LyricsState {
            query: "x".into(),
            results: vec![make_track(1, &"X".repeat(100))],
            selected: 0,
            scroll: 0,
            fetch_state: FetchState::Done,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let title_cell = cmds.iter().find_map(|c| match c {
            RenderCmd::Table { rows, .. } => rows.first().map(|r| r[0].clone()),
            _ => None,
        });
        let title_cell = title_cell.expect("expected a title cell");
        assert!(title_cell.ends_with("..."), "got: {title_cell:?}");
        assert_eq!(
            title_cell.chars().count(),
            29,
            "title cell must fit the rendered column width"
        );
    }

    #[test]
    fn renders_no_results_message() {
        let state = LyricsState {
            query: "xyzzy".into(),
            results: vec![],
            fetch_state: FetchState::Done,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_no_results = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("No results")));
        assert!(has_no_results);
    }

    #[test]
    fn renders_border_title() {
        let state = LyricsState::default();
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_title = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { title, .. } if title.as_deref() == Some("Music Lyrics"))
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
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_title = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "Lose Yourself"));
        assert!(has_title);
        let has_artist = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "Eminem"));
        assert!(has_artist);
        let has_source = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Source: LRCLib")));
        assert!(has_source);
        let has_lyric = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "line one"));
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
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_msg = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "No lyrics found"));
        assert!(has_msg);
    }

    #[test]
    fn lyrics_view_shows_loading_message() {
        let state = LyricsState {
            show_lyrics: true,
            lyrics_loading: true,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_msg = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "Searching lyrics..."));
        assert!(has_msg);
    }

    #[test]
    fn lyrics_panel_rendered_when_open() {
        let state = LyricsState {
            show_lyrics: true,
            lyrics_title: "Lose Yourself".into(),
            lyrics_artist: "Eminem".into(),
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let panel = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { title, x, w, .. } if title.as_deref() == Some("Lyrics") && *x == 48 && *w == 32)
        });
        assert!(panel, "expected Lyrics panel snapped right");
        let has_backdrop = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Rect { x, w, bg, .. } if *x == 48 && *w == 32 && *bg == santui_ipc::test::theme().background_panel)
        });
        assert!(has_backdrop, "expected backdrop behind panel");
    }

    #[test]
    fn lyrics_panel_not_rendered_when_closed() {
        let state = LyricsState::default();
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let panel = cmds.iter().any(
            |c| matches!(c, RenderCmd::Border { title, .. } if title.as_deref() == Some("Lyrics")),
        );
        assert!(!panel, "no panel when lyrics closed");
    }

    #[test]
    fn lyrics_panel_shows_footer_hints() {
        let state = LyricsState {
            show_lyrics: true,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_scroll = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "\u{2191}\u{2193}"));
        let has_close = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "esc"));
        assert!(has_scroll, "expected scroll hint in panel footer");
        assert!(has_close, "expected close hint in panel footer");
    }

    #[test]
    fn lyrics_scroll_shows_percentage_when_overflow() {
        let state = LyricsState {
            show_lyrics: true,
            lyrics_text: (0..50)
                .map(|i| format!("line {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
            lyrics_scroll: 30,
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let texts: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Text { .. }))
            .collect();
        let has_pct = texts.iter().any(|t| {
            if let RenderCmd::Text { text, .. } = t {
                text.ends_with('%')
            } else {
                false
            }
        });
        assert!(has_pct, "expected scroll percentage indicator");
    }

    #[test]
    fn lyrics_scroll_no_percentage_when_fits() {
        let state = LyricsState {
            show_lyrics: true,
            lyrics_text: "only three lines".into(),
            ..LyricsState::default()
        };
        let cmds = render_ui(&state, &santui_ipc::test::theme(), 80, 24);
        let has_pct = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.ends_with('%')));
        assert!(!has_pct, "no percentage when content fits");
    }
}
