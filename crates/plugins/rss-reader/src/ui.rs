use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};

use crate::state::{FetchStatus, RssState, Screen};

fn time_ago(published: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let diff = now.saturating_sub(published);
    if diff < 60 {
        "just now".into()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

pub fn render_ui(state: &RssState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];

    match &state.screen {
        Screen::FeedList => render_feed_list(&mut cmds, state, theme, w, h),
        Screen::ItemList(feed_url) => render_item_list(&mut cmds, state, theme, w, h, feed_url),
        Screen::ItemView(idx) => render_item_view(&mut cmds, state, theme, w, h, *idx),
        Screen::AddFeed => {
            render_feed_list(&mut cmds, state, theme, w, h);
            render_add_feed(&mut cmds, state, theme, w, h);
        }
        Screen::ConfirmRemoveFeed(idx) => {
            render_feed_list(&mut cmds, state, theme, w, h);
            render_confirm_remove(&mut cmds, state, theme, w, h, *idx);
        }
    }

    cmds
}

fn render_feed_list(
    cmds: &mut Vec<RenderCmd>,
    state: &RssState,
    theme: &ThemeData,
    w: u16,
    h: u16,
) {
    let total = state.total_unread();
    let title = if total > 0 {
        format!("RSS Reader — {total} unread")
    } else {
        "RSS Reader".into()
    };

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

    let mut row = 1u16;

    if matches!(state.fetch_status, FetchStatus::Fetching(_)) {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: row,
            text: "↻ Fetching...".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
        row += 1;
    }

    let all_label = if total > 0 {
        format!("  ▶ All feeds  {total} unread")
    } else {
        "  ▶ All feeds".into()
    };
    let is_all_selected = state.feed_cursor == 0;
    cmds.push(RenderCmd::Text {
        x: 2,
        y: row,
        text: all_label,
        fg: Some(if is_all_selected {
            theme.inverted_text
        } else {
            theme.accent
        }),
        bg: if is_all_selected {
            Some(theme.highlight)
        } else {
            None
        },
        bold: true,
    });
    row += 1;

    for (i, feed) in state.data.feeds.iter().enumerate() {
        let unread = state.unread_count(&feed.url);
        let label = if unread > 0 {
            format!("    {}  {unread} unread", feed.title)
        } else {
            format!("    {}", feed.title)
        };
        let is_selected = state.feed_cursor == i + 1;
        let fg = if unread > 0 {
            theme.accent
        } else {
            theme.text_muted
        };
        cmds.push(RenderCmd::Text {
            x: 2,
            y: row,
            text: label,
            fg: Some(if is_selected { theme.inverted_text } else { fg }),
            bg: if is_selected {
                Some(theme.highlight)
            } else {
                None
            },
            bold: is_selected || unread > 0,
        });
        row += 1;
    }
}

fn render_item_list(
    cmds: &mut Vec<RenderCmd>,
    state: &RssState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    feed_url: &Option<String>,
) {
    let feed_name = match feed_url {
        Some(url) => state
            .data
            .feeds
            .iter()
            .find(|f| &f.url == url)
            .map(|f| f.title.as_str())
            .unwrap_or("Items"),
        None => "All Items",
    };
    let total = state.total_unread();
    let title = if total > 0 {
        format!("{feed_name} — {total} unread")
    } else {
        feed_name.to_string()
    };

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

    let max_w = w.saturating_sub(4) as usize;
    for (i, item) in state.current_items.iter().enumerate() {
        let y = 1 + i as u16;
        if y >= h.saturating_sub(1) {
            break;
        }
        let bullet = if item.is_read { "○" } else { "●" };
        let bullet_fg = if item.is_read {
            theme.text_muted
        } else {
            theme.text
        };
        let time_str = item.item.published.map(time_ago).unwrap_or_default();
        let time_w = time_str.len();
        let feed_label = if feed_url.is_none() {
            format!("  {}", item.feed_title)
        } else {
            String::new()
        };
        let feed_w = feed_label.chars().count();
        let title_w = max_w.saturating_sub(2 + time_w + feed_w);
        let truncated_title = if title_w > 1 {
            let t: String = item
                .item
                .title
                .chars()
                .take(title_w.saturating_sub(1))
                .collect();
            if title_w <= item.item.title.chars().count() {
                format!("{t}…")
            } else {
                t
            }
        } else {
            String::new()
        };
        let display = format!(" {bullet} {truncated_title}{feed_label}{time_str}");
        let is_selected = i == state.item_cursor;
        cmds.push(RenderCmd::Text {
            x: 2,
            y,
            text: display,
            fg: Some(if is_selected {
                theme.inverted_text
            } else {
                bullet_fg
            }),
            bg: if is_selected {
                Some(theme.highlight)
            } else {
                None
            },
            bold: is_selected || !item.is_read,
        });
    }
}

fn render_item_view(
    cmds: &mut Vec<RenderCmd>,
    state: &RssState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    idx: usize,
) {
    if idx >= state.current_items.len() {
        return;
    }
    let item = &state.current_items[idx];

    let title_str = if w > 5 {
        let max = (w - 4) as usize;
        let t: String = item
            .item
            .title
            .chars()
            .take(max.saturating_sub(1))
            .collect();
        if max <= item.item.title.chars().count() {
            format!("{t}…")
        } else {
            t
        }
    } else {
        item.item.title.clone()
    };

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title_str),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    let mut row = 1u16;

    if let Some(ref url) = item.item.url {
        let url_w = (w.saturating_sub(4)) as usize;
        let display_url: String = if url_w > 1 {
            let t: String = url.chars().take(url_w.saturating_sub(1)).collect();
            if url_w <= url.chars().count() {
                format!("{t}…")
            } else {
                url.clone()
            }
        } else {
            url.clone()
        };
        cmds.push(RenderCmd::Text {
            x: 2,
            y: row,
            text: display_url,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
        row += 1;
    }

    if let Some(pub_time) = item.item.published {
        let time_str = time_ago(pub_time);
        cmds.push(RenderCmd::Text {
            x: w.saturating_sub(time_str.len() as u16 + 2),
            y: 0,
            text: time_str,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
    }

    let para_h = h.saturating_sub(row + 1);
    if para_h > 0 {
        cmds.push(RenderCmd::Paragraph {
            x: 2,
            y: row,
            w: w.saturating_sub(4),
            h: para_h,
            text: item.item.summary.clone(),
            style: TextStyle {
                fg: Some(theme.text),
                bg: None,
                bold: false,
            },
            wrap: true,
        });
    }
}

fn render_add_feed(cmds: &mut Vec<RenderCmd>, state: &RssState, theme: &ThemeData, w: u16, h: u16) {
    let popup_w = 60u16.min(w.saturating_sub(4));
    let popup_h = 6;
    let popup_x = (w - popup_w) / 2;
    let popup_y = (h - popup_h) / 2;

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
        bg: Some(theme.background_panel),
        borders: BORDER_ALL,
        title: Some("Add Feed".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: popup_y + 1,
        text: format!("URL: {}", state.add_url_buf),
        fg: Some(theme.text),
        bg: None,
        bold: false,
    });
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: popup_y + 3,
        text: "enter add   esc cancel".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });
}

fn render_confirm_remove(
    cmds: &mut Vec<RenderCmd>,
    state: &RssState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    idx: usize,
) {
    let feed_name = state
        .data
        .feeds
        .get(idx)
        .map(|f| f.title.as_str())
        .unwrap_or("this feed");
    let popup_w = 50u16.min(w.saturating_sub(4));
    let popup_h = 7;
    let popup_x = (w - popup_w) / 2;
    let popup_y = (h - popup_h) / 2;

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
        bg: Some(theme.background_panel),
        borders: BORDER_ALL,
        title: Some("Remove Feed?".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: popup_y + 1,
        text: format!("Remove \"{feed_name}\"?"),
        fg: Some(theme.text),
        bg: None,
        bold: false,
    });
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: popup_y + 2,
        text: "All its items will be removed from your feed.".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: popup_y + 4,
        text: "y confirm    n / esc cancel".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetcher::FeedItem;
    use crate::state::RssState;

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

    fn state_with_items() -> RssState {
        let mut state = RssState::new();
        state.data.feeds.push(crate::state::Feed {
            url: "http://feed1".into(),
            title: "Feed 1".into(),
            last_fetched: None,
        });
        state.data.feeds.push(crate::state::Feed {
            url: "http://feed2".into(),
            title: "Feed 2".into(),
            last_fetched: None,
        });
        state.fetch_status = FetchStatus::Idle;
        state
    }

    #[test]
    fn renders_feed_list() {
        let state = state_with_items();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_border = cmds.iter().any(|c| matches!(c, RenderCmd::Border { title, .. } if title.as_deref() == Some("RSS Reader")));
        assert!(has_border);
    }

    #[test]
    fn renders_all_feeds_row_first() {
        let state = state_with_items();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let texts: Vec<&String> = cmds
            .iter()
            .filter_map(|c| {
                if let RenderCmd::Text { text, .. } = c {
                    Some(text)
                } else {
                    None
                }
            })
            .collect();
        let all_pos = texts.iter().position(|t| t.contains("All feeds"));
        let feed1_pos = texts.iter().position(|t| t.contains("Feed 1"));
        assert!(all_pos.is_some());
        assert!(feed1_pos.is_some());
        assert!(all_pos.unwrap() < feed1_pos.unwrap());
    }

    #[test]
    fn renders_unread_count_per_feed() {
        let mut state = state_with_items();
        state.apply_feed_items(
            "http://feed1",
            vec![
                FeedItem {
                    id: "a".into(),
                    title: "A".into(),
                    summary: "s".into(),
                    url: None,
                    published: Some(100),
                },
                FeedItem {
                    id: "b".into(),
                    title: "B".into(),
                    summary: "s".into(),
                    url: None,
                    published: Some(200),
                },
            ],
            None,
        );
        state.mark_read("a");
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_unread = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("1 unread")));
        assert!(has_unread);
    }

    #[test]
    fn renders_item_list() {
        let mut state = state_with_items();
        state.apply_feed_items(
            "http://feed1",
            vec![FeedItem {
                id: "a".into(),
                title: "Article A".into(),
                summary: "s".into(),
                url: None,
                published: Some(100),
            }],
            None,
        );
        state.screen = Screen::ItemList(Some("http://feed1".into()));
        state.rebuild_current_items(&state.screen.clone());
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_item = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Article A")));
        assert!(has_item);
    }

    #[test]
    fn renders_unread_bullet() {
        let mut state = state_with_items();
        state.apply_feed_items(
            "http://feed1",
            vec![FeedItem {
                id: "a".into(),
                title: "A".into(),
                summary: "s".into(),
                url: None,
                published: Some(100),
            }],
            None,
        );
        state.screen = Screen::ItemList(Some("http://feed1".into()));
        state.rebuild_current_items(&state.screen.clone());
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_bullet = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains('●')));
        assert!(has_bullet);
    }

    #[test]
    fn renders_read_bullet_muted() {
        let mut state = state_with_items();
        state.apply_feed_items(
            "http://feed1",
            vec![FeedItem {
                id: "a".into(),
                title: "A".into(),
                summary: "s".into(),
                url: None,
                published: Some(100),
            }],
            None,
        );
        state.mark_read("a");
        state.screen = Screen::ItemList(Some("http://feed1".into()));
        state.rebuild_current_items(&state.screen.clone());
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_bullet = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains('○')));
        assert!(has_bullet);
    }

    #[test]
    fn renders_item_view_body() {
        let mut state = state_with_items();
        state.apply_feed_items(
            "http://feed1",
            vec![FeedItem {
                id: "a".into(),
                title: "Title".into(),
                summary: "Article body".into(),
                url: Some("http://url".into()),
                published: Some(100),
            }],
            None,
        );
        state.screen = Screen::ItemList(Some("http://feed1".into()));
        state.rebuild_current_items(&state.screen.clone());
        state.screen = Screen::ItemView(0);
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_paragraph = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Paragraph { text, .. } if text == "Article body"));
        assert!(has_paragraph);
    }

    #[test]
    fn renders_add_feed_overlay() {
        let mut state = state_with_items();
        state.screen = Screen::AddFeed;
        state.add_url_buf = "http://example".into();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_border = cmds.iter().any(|c| matches!(c, RenderCmd::Border { title, .. } if title.as_deref() == Some("Add Feed")));
        assert!(has_border);
        let has_url = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("http://example")));
        assert!(has_url);
    }

    #[test]
    fn renders_confirm_remove_overlay() {
        let mut state = state_with_items();
        state.screen = Screen::ConfirmRemoveFeed(0);
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_border = cmds.iter().any(|c| matches!(c, RenderCmd::Border { title, .. } if title.as_deref() == Some("Remove Feed?")));
        assert!(has_border);
    }

    #[test]
    fn renders_fetching_indicator() {
        let mut state = state_with_items();
        state.fetch_status = FetchStatus::Fetching(2);
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_fetching = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text.contains("Fetching")));
        assert!(has_fetching);
    }
}
