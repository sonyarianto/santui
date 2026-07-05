use crate::state::{IptvState, PlaybackState, Screen};
use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};
use santui_ipc::ui;

pub const TABLE_TOP: u16 = 3;
pub const HEADER_H: u16 = 1;

const GAP: u16 = 1;

#[allow(clippy::too_many_arguments)]
fn draw_panel(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    title: &str,
    focused: bool,
    footer: Option<&[(&str, &str)]>,
) {
    if w < 3 || h < 2 {
        return;
    }
    let t = if focused {
        format!("{} \u{25cf}", title.trim())
    } else {
        title.trim().to_string()
    };
    cmds.push(RenderCmd::Border {
        x,
        y,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(t),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    border_type: None,
    });

    if let Some(hints) = footer {
        let max_chars = w.saturating_sub(3) as usize;
        let mut cx = x + 2;
        let footer_y = y + h - 2;
        let mut remaining = max_chars;
        for (i, (key, desc)) in hints.iter().enumerate() {
            if remaining == 0 {
                break;
            }
            if i > 0 {
                const SEP: &str = " \u{2022} ";
                let sep_w = SEP.chars().count();
                if sep_w <= remaining {
                    cmds.push(RenderCmd::Text {
                        x: cx,
                        y: footer_y,
                        text: SEP.into(),
                        fg: Some(theme.text_muted),
                        bg: None,
                        bold: false,
                    modifiers: 0,
                    });
                    cx += sep_w as u16;
                    remaining -= sep_w;
                }
            }
            if remaining == 0 {
                break;
            }
            let k: String = key.chars().take(remaining).collect();
            if !k.is_empty() {
                let kw = k.chars().count();
                cmds.push(RenderCmd::Text {
                    x: cx,
                    y: footer_y,
                    text: k,
                    fg: Some(theme.text),
                    bg: None,
                    bold: false,
                modifiers: 0,
                });
                cx += kw as u16;
                remaining -= kw;
            }
            if remaining == 0 {
                break;
            }
            if !desc.is_empty() {
                let desc_w = desc.chars().count();
                let space_needed = 1 + desc_w;
                if space_needed <= remaining {
                    let d: String = desc.chars().take(remaining - 1).collect();
                    let dw = d.chars().count();
                    let display = format!(" {d}");
                    cmds.push(RenderCmd::Text {
                        x: cx,
                        y: footer_y,
                        text: display,
                        fg: Some(theme.text_muted),
                        bg: None,
                        bold: false,
                    modifiers: 0,
                    });
                    cx += (1 + dw) as u16;
                    remaining -= 1 + dw;
                }
            }
        }
    }
}

pub fn render_ui(state: &IptvState, theme: &ThemeData, area_w: u16, area_h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    if area_w < 15 || area_h < 6 {
        cmds.push(RenderCmd::Text {
            x: 0,
            y: 0,
            text: "Window too small".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        modifiers: 0,
        });
        return cmds;
    }

    cmds.push(RenderCmd::Clear {
        x: 0,
        y: 0,
        w: area_w,
        h: area_h,
    });

    match state.screen {
        Screen::ChannelList | Screen::GroupFilter => {
            render_channel_list(&mut cmds, state, theme, area_w, area_h);
        }
        Screen::Search => {
            render_search(&mut cmds, state, theme, area_w, area_h);
        }
        Screen::PlaylistUrlEditor => {
            render_url_editor(&mut cmds, state, theme, area_w, area_h);
        }
    }

    cmds
}

fn render_channel_list(
    cmds: &mut Vec<RenderCmd>,
    state: &IptvState,
    theme: &ThemeData,
    area_w: u16,
    area_h: u16,
) {
    let info_h = state.info_h();
    let channels_h = area_h.saturating_sub(GAP + info_h);

    let stations_footer: &[(&str, &str)] = match &state.group_filter {
        Some(_) => &[
            ("j/k", "navigate"),
            ("enter", "play"),
            ("space", "fav"),
            ("/", "search"),
            ("x", "stop"),
            ("g", "all groups"),
            ("f", "fav only"),
            ("r", "refresh"),
        ],
        None => &[
            ("j/k", "navigate"),
            ("enter", "play"),
            ("space", "fav"),
            ("/", "search"),
            ("x", "stop"),
            ("g", "filter"),
            ("f", "fav only"),
            ("r", "refresh"),
        ],
    };
    let stations_footer_rows: u16 = if stations_footer.is_empty() { 0 } else { 2 };

    draw_panel(
        cmds,
        theme,
        0,
        0,
        area_w,
        channels_h,
        "Channels",
        matches!(state.screen, Screen::GroupFilter),
        Some(stations_footer),
    );

    let inner_w = area_w.saturating_sub(4) as usize;

    // Top line
    if let Some(ref msg) = state.scan_msg {
        let max_w = area_w.saturating_sub(4) as usize;
        let top_text = if msg.chars().count() > max_w {
            let truncated: String = msg.chars().take(max_w.saturating_sub(1)).collect();
            format!("{}\u{2026}", truncated)
        } else {
            msg.clone()
        };
        let top_x = area_w.saturating_sub(2u16.saturating_add(top_text.chars().count() as u16));
        cmds.push(RenderCmd::Text {
            x: top_x,
            y: 1,
            text: top_text,
            fg: Some(theme.accent),
            bg: None,
            bold: false,
        modifiers: 0,
        });
    } else if let Some(ref gf) = state.group_filter {
        let left_text = format!("Group: {}", gf);
        let right_text = format!("{}/{}", state.filtered.len(), state.channels.len());
        ui::text_at(
            cmds,
            2,
            1,
            &left_text,
            theme.accent,
            None,
            inner_w.saturating_sub(right_text.len() + 1) as u16,
        );
        let right_x = area_w.saturating_sub(2u16.saturating_add(right_text.len() as u16));
        cmds.push(RenderCmd::Text {
            x: right_x,
            y: 1,
            text: right_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        modifiers: 0,
        });
    } else if !state.query.is_empty() {
        let left_text = format!("Filter: \"{}\"", state.query);
        let right_text = format!("{}/{}", state.filtered.len(), state.channels.len());
        ui::text_at(
            cmds,
            2,
            1,
            &left_text,
            theme.accent,
            None,
            inner_w.saturating_sub(right_text.len() + 1) as u16,
        );
        let right_x = area_w.saturating_sub(2u16.saturating_add(right_text.len() as u16));
        cmds.push(RenderCmd::Text {
            x: right_x,
            y: 1,
            text: right_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        modifiers: 0,
        });
    } else {
        let fav_count = state.favorites_count();
        let top_text = if state.show_favorites_only {
            format!("\u{2665} {} favorites", state.filtered.len())
        } else if fav_count > 0 {
            format!(
                "Total channels: {}  \u{2665} {}",
                state.channels.len(),
                fav_count
            )
        } else {
            format!("Total channels: {}", state.channels.len())
        };
        let top_x = area_w.saturating_sub(2u16.saturating_add(top_text.chars().count() as u16));
        cmds.push(RenderCmd::Text {
            x: top_x,
            y: 1,
            text: top_text,
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        modifiers: 0,
        });
    }

    // Table
    let table_top = TABLE_TOP;
    let header_h = HEADER_H;
    let table_avail = channels_h.saturating_sub(table_top + header_h + 1 + stations_footer_rows);
    let max_visible = table_avail as usize;

    let scroll = state.scroll.min(state.filtered.len().saturating_sub(1));
    let visible_count = max_visible.min(state.filtered.len().saturating_sub(scroll));

    let name_w = ((inner_w - 2) * 45 / 100).max(10);
    let group_w = ((inner_w - 2) * 35 / 100).max(10);
    let country_w = inner_w.saturating_sub(2 + name_w + group_w);

    let mut rows: Vec<Vec<String>> = Vec::with_capacity(visible_count);
    for i in 0..visible_count {
        let ch_idx = state.filtered[scroll + i];
        let ch = &state.channels[ch_idx];
        let fav = if state.is_favorite(&ch.url) {
            "\u{2665} "
        } else {
            "  "
        };
        rows.push(vec![
            ui::truncate(&format!("{fav}{}", ch.name), name_w),
            ui::truncate(ch.group_title.as_deref().unwrap_or(""), group_w),
            ui::truncate("", country_w),
        ]);
    }

    let vis_selected = if state.selected >= scroll && state.selected < scroll + visible_count {
        Some(state.selected - scroll)
    } else {
        None
    };

    let current_row = match &state.play_state {
        PlaybackState::Playing { channel_index }
        | PlaybackState::Paused { channel_index }
        | PlaybackState::Buffering { channel_index } => {
            let idx = *channel_index;
            state.filtered[scroll..scroll + visible_count]
                .iter()
                .position(|&i| i == idx)
        }
        _ => None,
    };

    cmds.push(RenderCmd::Table {
        x: 2,
        y: table_top,
        w: inner_w as u16,
        h: (visible_count + 1).max(1) as u16,
        header: vec!["Name".into(), "Group".into(), "Country".into()],
        header_style: TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
        modifiers: 0,
        },
        rows,
        column_widths: vec![name_w as u16, group_w as u16, country_w as u16],
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
        current_row,
        current_style: Some(TextStyle {
            fg: Some(theme.success),
            bg: None,
            bold: false,
            modifiers: 0,
        }),
    cell_styles: None,
    });

    // Heart overlay for favorites
    let heart_red = [255, 60, 60];
    for i in 0..visible_count {
        let ch_idx = state.filtered[scroll + i];
        if state.is_favorite(&state.channels[ch_idx].url) {
            let bg = if vis_selected == Some(i) {
                Some(theme.highlight)
            } else {
                None
            };
            cmds.push(RenderCmd::Text {
                x: 2,
                y: table_top + 1 + i as u16,
                text: "\u{2665}".into(),
                fg: Some(heart_red),
                bg,
                bold: false,
            modifiers: 0,
            });
        }
    }

    // Now Playing panel (bottom)
    let np_y = channels_h + GAP;
    draw_panel(
        cmds, theme, 0, np_y, area_w, info_h, "Playback", false, None,
    );
    let vol_text = format!(" Vol: {}% ", state.volume);
    cmds.push(RenderCmd::Text {
        x: 5u16.saturating_add("Playback".len() as u16),
        y: np_y,
        text: vol_text,
        fg: Some(theme.text),
        bg: None,
        bold: false,
    modifiers: 0,
    });

    let r_inner_w = area_w.saturating_sub(4);

    match &state.play_state {
        PlaybackState::Stopped => {
            ui::text_at(
                cmds,
                2,
                np_y + 1,
                "No channel selected",
                theme.text_muted,
                None,
                r_inner_w,
            );
        }
        PlaybackState::Buffering { channel_index } => {
            let name = state
                .channels
                .get(*channel_index)
                .map(|c| c.name.as_str())
                .unwrap_or("Unknown");
            cmds.push(RenderCmd::Text {
                x: 2,
                y: np_y + 1,
                text: ui::truncate(name, r_inner_w as usize),
                fg: Some(theme.accent),
                bg: None,
                bold: true,
            modifiers: 0,
            });
            ui::text_at(
                cmds,
                2,
                np_y + 2,
                "Buffering...",
                theme.text_muted,
                None,
                r_inner_w,
            );
        }
        PlaybackState::Playing { channel_index } => {
            let name = state
                .channels
                .get(*channel_index)
                .map(|c| c.name.as_str())
                .unwrap_or("Unknown");
            cmds.push(RenderCmd::Text {
                x: 2,
                y: np_y + 1,
                text: ui::truncate(name, r_inner_w as usize),
                fg: Some(theme.success),
                bg: None,
                bold: true,
            modifiers: 0,
            });
            ui::text_at(
                cmds,
                2,
                np_y + 2,
                "\u{25b6} Playing",
                theme.text,
                None,
                r_inner_w,
            );
        }
        PlaybackState::Paused { channel_index } => {
            let name = state
                .channels
                .get(*channel_index)
                .map(|c| c.name.as_str())
                .unwrap_or("Unknown");
            cmds.push(RenderCmd::Text {
                x: 2,
                y: np_y + 1,
                text: ui::truncate(name, r_inner_w as usize),
                fg: Some(theme.accent),
                bg: None,
                bold: true,
            modifiers: 0,
            });
            ui::text_at(
                cmds,
                2,
                np_y + 2,
                "\u{23f8} Paused",
                theme.text_muted,
                None,
                r_inner_w,
            );
        }
        PlaybackState::Error(e) => {
            ui::text_at(
                cmds,
                2,
                np_y + 1,
                "\u{26a0} Error",
                theme.error,
                None,
                r_inner_w,
            );
            ui::text_at(cmds, 2, np_y + 2, e, theme.error, None, r_inner_w);
            let hint = "Press 'r' to refresh playlist or try another channel";
            ui::text_at(cmds, 2, np_y + 3, hint, theme.text_muted, None, r_inner_w);
        }
    }
}

fn render_search(
    cmds: &mut Vec<RenderCmd>,
    state: &IptvState,
    theme: &ThemeData,
    area_w: u16,
    area_h: u16,
) {
    cmds.push(RenderCmd::Clear {
        x: 0,
        y: 0,
        w: area_w,
        h: area_h,
    });

    let cursor = if state.tick_counter % 6 < 3 {
        '\u{2588}'
    } else {
        ' '
    };
    let search_text = format!("Search: {}{cursor}", state.query);
    let count_text = format!("{}/{}", state.filtered.len(), state.channels.len());

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: search_text,
        fg: Some(theme.accent),
        bg: None,
        bold: false,
    modifiers: 0,
    });
    let right_x = area_w.saturating_sub(2u16.saturating_add(count_text.len() as u16));
    cmds.push(RenderCmd::Text {
        x: right_x,
        y: 1,
        text: count_text,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    modifiers: 0,
    });

    let hint_y = area_h.saturating_sub(2);
    let hints = [("esc", "back"), ("enter", "play"), ("up/down", "navigate")];
    let mut cx = 2u16;
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            cmds.push(RenderCmd::Text {
                x: cx,
                y: hint_y,
                text: " \u{2022} ".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
            modifiers: 0,
            });
            cx += 3;
        }
        cmds.push(RenderCmd::Text {
            x: cx,
            y: hint_y,
            text: key.to_string(),
            fg: Some(theme.text),
            bg: None,
            bold: false,
        modifiers: 0,
        });
        cx += key.len() as u16;
        cmds.push(RenderCmd::Text {
            x: cx,
            y: hint_y,
            text: format!(" {desc}"),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        modifiers: 0,
        });
        cx += (1 + desc.len()) as u16;
    }

    // Preview list of results
    let inner_w = area_w.saturating_sub(4) as usize;
    let visible_h = area_h.saturating_sub(5) as usize;
    let scroll = state.scroll.min(state.filtered.len().saturating_sub(1));
    let visible_count = visible_h.min(state.filtered.len().saturating_sub(scroll));

    for i in 0..visible_count {
        let ch_idx = state.filtered[scroll + i];
        let ch = &state.channels[ch_idx];
        let fav = if state.is_favorite(&ch.url) {
            "\u{2665} "
        } else {
            "  "
        };
        let line = format!(
            "{fav}{}  [{}]",
            ch.name,
            ch.group_title.as_deref().unwrap_or("-")
        );
        let fg = if Some(i) == Some(state.selected.saturating_sub(scroll)) {
            theme.highlight
        } else {
            theme.text
        };
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 3 + i as u16,
            text: ui::truncate(&line, inner_w),
            fg: Some(fg),
            bg: None,
            bold: Some(i) == Some(state.selected.saturating_sub(scroll)),
        modifiers: 0,
        });
    }
}

fn render_url_editor(
    cmds: &mut Vec<RenderCmd>,
    state: &IptvState,
    theme: &ThemeData,
    area_w: u16,
    area_h: u16,
) {
    cmds.push(RenderCmd::Clear {
        x: 0,
        y: 0,
        w: area_w,
        h: area_h,
    });

    let inner_w = area_w.saturating_sub(4);

    draw_panel(
        cmds,
        theme,
        0,
        0,
        area_w,
        area_h.saturating_sub(2),
        "Playlist URL",
        true,
        None,
    );

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: "Enter playlist URL:".into(),
        fg: Some(theme.text),
        bg: None,
        bold: true,
    modifiers: 0,
    });

    let cursor = if state.tick_counter % 6 < 3 {
        '\u{2588}'
    } else {
        ' '
    };
    let before: String = state.url_edit.chars().take(state.url_edit_cursor).collect();
    let after: String = state.url_edit.chars().skip(state.url_edit_cursor).collect();
    let display = format!("{before}{cursor}{after}");
    let max_chars = inner_w as usize;
    let display_str: String = display.chars().take(max_chars).collect();

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: display_str,
        fg: Some(theme.accent),
        bg: None,
        bold: false,
    modifiers: 0,
    });

    let hint_text = "enter: save  esc: cancel  ctrl+b: default";
    cmds.push(RenderCmd::Text {
        x: 2,
        y: area_h.saturating_sub(1),
        text: hint_text.into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    modifiers: 0,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m3u::Channel;
    use crate::state::IptvState;
    use std::collections::BTreeMap;

    fn default_theme() -> ThemeData {
        ThemeData {
            text: [220, 220, 220],
            text_muted: [140, 140, 140],
            accent: [157, 124, 216],
            highlight: [250, 178, 131],
            logo: [255, 185, 0],
            background: [20, 20, 20],
            background_panel: [20, 20, 20],
            background_overlay: [10, 10, 10],
            border: [250, 178, 131],
            success: [127, 216, 143],
            error: [224, 108, 117],
            inverted_text: [20, 20, 20],
        }
    }

    #[test]
    fn small_area_shows_hint() {
        let state = IptvState::new();
        let cmds = render_ui(&state, &default_theme(), 10, 5);
        assert!(cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { text, .. } if text == "Window too small")));
    }

    #[test]
    fn channel_list_has_borders() {
        let mut state = IptvState::new();
        state.channels = vec![Channel {
            name: "Test".into(),
            url: "http://test".into(),
            tvg_id: None,
            tvg_name: None,
            tvg_logo: None,
            group_title: Some("News".into()),
            attrs: BTreeMap::new(),
        }];
        state.apply_filter();
        let cmds = render_ui(&state, &default_theme(), 80, 24);
        let borders: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        assert_eq!(borders.len(), 2); // Channels + Playback
    }

    #[test]
    fn search_screen_renders() {
        let mut state = IptvState::new();
        state.screen = Screen::Search;
        state.query = "news".into();
        state.channels = vec![Channel {
            name: "News 24".into(),
            url: "http://news".into(),
            tvg_id: None,
            tvg_name: None,
            tvg_logo: None,
            group_title: Some("News".into()),
            attrs: BTreeMap::new(),
        }];
        state.apply_filter();
        let cmds = render_ui(&state, &default_theme(), 80, 24);
        let has_search = cmds.iter().any(|c| {
            if let RenderCmd::Text { text, .. } = c {
                text.contains("Search: news")
            } else {
                false
            }
        });
        assert!(has_search);
    }

    #[test]
    fn url_editor_renders() {
        let mut state = IptvState::new();
        state.screen = Screen::PlaylistUrlEditor;
        state.url_edit = "https://example.com/playlist.m3u".into();
        let cmds = render_ui(&state, &default_theme(), 80, 24);
        let has_url = cmds.iter().any(|c| {
            if let RenderCmd::Text { text, .. } = c {
                text.contains("example.com")
            } else {
                false
            }
        });
        assert!(has_url);
    }

    #[test]
    fn group_filter_screen_renders() {
        let mut state = IptvState::new();
        state.screen = Screen::GroupFilter;
        state.group_filter = Some("News".into());
        state.channels = vec![Channel {
            name: "News 24".into(),
            url: "http://news".into(),
            tvg_id: None,
            tvg_name: None,
            tvg_logo: None,
            group_title: Some("News".into()),
            attrs: BTreeMap::new(),
        }];
        state.apply_filter();
        let cmds = render_ui(&state, &default_theme(), 80, 24);
        let has_group = cmds.iter().any(|c| {
            if let RenderCmd::Text { text, .. } = c {
                text == "Group: News"
            } else {
                false
            }
        });
        assert!(has_group);
    }
}
