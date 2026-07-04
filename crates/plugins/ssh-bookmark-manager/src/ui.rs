use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};

use crate::state::{unix_now, Screen, SshState};

type FieldAccessor = fn(&crate::state::SshBookmark) -> String;

const RECENTLY_CONNECTED_SECS: u64 = 300;

pub fn render_ui(state: &SshState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];

    match &state.screen {
        Screen::List => cmds.extend(render_list(state, theme, w, h)),
        Screen::Detail => cmds.extend(render_detail(state, theme, w, h)),
        Screen::Connect => cmds.extend(render_connect(state, theme, w, h)),
    }

    cmds
}

fn render_list(state: &SshState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let title = format!(
        "SSH Bookmarks ({}/{})",
        state.filtered_indices.len(),
        state.data.bookmarks.len()
    );
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
    });

    let inner_w = w.saturating_sub(4) as usize;
    let mut row: u16 = 1;

    let filter_display = if state.filter_active {
        format!(
            "filter: {}_",
            santui_ipc::ui::truncate(&state.filter_text, inner_w.saturating_sub(12))
        )
    } else if !state.filter_text.is_empty() {
        format!(
            "filter: {}",
            santui_ipc::ui::truncate(&state.filter_text, inner_w.saturating_sub(10))
        )
    } else {
        "/ to filter".into()
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: row,
        text: filter_display,
        fg: if state.filter_active {
            Some(theme.accent)
        } else {
            Some(theme.text_muted)
        },
        bg: None,
        bold: state.filter_active,
    });
    row += 1;

    let max_items = h.saturating_sub(row).saturating_sub(4) as usize;
    if max_items == 0 {
        return cmds;
    }

    let visible_start = state
        .scroll
        .min(state.filtered_indices.len().saturating_sub(1));
    let visible_end = (visible_start + max_items).min(state.filtered_indices.len());

    if state.filtered_indices.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: row + 2,
            text: if state.data.bookmarks.is_empty() {
                "No bookmarks. Press n to create one.".into()
            } else {
                "No matching bookmarks.".into()
            },
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
    } else {
        for i in visible_start..visible_end {
            let bm_idx = state.filtered_indices[i];
            let bm = &state.data.bookmarks[bm_idx];
            let is_selected = i == state.cursor;

            let connection_indicator = {
                let now = unix_now();
                match bm.last_connected_at {
                    Some(t) if now.saturating_sub(t) < RECENTLY_CONNECTED_SECS => "●",
                    _ => "○",
                }
            };

            let conn_str = format!("{}@{}:{}", bm.user, bm.host, bm.port);
            let line = format!(
                "{} {:<20} {:<30} {:<15} {}",
                if is_selected { "▶" } else { " " },
                santui_ipc::ui::truncate(&bm.name, 20),
                santui_ipc::ui::truncate(&conn_str, 30),
                santui_ipc::ui::truncate(&bm.category, 15),
                connection_indicator,
            );

            cmds.push(RenderCmd::Text {
                x: 2,
                y: row + (i - visible_start) as u16,
                text: santui_ipc::ui::truncate(&line, inner_w),
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
            });
        }
    }

    let legend_y = h.saturating_sub(3);
    cmds.push(RenderCmd::Text {
        x: 2,
        y: legend_y,
        text: "● connected (last 5 min)   ○ untested".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    cmds
}

fn render_detail(state: &SshState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let bm_name = state
        .detail_idx
        .and_then(|i| state.data.bookmarks.get(i))
        .map(|bm| bm.name.clone())
        .unwrap_or_else(|| "New Bookmark".into());

    let title = format!("Edit Bookmark — {}", bm_name);
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
    });

    let fields: &[(&str, FieldAccessor)] = &[
        ("Name", |bm: &crate::state::SshBookmark| bm.name.clone()),
        ("Host", |bm| bm.host.clone()),
        ("Port", |bm| bm.port.to_string()),
        ("User", |bm| bm.user.clone()),
        ("Key path", |bm| {
            bm.key_path.clone().unwrap_or_else(|| "(none)".into())
        }),
        ("Category", |bm| bm.category.clone()),
        ("Description", |bm| bm.description.clone()),
    ];

    let bookmark = state
        .detail_idx
        .and_then(|i| state.data.bookmarks.get(i))
        .cloned()
        .unwrap_or_default();

    let inner_w = w.saturating_sub(4) as usize;

    for (i, (label, getter)) in fields.iter().enumerate() {
        let y = 1 + i as u16;
        let is_active = i == state.detail_edit_field;
        let value = getter(&bookmark);

        let display_value = if is_active && state.editing {
            format!("{}_", state.edit_buffer)
        } else {
            value
        };

        let line = format!(
            "{:<14} {} {}",
            label,
            if is_active { "▸" } else { " " },
            santui_ipc::ui::truncate(&display_value, inner_w.saturating_sub(18)),
        );

        cmds.push(RenderCmd::Text {
            x: 2,
            y,
            text: line,
            fg: if is_active {
                Some(theme.inverted_text)
            } else {
                Some(theme.text)
            },
            bg: if is_active { Some(theme.accent) } else { None },
            bold: is_active,
        });
    }

    let hints_y = h.saturating_sub(2);
    let hints = if state.editing {
        "enter commit | esc cancel | type to edit".into()
    } else {
        "↑↓ navigate | enter edit | esc save & back | F2 save".into()
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: hints_y,
        text: hints,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    cmds
}

fn render_connect(state: &SshState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let target = state
        .detail_idx
        .and_then(|i| state.data.bookmarks.get(i))
        .map(|bm| format!("{}@{}:{}", bm.user, bm.host, bm.port))
        .unwrap_or_else(|| "unknown".into());

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("SSH Connect".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    cmds.push(RenderCmd::Text {
        x: w / 2 - 10,
        y: h / 2,
        text: format!("Connecting to {}...", target),
        fg: Some(theme.highlight),
        bg: None,
        bold: true,
    });

    cmds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SshBookmark;

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

    fn test_bm(name: &str, host: &str, user: &str, category: &str) -> SshBookmark {
        SshBookmark {
            id: name.to_lowercase().replace(' ', "-"),
            name: name.into(),
            host: host.into(),
            port: 22,
            user: user.into(),
            key_path: None,
            category: category.into(),
            description: String::new(),
            last_connected_at: None,
        }
    }

    #[test]
    fn renders_empty_list_message() {
        let state = SshState::default();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_msg = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("No bookmarks")),
        );
        assert!(has_msg);
    }

    #[test]
    fn renders_bookmark_list() {
        let mut state = SshState::default();
        state.data.bookmarks = vec![test_bm("Prod Web", "10.0.1.10", "root", "Production")];
        state.rebuild_filtered_indices();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_bm = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Prod Web")));
        assert!(has_bm);
    }

    #[test]
    fn renders_filter_text() {
        let mut state = SshState::default();
        state.filter_text = "prod".into();
        state.filter_active = true;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_filter = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("filter: prod_")),
        );
        assert!(has_filter);
    }

    #[test]
    fn renders_detail_screen() {
        let mut state = SshState::default();
        state.screen = Screen::Detail;
        state.data.bookmarks = vec![test_bm("Prod Web", "10.0.1.10", "root", "Production")];
        state.detail_idx = Some(0);
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_border = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { ref title, .. } if title.as_deref().map(|t| t.contains("Edit Bookmark")).unwrap_or(false))
        });
        assert!(has_border);
    }

    #[test]
    fn renders_detail_edit_field_highlighted() {
        let mut state = SshState::default();
        state.screen = Screen::Detail;
        state.data.bookmarks = vec![test_bm("Prod Web", "10.0.1.10", "root", "Production")];
        state.detail_idx = Some(0);
        state.detail_edit_field = 0;
        state.editing = false;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let field_line = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.starts_with("Name")));
        assert!(field_line.is_some());
    }
}
