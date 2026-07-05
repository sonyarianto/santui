use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};

use crate::state::{ClipState, Screen};

pub fn render_ui(state: &ClipState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];
    match &state.screen {
        Screen::List => cmds.extend(render_list(state, theme, w, h)),
        Screen::View(idx) => cmds.extend(render_view(state, theme, w, h, *idx)),
    }
    cmds
}

fn render_list(state: &ClipState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let entry_count = state.filtered.len();
    let title = format!(" Clipboard History — {} entries ", entry_count);

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let search_text = if state.search_query.is_empty() {
        "> ".to_string()
    } else {
        format!("> {}", state.search_query)
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: search_text,
        fg: Some(theme.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    if state.filtered.is_empty() && state.entries.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 3,
            text: "Clipboard history is empty. Copy something to get started.".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else if state.filtered.is_empty() && !state.entries.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 3,
            text: "No matching entries.".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else {
        for (i, &entry_idx) in state.filtered.iter().enumerate() {
            let y = 3 + i as u16;
            let is_selected = i == state.cursor;
            let entry = &state.entries[entry_idx];
            let age = now.saturating_sub(entry.id);
            let ts = human_time(age);
            let is_copied = state.last_copied_id == Some(entry.id);

            let prefix = if is_selected { "▶ " } else { "  " };
            let line = format!("{}{}", prefix, entry.preview);
            let line_len = line.chars().count();

            cmds.push(RenderCmd::Text {
                x: 2,
                y,
                text: line,
                fg: Some(theme.text),
                bg: if is_selected {
                    Some(theme.accent)
                } else {
                    None
                },
                bold: is_selected,
                modifiers: 0,
            });

            if is_copied {
                let flash = " ✓ Copied!";
                cmds.push(RenderCmd::Text {
                    x: 2 + line_len as u16,
                    y,
                    text: flash.into(),
                    fg: Some(theme.success),
                    bg: if is_selected {
                        Some(theme.accent)
                    } else {
                        None
                    },
                    bold: false,
                    modifiers: 0,
                });
            }

            let ts_x = w.saturating_sub(2 + ts.len() as u16);
            cmds.push(RenderCmd::Text {
                x: ts_x,
                y,
                text: ts,
                fg: Some(theme.text_muted),
                bg: if is_selected {
                    Some(theme.accent)
                } else {
                    None
                },
                bold: false,
                modifiers: 0,
            });
        }
    }

    if let Some(ref err) = state.clipboard_error {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: h.saturating_sub(2),
            text: format!("⚠ Clipboard unavailable: {err}"),
            fg: Some(theme.error),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds
}

fn render_view(state: &ClipState, theme: &ThemeData, w: u16, h: u16, idx: usize) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let entry = &state.entries[idx];

    let title = " Clipboard Entry ".to_string();

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Paragraph {
        x: 2,
        y: 1,
        w: w.saturating_sub(4),
        h: h.saturating_sub(4),
        text: entry.content.clone(),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        wrap: true,
        spans: None,
        alignment: None,
    });

    let char_count = format!("{} characters", entry.content.chars().count());
    cmds.push(RenderCmd::Text {
        x: w.saturating_sub(2 + char_count.len() as u16),
        y: h.saturating_sub(2),
        text: char_count,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

fn human_time(secs: u64) -> String {
    if secs < 60 {
        "just now".into()
    } else if secs < 3600 {
        format!("{} min ago", secs / 60)
    } else if secs < 86400 {
        let hrs = secs / 3600;
        if hrs == 1 {
            "1 hour ago".into()
        } else {
            format!("{} hours ago", hrs)
        }
    } else {
        let days = secs / 86400;
        if days == 1 {
            "1 day ago".into()
        } else {
            format!("{} days ago", days)
        }
    }
}

pub fn help_text(state: &ClipState) -> Vec<(String, String)> {
    match &state.screen {
        Screen::List => {
            if state.search_query.is_empty() {
                vec![
                    ("enter".into(), "copy".into()),
                    ("v".into(), "view".into()),
                    ("d".into(), "delete".into()),
                    ("/".into(), "search".into()),
                ]
            } else {
                vec![
                    ("enter".into(), "copy".into()),
                    ("v".into(), "view".into()),
                    ("d".into(), "delete".into()),
                    ("esc".into(), "clear search".into()),
                ]
            }
        }
        Screen::View(_) => {
            vec![
                ("enter".into(), "copy".into()),
                ("esc".into(), "back".into()),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn renders_entry_list() {
        let mut state = ClipState::new();
        state.push("test entry content".into(), 1000);
        state.apply_filter();
        let theme = test_theme();
        let cmds = render_ui(&state, &theme, 80, 24);
        let has_entry = cmds.iter().any(|c| match c {
            RenderCmd::Text { ref text, .. } => text.contains("test entry content"),
            _ => false,
        });
        assert!(has_entry);
    }

    #[test]
    fn renders_empty_state_message() {
        let state = ClipState::new();
        let theme = test_theme();
        let cmds = render_ui(&state, &theme, 80, 24);
        let has_empty = cmds.iter().any(|c| match c {
            RenderCmd::Text { ref text, .. } => text.contains("Clipboard history is empty"),
            _ => false,
        });
        assert!(has_empty);
    }

    #[test]
    fn renders_search_bar() {
        let mut state = ClipState::new();
        state.search_query = "test query".into();
        let theme = test_theme();
        let cmds = render_ui(&state, &theme, 80, 24);
        let has_search = cmds.iter().any(|c| match c {
            RenderCmd::Text { ref text, .. } => text.contains("> test query"),
            _ => false,
        });
        assert!(has_search);
    }

    #[test]
    fn renders_selected_highlight() {
        let mut state = ClipState::new();
        state.push("highlighted entry".into(), 100);
        state.apply_filter();
        state.cursor = 0;
        let theme = test_theme();
        let cmds = render_ui(&state, &theme, 80, 24);
        let has_highlight = cmds.iter().any(|c| match c {
            RenderCmd::Text { ref text, bg, .. } => {
                text.contains("highlighted entry") && *bg == Some(theme.accent)
            }
            _ => false,
        });
        assert!(has_highlight);
    }

    #[test]
    fn renders_entry_count_in_header() {
        let mut state = ClipState::new();
        for i in 0..3 {
            state.push(format!("entry {i}"), i as u64);
        }
        state.apply_filter();
        let theme = test_theme();
        let cmds = render_ui(&state, &theme, 80, 24);
        let has_count = cmds.iter().any(|c| match c {
            RenderCmd::Border { ref title, .. } => {
                title.as_deref().map_or(false, |t| t.contains("3 entries"))
            }
            _ => false,
        });
        assert!(has_count);
    }

    #[test]
    fn renders_clipboard_error_banner() {
        let mut state = ClipState::new();
        state.clipboard_error = Some("test error".into());
        let theme = test_theme();
        let cmds = render_ui(&state, &theme, 80, 24);
        let has_error = cmds.iter().any(|c| match c {
            RenderCmd::Text { ref text, fg, .. } => {
                text.contains("Clipboard unavailable") && *fg == Some(theme.error)
            }
            _ => false,
        });
        assert!(has_error);
    }

    #[test]
    fn renders_view_screen_full_content() {
        let mut state = ClipState::new();
        let long_content = "line1\nline2\nline3\nwith some longer text for testing purposes";
        state.push(long_content.into(), 100);
        state.apply_filter();
        state.screen = Screen::View(0);
        let theme = test_theme();
        let cmds = render_ui(&state, &theme, 80, 24);
        let has_paragraph = cmds.iter().any(|c| match c {
            RenderCmd::Paragraph { ref text, wrap, .. } => {
                text.contains("testing purposes") && *wrap
            }
            _ => false,
        });
        assert!(has_paragraph);
    }

    #[test]
    fn renders_copied_flash_when_last_copied_id_set() {
        let mut state = ClipState::new();
        state.push("copied entry".into(), 42);
        state.apply_filter();
        state.last_copied_id = Some(42);
        let theme = test_theme();
        let cmds = render_ui(&state, &theme, 80, 24);
        let has_flash = cmds.iter().any(|c| match c {
            RenderCmd::Text { ref text, fg, .. } => {
                text.contains("Copied!") && *fg == Some(theme.success)
            }
            _ => false,
        });
        assert!(has_flash);
    }
}
