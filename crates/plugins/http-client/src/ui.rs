use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};

use crate::state::{ClientState, EditField, FetchState, Screen};

pub fn render_ui(state: &ClientState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];

    match &state.screen {
        Screen::Editor => cmds.extend(render_editor(state, theme, w, h)),
        Screen::Response => cmds.extend(render_response(state, theme, w, h)),
        Screen::History => cmds.extend(render_history(state, theme, w, h)),
        Screen::Saved => cmds.extend(render_saved(state, theme, w, h)),
        Screen::MethodPicker => {
            cmds.extend(render_editor(state, theme, w, h));
            cmds.extend(render_method_picker(state, theme, w, h));
        }
    }

    cmds
}

fn method_label(method: &crate::client::HttpMethod) -> String {
    format!("[ {} ]", method.as_str())
}

fn render_editor(state: &ClientState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("HTTP Client".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let field_w = w.saturating_sub(8).max(20);
    let col_x: u16 = 4;

    let mut row = 1u16;

    // --- Method field ---
    let method_active = state.edit_field == EditField::Method;
    cmds.push(RenderCmd::Text {
        x: col_x,
        y: row,
        text: if method_active {
            format!("▶ Method:  {}", method_label(&state.method))
        } else {
            format!("  Method:  {}", method_label(&state.method))
        },
        fg: if method_active {
            Some(theme.accent)
        } else {
            Some(theme.text)
        },
        bg: None,
        bold: method_active,
        modifiers: 0,
    });
    row += 1;

    // --- URL field ---
    let url_active = state.edit_field == EditField::Url;
    let url_label = format!(
        "{} URL:     > {}",
        if url_active { "▶" } else { " " },
        visible_text(
            &state.url,
            state.edit_cursor,
            state.edit_scroll,
            field_w as usize
        )
    );
    cmds.push(RenderCmd::Text {
        x: col_x,
        y: row,
        text: url_label,
        fg: if url_active {
            Some(theme.accent)
        } else {
            Some(theme.text)
        },
        bg: None,
        bold: url_active,
        modifiers: 0,
    });
    row += 1;

    // --- Headers field ---
    let headers_active = state.edit_field == EditField::Headers;
    let headers_label = format!(
        "{} Headers:  {}",
        if headers_active { "▶" } else { " " },
        if state.headers_text.is_empty() {
            "(empty)".to_string()
        } else {
            first_line(&state.headers_text, field_w as usize)
        }
    );
    cmds.push(RenderCmd::Text {
        x: col_x,
        y: row,
        text: headers_label,
        fg: if headers_active {
            Some(theme.accent)
        } else {
            Some(theme.text)
        },
        bg: None,
        bold: headers_active,
        modifiers: 0,
    });

    if headers_active && !state.headers_text.is_empty() {
        for (i, line) in state.headers_text.lines().skip(1).enumerate() {
            if row + 1 + i as u16 >= h.saturating_sub(3) {
                break;
            }
            cmds.push(RenderCmd::Text {
                x: col_x + 11,
                y: row + 1 + i as u16,
                text: line.to_string(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    }
    if headers_active {
        row += 1 + count_header_lines(&state.headers_text, h.saturating_sub(3));
    } else {
        row += 1;
    }

    // --- Body field ---
    let body_active = state.edit_field == EditField::Body;
    let body_label = format!(
        "{} Body:     {}",
        if body_active { "▶" } else { " " },
        if state.body_text.is_empty() {
            "(empty)".to_string()
        } else {
            first_line(&state.body_text, field_w.saturating_sub(12) as usize)
        }
    );
    cmds.push(RenderCmd::Text {
        x: col_x,
        y: row,
        text: body_label,
        fg: if body_active {
            Some(theme.accent)
        } else {
            Some(theme.text)
        },
        bg: None,
        bold: body_active,
        modifiers: 0,
    });

    if body_active && !state.body_text.is_empty() {
        for (i, line) in state.body_text.lines().skip(1).enumerate() {
            if row + 1 + i as u16 >= h.saturating_sub(3) {
                break;
            }
            cmds.push(RenderCmd::Text {
                x: col_x + 11,
                y: row + 1 + i as u16,
                text: line.to_string(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    }

    cmds
}

fn render_response(state: &ClientState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("HTTP Client — Response".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let col_x = 2u16;
    let mut row = 1u16;

    match &state.fetch_state {
        FetchState::Sending => {
            cmds.push(RenderCmd::Text {
                x: col_x,
                y: row,
                text: "Sending request...".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            return cmds;
        }
        FetchState::Error(e) => {
            cmds.push(RenderCmd::Border {
                x: col_x,
                y: row,
                w: w.saturating_sub(4),
                h: 3,
                fg: theme.error,
                bg: None,
                borders: BORDER_ALL,
                title: Some("Status".into()),
                title_fg: Some(theme.error),
                title_dash_fg: None,
                border_type: None,
            });
            cmds.push(RenderCmd::Text {
                x: col_x + 1,
                y: row + 1,
                text: format!("Error: {}", e),
                fg: Some(theme.error),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            return cmds;
        }
        _ => {}
    }

    let resp = match &state.response {
        Some(r) => r,
        None => {
            cmds.push(RenderCmd::Text {
                x: col_x,
                y: row,
                text: "(No response)".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            return cmds;
        }
    };

    // Status
    let status_color = if resp.status < 400 {
        theme.success
    } else if resp.status < 500 {
        theme.highlight
    } else {
        theme.error
    };
    cmds.push(RenderCmd::Text {
        x: col_x,
        y: row,
        text: "═══ Status ═══════════════════════════════════════════".into(),
        fg: Some(theme.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    row += 1;
    cmds.push(RenderCmd::Text {
        x: col_x,
        y: row,
        text: format!(
            "{} {}  ({} ms)",
            resp.status, resp.status_text, resp.elapsed_ms
        ),
        fg: Some(status_color),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    row += 2;

    // Headers
    cmds.push(RenderCmd::Text {
        x: col_x,
        y: row,
        text: "═══ Response Headers ═════════════════════════════════".into(),
        fg: Some(theme.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    row += 1;
    let max_headers = 8usize;
    let max_h = h.saturating_sub(row).saturating_sub(5) as usize;
    for (name, value) in resp.headers.iter().take(max_headers.min(max_h)) {
        if row >= h.saturating_sub(5) {
            break;
        }
        cmds.push(RenderCmd::Text {
            x: col_x,
            y: row,
            text: format!("{}: {}", name, value),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        row += 1;
    }
    row += 1;

    // Body
    cmds.push(RenderCmd::Text {
        x: col_x,
        y: row,
        text: "═══ Body ══════════════════════════════════════════════".into(),
        fg: Some(theme.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    row += 1;

    let body_lines: Vec<&str> = resp.body.lines().collect();
    let max_body_lines = h.saturating_sub(row).saturating_sub(2) as usize;
    for line in body_lines
        .iter()
        .skip(state.response_scroll)
        .take(max_body_lines)
    {
        cmds.push(RenderCmd::Text {
            x: col_x,
            y: row,
            text: line.to_string(),
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        row += 1;
    }

    if resp.body_truncated {
        cmds.push(RenderCmd::Text {
            x: col_x,
            y: row,
            text: "(truncated at 10KB)".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds
}

fn render_history(state: &ClientState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("HTTP Client — History".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    render_entry_list(&mut cmds, state, theme, w, h, true);

    cmds
}

fn render_saved(state: &ClientState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("HTTP Client — Saved".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    render_entry_list(&mut cmds, state, theme, w, h, false);

    cmds
}

fn render_entry_list(
    cmds: &mut Vec<RenderCmd>,
    state: &ClientState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    is_history: bool,
) {
    let entries: &[crate::state::RequestEntry] = if is_history {
        &state.history
    } else {
        &state.saved_requests
    };
    let cursor = if is_history {
        state.history_cursor
    } else {
        state.saved_cursor
    };

    let max_visible = h.saturating_sub(3) as usize;
    let col_x = 2u16;

    for (i, entry) in entries.iter().enumerate().take(max_visible) {
        let y = 1 + i as u16;
        let is_selected = i == cursor;
        let prefix = if is_selected { "▶ " } else { "  " };

        let line = format!(
            "{}{:<7} {}",
            prefix,
            entry.method,
            santui_ipc::ui::truncate(&entry.url, w.saturating_sub(14) as usize),
        );
        cmds.push(RenderCmd::Text {
            x: col_x,
            y,
            text: line,
            fg: if is_selected {
                Some(theme.inverted_text)
            } else {
                Some(theme.text)
            },
            bg: if is_selected {
                Some(theme.accent)
            } else {
                None
            },
            bold: is_selected,
            modifiers: 0,
        });
    }
}

fn render_method_picker(state: &ClientState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let methods = crate::client::HttpMethod::all();
    let popup_w: u16 = 30.min(w.saturating_sub(4));
    let popup_h: u16 = (methods.len() + 2) as u16;
    let popup_x: u16 = (w - popup_w) / 2;
    let popup_y: u16 = (h - popup_h) / 2;

    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
        bg: theme.background_overlay,
    });

    cmds.push(RenderCmd::Border {
        x: popup_x,
        y: popup_y,
        w: popup_w,
        h: popup_h,
        fg: theme.border,
        bg: Some(theme.background_overlay),
        borders: BORDER_ALL,
        title: Some("Select Method".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    for (i, method) in methods.iter().enumerate() {
        let y = popup_y + 1 + i as u16;
        let is_selected = i == state.picker_cursor;
        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y,
            text: format!(
                "  {}  {}",
                if is_selected { "▶" } else { " " },
                method.as_str()
            ),
            fg: if is_selected {
                Some(theme.inverted_text)
            } else {
                Some(theme.text)
            },
            bg: if is_selected {
                Some(theme.accent)
            } else {
                None
            },
            bold: is_selected,
            modifiers: 0,
        });
    }

    cmds
}

fn visible_text(text: &str, cursor: usize, _scroll: usize, max_width: usize) -> String {
    if text.is_empty() {
        return "_".to_string();
    }
    if text.len() <= max_width {
        return text.to_string();
    }
    let start = cursor.saturating_sub(max_width / 2);
    let end = (start + max_width).min(text.len());
    text.chars().skip(start).take(end - start).collect()
}

fn first_line(text: &str, max_width: usize) -> String {
    let line = text.lines().next().unwrap_or("");
    if line.len() <= max_width {
        line.to_string()
    } else {
        let truncated: String = line.chars().take(max_width.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

fn count_header_lines(text: &str, max_h: u16) -> u16 {
    let count = text.lines().count().saturating_sub(1);
    count.min(max_h.saturating_sub(1) as usize) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::HttpResponse;
    use crate::state::{ClientState, FetchState, RequestEntry, Screen};

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

    fn base_state() -> ClientState {
        ClientState::default()
    }

    #[test]
    fn renders_editor_with_fields() {
        let mut s = base_state();
        s.url = "https://api.example.com".into();
        s.headers_text = "Content-Type: application/json".into();
        s.body_text = r#"{"key":"value"}"#.into();
        let cmds = render_ui(&s, &test_theme(), 80, 24);
        let has_method = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Method")));
        let has_url = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("api.example.com")),
        );
        assert!(has_method);
        assert!(has_url);
    }

    #[test]
    fn renders_active_field_highlighted() {
        let mut s = base_state();
        s.edit_field = EditField::Url;
        let cmds = render_ui(&s, &test_theme(), 80, 24);
        let has_accent_url = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Text { ref text, fg, .. } if text.contains("URL") && fg == &Some([180; 3]))
        });
        assert!(has_accent_url);
    }

    #[test]
    fn renders_method_picker() {
        let mut s = base_state();
        s.screen = Screen::MethodPicker;
        let cmds = render_ui(&s, &test_theme(), 80, 24);
        let has_overlay = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { ref title, .. } if title.as_deref() == Some("Select Method"))
        });
        let has_get = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("GET")));
        assert!(has_overlay);
        assert!(has_get);
    }

    #[test]
    fn renders_response_status_and_body() {
        let mut s = base_state();
        s.screen = Screen::Response;
        s.fetch_state = FetchState::Done;
        s.response = Some(HttpResponse {
            status: 200,
            status_text: "OK".into(),
            headers: vec![("content-type".into(), "application/json".into())],
            body: r#"{"id":42}"#.into(),
            elapsed_ms: 150,
            body_truncated: false,
        });
        let cmds = render_ui(&s, &test_theme(), 80, 24);
        let has_status = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("200 OK")));
        let has_body = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains(r#""id":42"#)));
        assert!(has_status);
        assert!(has_body);
    }

    #[test]
    fn renders_history_list() {
        let mut s = base_state();
        s.screen = Screen::History;
        s.history.push(RequestEntry {
            method: "GET".into(),
            url: "https://api.example.com".into(),
            headers: String::new(),
            body: String::new(),
        });
        s.history_cursor = 0;
        let cmds = render_ui(&s, &test_theme(), 80, 24);
        let has_title = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { ref title, .. } if title.as_deref() == Some("HTTP Client — History"))
        });
        let has_url = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("api.example.com")),
        );
        assert!(has_title);
        assert!(has_url);
    }

    #[test]
    fn renders_saved_list() {
        let mut s = base_state();
        s.screen = Screen::Saved;
        s.saved_requests.push(RequestEntry {
            method: "POST".into(),
            url: "https://api.example.com/create".into(),
            headers: String::new(),
            body: String::new(),
        });
        s.saved_cursor = 0;
        let cmds = render_ui(&s, &test_theme(), 80, 24);
        let has_title = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { ref title, .. } if title.as_deref() == Some("HTTP Client — Saved"))
        });
        let has_url = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("api.example.com/create")));
        assert!(has_title);
        assert!(has_url);
    }

    #[test]
    fn renders_error_response() {
        let mut s = base_state();
        s.screen = Screen::Response;
        s.fetch_state = FetchState::Error("connection refused".into());
        let cmds = render_ui(&s, &test_theme(), 80, 24);
        let has_error = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("connection refused")),
        );
        assert!(has_error);
    }

    #[test]
    fn renders_sending_state() {
        let mut s = base_state();
        s.screen = Screen::Response;
        s.fetch_state = FetchState::Sending;
        let cmds = render_ui(&s, &test_theme(), 80, 24);
        let has_sending = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Sending")));
        assert!(has_sending);
    }

    #[test]
    fn renders_truncated_notice() {
        let mut s = base_state();
        s.screen = Screen::Response;
        s.fetch_state = FetchState::Done;
        s.response = Some(HttpResponse {
            status: 200,
            status_text: "OK".into(),
            headers: vec![],
            body: "{}".into(),
            elapsed_ms: 0,
            body_truncated: true,
        });
        let cmds = render_ui(&s, &test_theme(), 80, 24);
        let has_truncated = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("truncated at 10KB")),
        );
        assert!(has_truncated);
    }

    #[test]
    fn visible_text_shows_underscore_for_empty() {
        assert_eq!(visible_text("", 0, 0, 10), "_");
    }

    #[test]
    fn visible_text_shows_full_when_fits() {
        assert_eq!(visible_text("hello", 0, 0, 10), "hello");
    }

    #[test]
    fn first_line_returns_first_line() {
        assert_eq!(first_line("a\nb\nc", 100), "a");
    }

    #[test]
    fn first_line_truncates_long() {
        let result = first_line("abcdefghijklmnop", 8);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 8);
    }

    #[test]
    fn count_header_lines_correct() {
        assert_eq!(count_header_lines("a\nb\nc\nd", 100), 3);
    }
}
