use crate::theme::Theme;
use ratatui::Frame;

/// Manages theme selection, preview, and the theme-picker UI state.
///
/// Owns the full theme list, the currently selected index, and all
/// theme-picker interaction state (query, cursor, scroll).  The parent
/// [`Santui`](super::Santui) keeps a cached `theme: Theme` that is synced
/// via calls to [`ThemeManager::select`] and [`ThemeManager::preview`].
#[derive(Debug)]
pub(crate) struct ThemeManager {
    /// All known themes: `(display_name, Theme)`.
    pub(super) themes: Vec<(&'static str, Theme)>,
    /// Index into `themes` for the currently applied theme.
    pub(super) current_idx: usize,
    /// The currently applied theme (same as `themes[current_idx].1`).
    pub(super) current: Theme,
    /// Whether the theme picker overlay is open.
    pub(super) picker_open: bool,
    /// Search text typed into the theme picker.
    pub(super) picker_query: String,
    /// Flat-list cursor inside the theme picker.
    pub(super) picker_cursor: usize,
    /// Scroll offset (lines) inside the theme picker.
    pub(super) picker_scroll: u16,
    /// Theme index that was selected when the picker was opened,
    /// so Esc restores it.
    pub(super) picker_orig_idx: usize,
}

impl ThemeManager {
    pub(super) fn new() -> Self {
        let themes = Theme::all();
        let current_idx = 1; // "Santui"
        let current = themes[current_idx].1.clone();
        ThemeManager {
            themes,
            current_idx,
            current,
            picker_open: false,
            picker_query: String::new(),
            picker_cursor: 0,
            picker_scroll: 0,
            picker_orig_idx: 0,
        }
    }

    /// Apply the theme at `idx` and return a reference to the new theme.
    pub(super) fn select(&mut self, idx: usize) -> &Theme {
        self.current_idx = idx;
        self.current = self.themes[idx].1.clone();
        &self.current
    }

    /// Preview a theme without permanently selecting it.
    /// Returns a reference to the previewed theme.
    pub(super) fn preview(&mut self, idx: usize) -> &Theme {
        self.current_idx = idx;
        self.current = self.themes[idx].1.clone();
        &self.current
    }

    /// Return the indices of themes matching the current picker query.
    pub(super) fn filtered(&self) -> Vec<usize> {
        if self.picker_query.is_empty() {
            return (0..self.themes.len()).collect();
        }
        let q = self.picker_query.to_lowercase();
        self.themes
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    /// Adjust scroll so that the cursor is visible in the picker list.
    pub(super) fn ensure_cursor_visible(&mut self, content_h: u16) {
        let list_h = super::max_list_h(content_h);
        let cursor = self.picker_cursor as u16;
        if cursor < self.picker_scroll {
            self.picker_scroll = cursor;
        } else if cursor >= self.picker_scroll + list_h {
            self.picker_scroll = cursor.saturating_sub(list_h.saturating_sub(1));
        }
    }

    /// Render the theme-picker overlay.
    pub(super) fn render_picker(&self, f: &mut Frame, content: ratatui::layout::Rect, tick: u64) {
        let t = &self.current;
        let filtered = self.filtered();
        let cursor = self.picker_cursor;

        let pw = super::pal_w(content.width);
        let inner_w = pw.saturating_sub(super::PAD_L * 2);

        let no_results = !self.picker_query.is_empty() && filtered.is_empty();
        let list_items = if no_results { 1 } else { filtered.len() };

        let max_visible = super::max_list_h(content.height);
        let ideal_pal = super::PAD_T + super::HEADER_H + max_visible + super::PAD_B;
        let pal_h = ideal_pal
            .max(super::PAD_T + super::HEADER_H + super::PAD_B + 1)
            .min(content.height);
        let max_list = pal_h.saturating_sub(super::PAD_T + super::HEADER_H + super::PAD_B);
        let list_h = (list_items as u16).min(max_list).max(1);

        let x = (content.width.saturating_sub(pw)) / 2;
        let y = content.y + (content.height.saturating_sub(pal_h)) / 2;
        let pal_area = ratatui::layout::Rect {
            x,
            y,
            width: pw,
            height: pal_h,
        };

        f.render_widget(ratatui::widgets::Clear, pal_area);
        f.render_widget(
            ratatui::widgets::Paragraph::new(vec![])
                .style(ratatui::style::Style::default().bg(t.background_panel)),
            pal_area,
        );

        let pad_w = inner_w.saturating_sub(9);
        let mut title_spans = vec![ratatui::text::Span::styled(
            "Themes",
            ratatui::style::Style::default()
                .fg(t.text)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )];
        if pad_w > 0 {
            title_spans.push(ratatui::text::Span::styled(
                " ".repeat(pad_w as usize),
                ratatui::style::Style::default(),
            ));
        }
        title_spans.push(ratatui::text::Span::styled(
            "esc",
            ratatui::style::Style::default().fg(t.text_muted),
        ));

        let cursor_on = (tick / 5).is_multiple_of(2);

        let input_line = if self.picker_query.is_empty() {
            let first_style = if cursor_on {
                ratatui::style::Style::default()
                    .fg(t.inverted_text)
                    .bg(t.highlight)
            } else {
                ratatui::style::Style::default().fg(t.text_muted)
            };
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("S", first_style),
                ratatui::text::Span::styled(
                    "earch",
                    ratatui::style::Style::default().fg(t.text_muted),
                ),
            ])
        } else {
            let cursor_style = if cursor_on {
                ratatui::style::Style::default()
                    .fg(t.inverted_text)
                    .bg(t.highlight)
            } else {
                ratatui::style::Style::default()
                    .fg(t.background_panel)
                    .bg(t.background_panel)
            };
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(
                    self.picker_query.clone(),
                    ratatui::style::Style::default().fg(t.text),
                ),
                ratatui::text::Span::styled(" ", cursor_style),
            ])
        };

        let header_lines = vec![
            ratatui::text::Line::from(title_spans),
            ratatui::text::Line::from(""),
            input_line,
            ratatui::text::Line::from(""),
        ];

        let header_area = ratatui::layout::Rect {
            x: pal_area.x + super::PAD_L,
            y: pal_area.y + super::PAD_T,
            width: inner_w,
            height: super::HEADER_H,
        };
        f.render_widget(ratatui::widgets::Paragraph::new(header_lines), header_area);

        let mut list_lines = Vec::new();

        if no_results {
            list_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                "No results found",
                ratatui::style::Style::default().fg(t.text_muted),
            )));
        }

        for (flat, &i) in filtered.iter().enumerate() {
            let (name, _) = &self.themes[i];
            let current = i == self.current_idx;
            let hovered = flat == cursor;
            let prefix = if current { " ● " } else { "   " };
            let text_fg = if hovered {
                t.inverted_text
            } else if current {
                t.accent
            } else {
                t.text
            };
            let mut style = ratatui::style::Style::default().fg(text_fg);
            if hovered {
                style = style
                    .bg(t.highlight)
                    .add_modifier(ratatui::style::Modifier::BOLD);
            } else if current {
                style = style.add_modifier(ratatui::style::Modifier::BOLD);
            }
            let display = format!("{prefix}{name}");
            list_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                format!("{:<width$}", display, width = inner_w as usize),
                style,
            )));
        }

        let list_top = pal_area.y + super::PAD_T + super::HEADER_H;
        let list_area = ratatui::layout::Rect {
            x: pal_area.x + super::PAD_L,
            y: list_top,
            width: inner_w,
            height: list_h,
        };
        f.render_widget(
            ratatui::widgets::Paragraph::new(list_lines).scroll((self.picker_scroll, 0)),
            list_area,
        );
    }
}
