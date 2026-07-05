use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};

use crate::api::{domain_from_url, strip_html, time_ago, HnItem};
use crate::state::{FetchState, HnState, Screen};

pub fn render_ui(state: &HnState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];

    match &state.screen {
        Screen::StoryList => cmds.extend(render_story_list(state, theme, w, h)),
        Screen::Comments => cmds.extend(render_comments(state, theme, w, h)),
    }

    cmds
}

fn render_story_list(state: &HnState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(format!("Hacker News \u{2014} {}", state.category.label())),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let inner_w = w.saturating_sub(4) as usize;
    let inner_h = h.saturating_sub(3) as usize;

    match &state.fetch_state {
        FetchState::FetchingIds | FetchState::FetchingStories => {
            cmds.push(RenderCmd::Text {
                x: w / 2 - 12,
                y: h / 2,
                text: "\u{27F3} Fetching stories...".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            cmds.push(RenderCmd::Text {
                x: 1,
                y: h.saturating_sub(1),
                text: " t=top  n=new  b=best  r=refresh  enter=comments  o=open".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            return cmds;
        }
        FetchState::Error(e) => {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: h / 2,
                text: format!("\u{26A0} {}", e),
                fg: Some(theme.error),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            cmds.push(RenderCmd::Text {
                x: 1,
                y: h.saturating_sub(1),
                text: " t=top  n=new  b=best  r=refresh  enter=comments  o=open".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            return cmds;
        }
        _ => {}
    }

    if state.stories.is_empty() && matches!(state.fetch_state, FetchState::Done) {
        cmds.push(RenderCmd::Text {
            x: w / 2 - 10,
            y: h / 2,
            text: "No stories loaded.".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 1,
            y: h.saturating_sub(1),
            text: " t=top  n=new  b=best  r=refresh  enter=comments  o=open".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        return cmds;
    }

    let max_visible = inner_h;
    let start = state.scroll;
    let visible = state
        .stories
        .iter()
        .enumerate()
        .skip(start)
        .take(max_visible);

    for (i, story) in visible {
        let y = 1 + (i - start) as u16;
        let is_selected = i == state.selected;

        let rank = i + 1;
        let title = story.title.as_deref().unwrap_or("[untitled]");
        let score = story.score.unwrap_or(0);
        let author = story.by.as_deref().unwrap_or("unknown");
        let comments = story.descendants.unwrap_or(0);
        let domain = story.url.as_deref().and_then(domain_from_url);
        let ago = story.time.map(time_ago).unwrap_or_default();

        let arrow = if is_selected { "\u{25B6}" } else { " " };

        let first_line = format!(
            "{} {:>3}. {}",
            arrow,
            rank,
            santui_ipc::ui::truncate(title, inner_w.saturating_sub(6))
        );
        cmds.push(RenderCmd::Text {
            x: 2,
            y,
            text: first_line,
            fg: if is_selected {
                Some(theme.highlight)
            } else {
                Some(theme.text)
            },
            bg: if is_selected {
                Some(theme.background_panel)
            } else {
                None
            },
            bold: is_selected,
            modifiers: 0,
        });

        let domain_part = domain
            .as_deref()
            .map(|d| format!(" ({})", d))
            .unwrap_or_default();

        let second_line = format!(
            "     {} points by {} | {} comments | {}{}",
            score, author, comments, ago, domain_part
        );
        let truncated = santui_ipc::ui::truncate(&second_line, inner_w);
        cmds.push(RenderCmd::Text {
            x: 2,
            y: y + 1,
            text: truncated,
            fg: Some(theme.text_muted),
            bg: if is_selected {
                Some(theme.background_panel)
            } else {
                None
            },
            bold: false,
            modifiers: 0,
        });
    }

    cmds.push(RenderCmd::Text {
        x: 1,
        y: h.saturating_sub(1),
        text: " t=top  n=new  b=best  r=refresh  enter=comments  o=open  tab=cycle".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

fn render_comments(state: &HnState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let story_title = state
        .comment_story
        .as_ref()
        .and_then(|s| s.title.as_deref())
        .unwrap_or("Hacker News");

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(format!("Hacker News \u{2014} {}", story_title)),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let inner_w = w.saturating_sub(4) as usize;

    if let Some(ref story) = state.comment_story {
        let score = story.score.unwrap_or(0);
        let author = story.by.as_deref().unwrap_or("unknown");
        let comments = story.descendants.unwrap_or(0);
        let domain = story
            .url
            .as_deref()
            .and_then(domain_from_url)
            .unwrap_or_default();

        let header = format!(
            "{} {} points by {} \u{00B7} {} comments \u{00B7} {}",
            "\u{25B2}", score, author, comments, domain
        );
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: santui_ipc::ui::truncate(&header, inner_w),
            fg: Some(theme.text),
            bg: None,
            bold: true,
            modifiers: 0,
        });
    }

    match &state.fetch_state {
        FetchState::FetchingComments => {
            cmds.push(RenderCmd::Text {
                x: w / 2 - 14,
                y: h / 2,
                text: "\u{27F3} Fetching comments...".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            cmds.push(RenderCmd::Text {
                x: 1,
                y: h.saturating_sub(1),
                text: " \u{2191}\u{2193} navigate  esc back  o open story link".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            return cmds;
        }
        FetchState::Error(e) => {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: h / 2,
                text: format!("\u{26A0} {}", e),
                fg: Some(theme.error),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            cmds.push(RenderCmd::Text {
                x: 1,
                y: h.saturating_sub(1),
                text: " \u{2191}\u{2193} navigate  esc back  o open story link".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            return cmds;
        }
        _ => {}
    }

    let max_visible = h.saturating_sub(4) as usize;
    if state.comments.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 3,
            text: "No comments yet.".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else {
        let tree = build_comment_tree(&state.comments, &state.comment_ids());
        let flat = flatten_comment_tree(&tree, 0);

        let start = state.scroll.min(flat.len().saturating_sub(1));
        let visible = flat.iter().skip(start).take(max_visible);

        let mut row = 2u16;
        for node in visible {
            if row >= h.saturating_sub(2) {
                break;
            }

            let depth = node.depth.min(5);
            let indent = depth * 2;

            if node.deleted {
                let prefix = format!("{:indent$}{}", "", "[deleted]", indent = indent);
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: row,
                    text: santui_ipc::ui::truncate(&prefix, inner_w.saturating_sub(2)),
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
                row += 1;
                continue;
            }

            let tree_prefix = if depth > 0 {
                let mut p = String::new();
                for d in 0..depth {
                    if d == depth - 1 {
                        p.push('\u{251C}');
                    } else {
                        p.push('\u{2502}');
                        if d > 0 {
                            p.push(' ');
                        }
                    }
                }
                p
            } else {
                String::new()
            };

            let author = node.by.as_deref().unwrap_or("unknown");
            let score = node.score.unwrap_or(0);
            let ago = node.time.map(time_ago).unwrap_or_default();
            let text = node.text.as_deref().map(strip_html).unwrap_or_default();

            let header = format!(
                "{}{} {} {} {} {}",
                if indent > 0 { &tree_prefix } else { "" },
                if indent == 0 { "" } else { " " },
                author,
                "\u{25B2}",
                score,
                ago,
            );
            cmds.push(RenderCmd::Text {
                x: 2,
                y: row,
                text: santui_ipc::ui::truncate(&header, inner_w.saturating_sub(2)),
                fg: Some(theme.accent),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            row += 1;

            if row >= h.saturating_sub(2) {
                break;
            }

            let text_indent = indent + 2;
            for text_line in text.lines() {
                if row >= h.saturating_sub(2) {
                    break;
                }
                let line = format!("{:indent$}{}", "", text_line, indent = text_indent);
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: row,
                    text: santui_ipc::ui::truncate(&line, inner_w.saturating_sub(2)),
                    fg: Some(theme.text),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
                row += 1;
            }

            if node.children_count > 0 && depth >= 5 {
                cmds.push(RenderCmd::Text {
                    x: 2 + (text_indent as u16),
                    y: row,
                    text: format!("... ({} more replies)", node.children_count),
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
                row += 1;
            }
        }
    }

    cmds.push(RenderCmd::Text {
        x: 1,
        y: h.saturating_sub(1),
        text: " \u{2191}\u{2193} navigate  esc back  o open story link".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

struct CommentNode {
    #[allow(dead_code)]
    id: u32,
    depth: usize,
    by: Option<String>,
    time: Option<u64>,
    score: Option<u32>,
    text: Option<String>,
    deleted: bool,
    children: Vec<CommentNode>,
    children_count: usize,
}

fn build_comment_tree(comments: &[HnItem], root_ids: &[u32]) -> Vec<CommentNode> {
    let comment_map: std::collections::HashMap<u32, &HnItem> =
        comments.iter().map(|c| (c.id, c)).collect();

    root_ids
        .iter()
        .filter_map(|id| build_node(*id, &comment_map, 0))
        .collect()
}

fn build_node(
    id: u32,
    map: &std::collections::HashMap<u32, &HnItem>,
    depth: usize,
) -> Option<CommentNode> {
    let item = map.get(&id)?;
    if item.deleted == Some(true) && item.text.is_none() {
        return Some(CommentNode {
            id,
            depth,
            by: None,
            time: None,
            score: None,
            text: None,
            deleted: true,
            children: Vec::new(),
            children_count: 0,
        });
    }
    let children: Vec<CommentNode> = item
        .kids
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter_map(|kid_id| build_node(*kid_id, map, depth + 1))
        .collect();
    let children_count = children.len();
    Some(CommentNode {
        id,
        depth,
        by: item.by.clone(),
        time: item.time,
        score: item.score,
        text: item.text.clone(),
        deleted: item.deleted.unwrap_or(false) || item.dead.unwrap_or(false),
        children,
        children_count,
    })
}

fn flatten_comment_tree(nodes: &[CommentNode], depth: usize) -> Vec<&CommentNode> {
    let mut result = Vec::new();
    for node in nodes {
        result.push(node);
        if depth < 5 {
            result.extend(flatten_comment_tree(&node.children, depth + 1));
        }
    }
    result
}

impl HnState {
    fn comment_ids(&self) -> Vec<u32> {
        self.comment_story
            .as_ref()
            .and_then(|s| s.kids.clone())
            .unwrap_or_default()
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
    fn renders_empty_list() {
        let mut state = HnState::default();
        state.fetch_state = FetchState::Done;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_empty = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("No stories loaded")),
        );
        assert!(has_empty);
    }

    #[test]
    fn renders_story_list() {
        let mut state = HnState::default();
        state.fetch_state = FetchState::Done;
        state.stories = vec![
            HnItem {
                id: 1,
                item_type: crate::api::HnItemType::Story,
                by: Some("user1".into()),
                time: Some(1710000000),
                title: Some("Test Story".into()),
                text: None,
                url: Some("https://example.com".into()),
                score: Some(100),
                descendants: Some(42),
                kids: None,
                parent: None,
                deleted: None,
                dead: None,
            },
            HnItem {
                id: 2,
                item_type: crate::api::HnItemType::Story,
                by: Some("user2".into()),
                time: Some(1710000000),
                title: Some("Another Story".into()),
                text: None,
                url: None,
                score: Some(50),
                descendants: Some(10),
                kids: None,
                parent: None,
                deleted: None,
                dead: None,
            },
        ];
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_first = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Test Story")));
        assert!(has_first);
        let has_second = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Another Story")),
        );
        assert!(has_second);
    }

    #[test]
    fn renders_fetching_indicator() {
        let mut state = HnState::default();
        state.fetch_state = FetchState::FetchingStories;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_fetching = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Fetching")));
        assert!(has_fetching);
    }

    #[test]
    fn renders_comments_screen() {
        let mut state = HnState::default();
        state.screen = Screen::Comments;
        state.comment_story = Some(HnItem {
            id: 1,
            item_type: crate::api::HnItemType::Story,
            by: Some("author".into()),
            time: Some(1710000000),
            title: Some("Story Title".into()),
            text: None,
            url: Some("https://example.com".into()),
            score: Some(100),
            descendants: Some(3),
            kids: Some(vec![10, 20]),
            parent: None,
            deleted: None,
            dead: None,
        });
        state.fetch_state = FetchState::Done;
        state.comments = vec![
            HnItem {
                id: 10,
                item_type: crate::api::HnItemType::Comment,
                by: Some("commenter1".into()),
                time: Some(1710000100),
                text: Some("Great post!".into()),
                score: Some(15),
                kids: None,
                parent: Some(1),
                descendants: None,
                title: None,
                url: None,
                deleted: None,
                dead: None,
            },
            HnItem {
                id: 20,
                item_type: crate::api::HnItemType::Comment,
                by: Some("commenter2".into()),
                time: Some(1710000200),
                text: Some("Thanks!".into()),
                score: Some(5),
                kids: None,
                parent: Some(1),
                descendants: None,
                title: None,
                url: None,
                deleted: None,
                dead: None,
            },
        ];
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_comment1 = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("commenter1")));
        assert!(has_comment1);
        let has_comment2 = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("commenter2")));
        assert!(has_comment2);
    }

    #[test]
    fn renders_deleted_comment() {
        let mut state = HnState::default();
        state.screen = Screen::Comments;
        state.comment_story = Some(HnItem {
            id: 1,
            item_type: crate::api::HnItemType::Story,
            by: Some("author".into()),
            time: Some(1710000000),
            title: Some("Story".into()),
            text: None,
            url: None,
            score: Some(10),
            descendants: Some(1),
            kids: Some(vec![10]),
            parent: None,
            deleted: None,
            dead: None,
        });
        state.fetch_state = FetchState::Done;
        state.comments = vec![HnItem {
            id: 10,
            item_type: crate::api::HnItemType::Comment,
            by: None,
            time: None,
            text: None,
            score: None,
            kids: None,
            parent: Some(1),
            descendants: None,
            title: None,
            url: None,
            deleted: Some(true),
            dead: None,
        }];
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_deleted = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("[deleted]")));
        assert!(has_deleted);
    }
}
