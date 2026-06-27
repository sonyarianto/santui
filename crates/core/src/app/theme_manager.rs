use crate::theme::{self, Theme};
use crate::widgets::filtered_list::{DisplayItem, FilteredListState};
use crate::widgets::popup::centered_rect;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Widget};
use ratatui::Frame;
use std::path::Path;

/// Manages theme selection, preview, and the theme-picker UI state.
#[derive(Debug)]
pub(crate) struct ThemeManager {
    /// All known themes: `(display_name, Theme)`.
    pub(super) themes: Vec<(String, Theme)>,
    /// Index into `themes` for the currently applied theme.
    pub(super) current_idx: usize,
    /// Theme picker state (filter, cursor, scroll).
    pub(super) picker_filter: Option<FilteredListState>,
    /// Theme index that was selected when the picker was opened,
    /// so Esc restores it.
    pub(super) picker_orig_idx: usize,
}

impl ThemeManager {
    pub(super) fn new() -> Self {
        let themes: Vec<(String, Theme)> = Theme::all()
            .into_iter()
            .map(|(n, t)| (n.to_string(), t))
            .collect();
        let current_idx = 1; // "Santui"
        ThemeManager {
            themes,
            current_idx,
            picker_filter: None,
            picker_orig_idx: 0,
        }
    }

    pub(super) fn current(&self) -> &Theme {
        &self.themes[self.current_idx].1
    }

    pub(super) fn select(&mut self, idx: usize) -> Theme {
        self.current_idx = idx;
        self.themes[idx].1.clone()
    }

    pub(super) fn preview(&mut self, idx: usize) -> Theme {
        self.current_idx = idx;
        self.themes[idx].1.clone()
    }

    pub(super) fn load_user_themes(&mut self, config_dir: &Path) {
        let user = theme::load_user_themes(config_dir);
        if user.is_empty() {
            return;
        }
        for (name, theme) in user {
            let lower = name.to_lowercase();
            if let Some(idx) = self
                .themes
                .iter()
                .position(|(n, _)| n.to_lowercase() == lower)
            {
                self.themes[idx] = (name, theme);
            } else {
                self.themes.push((name, theme));
            }
        }
    }

    /// Render the theme-picker overlay using ratatui widgets.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_picker(&mut self, f: &mut Frame, content: Rect, theme: &Theme, tick: u64) {
        // Clone theme names first to avoid borrow conflict with picker_filter.
        let theme_names: Vec<String> = self.themes.iter().map(|(n, _)| n.clone()).collect();

        let filter = match self.picker_filter.as_mut() {
            Some(f) => f,
            None => return,
        };

        if filter.is_dirty() {
            let items: Vec<DisplayItem> = theme_names
                .iter()
                .map(|name| DisplayItem {
                    category: "",
                    label: name.as_str(),
                })
                .collect();
            filter.set_query(filter.query.clone(), &items);
        }

        let min_w = 30;
        let ideal_w = 60;
        let title_h = 4;
        let footer_h = 3;
        let max_h = 20;
        let list_h = if filter.filtered_is_empty() {
            1
        } else {
            (filter.total_items() as u16).max(1)
        };
        let popup_h = (title_h + list_h + footer_h)
            .min(max_h)
            .min(content.height)
            .max(title_h + footer_h + 1);
        let popup_rect = centered_rect(content, min_w, ideal_w, popup_h);
        let inner_x = popup_rect.x.saturating_add(2);
        let inner_w = popup_rect.width.saturating_sub(4);

        render_picker_chrome(f.buffer_mut(), popup_rect, theme);

        // Header: title + search
        let header_area = Rect {
            x: inner_x,
            y: popup_rect.y + 1,
            width: inner_w,
            height: title_h,
        };
        render_picker_header(f.buffer_mut(), header_area, &filter.query, tick, theme);

        // Theme list
        let cursor = filter.cursor;
        let no_results = filter.filtered_is_empty();
        let filtered_indices = filter.filtered_indices().to_vec();

        // Build custom list items with ● for current theme
        let mut list_items: Vec<ListItem> = Vec::with_capacity(filtered_indices.len());
        if no_results {
            list_items.push(
                ListItem::new(Line::from(Span::styled(
                    "No results found",
                    Style::default().fg(theme.text_muted),
                )))
                .style(Style::default().fg(theme.text_muted)),
            );
        } else {
            for (flat, &theme_idx) in filtered_indices.iter().enumerate() {
                let (name, _) = &self.themes[theme_idx];
                let is_current = theme_idx == self.current_idx;
                let is_hovered = flat == cursor;
                let prefix = if is_current { " ● " } else { "   " };
                let text_fg = if is_hovered {
                    theme.inverted_text
                } else if is_current {
                    theme.accent
                } else {
                    theme.text
                };
                let mut item_style = Style::default().fg(text_fg);
                if is_hovered {
                    item_style = item_style.bg(theme.highlight).add_modifier(Modifier::BOLD);
                } else if is_current {
                    item_style = item_style.add_modifier(Modifier::BOLD);
                }
                let line = format!("{}{}", prefix, name);
                list_items.push(
                    ListItem::new(Line::from(Span::styled(line, item_style))).style(item_style),
                );
            }
        }

        let mut list_state = ListState::default();
        if !filtered_indices.is_empty() && cursor < filtered_indices.len() {
            list_state.select(Some(cursor));
        }

        let list_area = Rect {
            x: inner_x,
            y: header_area.bottom(),
            width: inner_w,
            height: popup_rect
                .bottom()
                .saturating_sub(header_area.bottom())
                .saturating_sub(footer_h),
        };
        if list_area.height > 0 {
            StatefulWidget::render(
                List::new(list_items)
                    .highlight_style(Style::default().fg(theme.inverted_text).bg(theme.highlight)),
                list_area,
                f.buffer_mut(),
                &mut list_state,
            );
        }

        // Footer: key hints
        let footer_area = Rect {
            x: inner_x,
            y: popup_rect.bottom().saturating_sub(footer_h),
            width: inner_w,
            height: footer_h,
        };
        render_picker_footer(f.buffer_mut(), footer_area, theme);
    }
}

fn render_picker_chrome(buf: &mut Buffer, popup_rect: Rect, theme: &Theme) {
    Clear.render(popup_rect, buf);
    Paragraph::new(vec![])
        .style(Style::default().bg(theme.background_panel))
        .render(popup_rect, buf);
}

fn render_picker_header(buf: &mut Buffer, area: Rect, query: &str, tick: u64, theme: &Theme) {
    let cursor_on = (tick / 5).is_multiple_of(2);

    let pad_w = area.width.saturating_sub(9) as usize;
    let title_spans = vec![
        Span::styled(
            "Themes",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ".repeat(pad_w), Style::default()),
        Span::styled("esc", Style::default().fg(theme.text_muted)),
    ];

    let input_line = if query.is_empty() {
        let first_style = if cursor_on {
            Style::default().fg(theme.inverted_text).bg(theme.highlight)
        } else {
            Style::default().fg(theme.text_muted)
        };
        Line::from(vec![
            Span::styled("S", first_style),
            Span::styled("earch", Style::default().fg(theme.text_muted)),
        ])
    } else {
        let cursor_style = if cursor_on {
            Style::default().fg(theme.inverted_text).bg(theme.highlight)
        } else {
            Style::default()
                .fg(theme.background_panel)
                .bg(theme.background_panel)
        };
        Line::from(vec![
            Span::styled(query.to_string(), Style::default().fg(theme.text)),
            Span::styled(" ", cursor_style),
        ])
    };

    let header_lines = vec![
        Line::from(title_spans),
        Line::from(""),
        input_line,
        Line::from(""),
    ];

    Paragraph::new(header_lines).render(area, buf);
}

fn render_picker_footer(buf: &mut Buffer, area: Rect, theme: &Theme) {
    let dim = Style::default().fg(theme.text_muted);
    let key = Style::default().fg(theme.text);
    let footer = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("↑↓", key),
            Span::styled(" navigate • ", dim),
            Span::styled("↵", key),
            Span::styled(" select", dim),
        ]),
        Line::from(""),
    ];
    Paragraph::new(footer).render(area, buf);
}
