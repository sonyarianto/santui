use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

impl super::Santui {
    pub(super) fn filtered_items(&self, query: &str) -> Vec<super::ItemIndex> {
        let q = query.to_lowercase();
        let mut results = Vec::new();
        // Built-in items
        for (i, item) in super::CMD_ITEMS.iter().enumerate() {
            if query.is_empty() || item.label.to_lowercase().contains(&q) {
                results.push(super::ItemIndex::Builtin(i));
            }
        }
        // Dynamic (registry) items
        for (i, (_cat, _id, name)) in self.dynamic_items.iter().enumerate() {
            if query.is_empty() || name.to_lowercase().contains(&q) {
                results.push(super::ItemIndex::Dynamic(i));
            }
        }
        results
    }

    pub(super) fn ensure_cursor_visible(&mut self, content_h: u16) {
        let query = self.palette.as_ref().unwrap().query.clone();
        let filtered = self.filtered_items(&query);
        let cursor = self.palette.as_ref().unwrap().cursor;
        let no_results = !query.is_empty() && filtered.is_empty();
        let mut line: u16 = 0;
        if no_results {
            line += 1;
        }
        let mut cat = String::new();
        let mut first_cat = true;
        for (flat, &idx) in filtered.iter().enumerate() {
            let c = match idx {
                super::ItemIndex::Builtin(i) => super::CMD_ITEMS[i].category,
                super::ItemIndex::Dynamic(i) => &self.dynamic_items[i].0,
            };
            if c != cat {
                cat = c.to_string();
                if !first_cat {
                    line += 1;
                }
                first_cat = false;
                line += 1;
            }
            if flat == cursor {
                break;
            }
            line += 1;
        }
        let list_h = super::max_list_h(content_h);
        let pal = self.palette.as_mut().unwrap();
        if line < pal.scroll {
            pal.scroll = line.saturating_sub(1);
        } else if line >= pal.scroll + list_h {
            pal.scroll = line.saturating_sub(list_h.saturating_sub(1));
        }
    }

    pub(super) fn filtered_themes(&self) -> Vec<usize> {
        if self.theme_picker_query.is_empty() {
            return (0..self.themes.len()).collect();
        }
        let q = self.theme_picker_query.to_lowercase();
        self.themes
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    pub(super) fn ensure_theme_cursor_visible(&mut self, content_h: u16) {
        let list_h = super::max_list_h(content_h);
        let cursor = self.theme_picker_cursor as u16;
        if cursor < self.theme_picker_scroll {
            self.theme_picker_scroll = cursor;
        } else if cursor >= self.theme_picker_scroll + list_h {
            self.theme_picker_scroll = cursor.saturating_sub(list_h.saturating_sub(1));
        }
    }

    pub(super) fn select_theme(&mut self, idx: usize) {
        self.theme_idx = idx;
        self.theme = self.themes[idx].1.clone();
        self.ctx.theme = self.theme.clone();
        for p in &mut self.plugins {
            p.on_theme_change(&self.theme);
        }
    }

    pub(super) fn preview_theme(&mut self, idx: usize) {
        self.theme_idx = idx;
        self.theme = self.themes[idx].1.clone();
        self.ctx.theme = self.theme.clone();
    }

    pub(super) fn render_theme_picker(&self, f: &mut Frame, content: Rect) {
        let t = &self.theme;
        let query = &self.theme_picker_query;
        let filtered = self.filtered_themes();
        let cursor = self.theme_picker_cursor;

        let pw = super::pal_w(content.width);
        let inner_w = pw.saturating_sub(super::PAD_L * 2);

        let no_results = !query.is_empty() && filtered.is_empty();
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
        let pal_area = Rect {
            x,
            y,
            width: pw,
            height: pal_h,
        };

        f.render_widget(Clear, pal_area);
        f.render_widget(
            Paragraph::new(vec![]).style(Style::default().bg(t.background_panel)),
            pal_area,
        );

        let pad_w = inner_w.saturating_sub(9);
        let mut title_spans = vec![Span::styled(
            "Themes",
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        )];
        if pad_w > 0 {
            title_spans.push(Span::styled(" ".repeat(pad_w as usize), Style::default()));
        }
        title_spans.push(Span::styled("esc", Style::default().fg(t.text_muted)));

        let cursor_on = (self.tick / 5).is_multiple_of(2);

        let input_line = if query.is_empty() {
            let first_style = if cursor_on {
                Style::default().fg(t.inverted_text).bg(t.highlight)
            } else {
                Style::default().fg(t.text_muted)
            };
            Line::from(vec![
                Span::styled("S", first_style),
                Span::styled("earch", Style::default().fg(t.text_muted)),
            ])
        } else {
            let cursor_style = if cursor_on {
                Style::default().fg(t.inverted_text).bg(t.highlight)
            } else {
                Style::default()
                    .fg(t.background_panel)
                    .bg(t.background_panel)
            };
            Line::from(vec![
                Span::styled(query.clone(), Style::default().fg(t.text)),
                Span::styled(" ", cursor_style),
            ])
        };

        let header_lines = vec![
            Line::from(title_spans),
            Line::from(""),
            input_line,
            Line::from(""),
        ];

        let header_area = Rect {
            x: pal_area.x + super::PAD_L,
            y: pal_area.y + super::PAD_T,
            width: inner_w,
            height: super::HEADER_H,
        };
        f.render_widget(Paragraph::new(header_lines), header_area);

        let mut list_lines = Vec::new();

        if no_results {
            list_lines.push(Line::from(Span::styled(
                "No results found",
                Style::default().fg(t.text_muted),
            )));
        }

        for (flat, &i) in filtered.iter().enumerate() {
            let (name, _) = &self.themes[i];
            let current = i == self.theme_idx;
            let hovered = flat == cursor;
            let prefix = if current { " ● " } else { "   " };
            let text_fg = if hovered {
                t.inverted_text
            } else if current {
                t.accent
            } else {
                t.text
            };
            let mut style = Style::default().fg(text_fg);
            if hovered {
                style = style.bg(t.highlight).add_modifier(Modifier::BOLD);
            } else if current {
                style = style.add_modifier(Modifier::BOLD);
            }
            let display = format!("{prefix}{name}");
            list_lines.push(Line::from(Span::styled(
                format!("{:<width$}", display, width = inner_w as usize),
                style,
            )));
        }

        let list_top = pal_area.y + super::PAD_T + super::HEADER_H;
        let list_area = Rect {
            x: pal_area.x + super::PAD_L,
            y: list_top,
            width: inner_w,
            height: list_h,
        };
        f.render_widget(
            Paragraph::new(list_lines).scroll((self.theme_picker_scroll, 0)),
            list_area,
        );
    }

    pub(super) fn render_palette(&self, f: &mut Frame, content: Rect) {
        let t = &self.theme;
        let query = &self.palette.as_ref().unwrap().query;
        let filtered = self.filtered_items(query);
        let cursor = self.palette.as_ref().map_or(0, |p| p.cursor);
        let scroll = self.palette.as_ref().map_or(0, |p| p.scroll);

        let mut current_cat = String::new();
        let mut cat_items: Vec<super::ItemIndex> = Vec::new();
        let mut groups: Vec<(String, Vec<super::ItemIndex>)> = Vec::new();
        for &idx in &filtered {
            let cat = match idx {
                super::ItemIndex::Builtin(i) => super::CMD_ITEMS[i].category.to_string(),
                super::ItemIndex::Dynamic(i) => self.dynamic_items[i].0.clone(),
            };
            if cat != current_cat && !cat_items.is_empty() {
                groups.push((current_cat.clone(), std::mem::take(&mut cat_items)));
            }
            current_cat = cat;
            cat_items.push(idx);
        }
        if !cat_items.is_empty() {
            groups.push((current_cat, cat_items));
        }

        let no_results = !query.is_empty() && filtered.is_empty();
        let pw = super::pal_w(content.width);
        let inner_w = pw.saturating_sub(super::PAD_L * 2);

        let mut list_lines = Vec::new();

        if no_results {
            list_lines.push(Line::from(Span::styled(
                "No results found",
                Style::default().fg(t.text_muted),
            )));
        }

        let mut flat_idx = 0;
        for (i, (cat, items)) in groups.iter().enumerate() {
            if i > 0 {
                list_lines.push(Line::from(Span::styled("", Style::default())));
            }
            list_lines.push(Line::from(Span::styled(
                format!("{:<width$}", cat, width = inner_w as usize),
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            )));
            for &idx in items {
                let sel = flat_idx == cursor;
                let label = match idx {
                    super::ItemIndex::Builtin(i) => super::CMD_ITEMS[i].label.to_string(),
                    super::ItemIndex::Dynamic(i) => self.dynamic_items[i].2.clone(),
                };
                let style = if sel {
                    Style::default().fg(t.inverted_text).bg(t.highlight)
                } else {
                    Style::default().fg(t.text)
                };
                list_lines.push(Line::from(Span::styled(
                    format!("{:<width$}", label, width = inner_w as usize),
                    style,
                )));
                flat_idx += 1;
            }
        }

        let max_visible = super::max_list_h(content.height);
        let ideal_pal = super::PAD_T + super::HEADER_H + max_visible + super::PAD_B;
        let pal_h = ideal_pal
            .max(super::PAD_T + super::HEADER_H + super::PAD_B + 1)
            .min(content.height);
        let max_list = pal_h.saturating_sub(super::PAD_T + super::HEADER_H + super::PAD_B);
        let list_h = (list_lines.len() as u16).min(max_list).max(1);

        let x = (content.width.saturating_sub(pw)) / 2;
        let y = content.y + (content.height.saturating_sub(pal_h)) / 2;
        let pal_area = Rect {
            x,
            y,
            width: pw,
            height: pal_h,
        };

        f.render_widget(Clear, pal_area);
        f.render_widget(
            Paragraph::new(vec![]).style(Style::default().bg(t.background_panel)),
            pal_area,
        );

        let mut header_lines = Vec::new();

        let pad_w = inner_w.saturating_sub(11);
        let mut title_spans = vec![Span::styled(
            "Commands",
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        )];
        if pad_w > 0 {
            title_spans.push(Span::styled(" ".repeat(pad_w as usize), Style::default()));
        }
        title_spans.push(Span::styled("esc", Style::default().fg(t.text_muted)));
        header_lines.push(Line::from(title_spans));
        header_lines.push(Line::from(Span::styled("", Style::default())));

        let cursor_on = (self.tick / 5).is_multiple_of(2);

        if query.is_empty() {
            let first_style = if cursor_on {
                Style::default().fg(t.inverted_text).bg(t.highlight)
            } else {
                Style::default().fg(t.text_muted)
            };
            header_lines.push(Line::from(vec![
                Span::styled("S", first_style),
                Span::styled("earch", Style::default().fg(t.text_muted)),
            ]));
        } else {
            let cursor_style = if cursor_on {
                Style::default().fg(t.inverted_text).bg(t.highlight)
            } else {
                Style::default()
                    .fg(t.background_panel)
                    .bg(t.background_panel)
            };
            header_lines.push(Line::from(vec![
                Span::styled(query.clone(), Style::default().fg(t.text)),
                Span::styled(" ", cursor_style),
            ]));
        }
        header_lines.push(Line::from(Span::styled("", Style::default())));

        let header_area = Rect {
            x: pal_area.x + super::PAD_L,
            y: pal_area.y + super::PAD_T,
            width: inner_w,
            height: super::HEADER_H,
        };
        f.render_widget(Paragraph::new(header_lines), header_area);

        let list_top = pal_area.y + super::PAD_T + super::HEADER_H;
        let list_area = Rect {
            x: pal_area.x + super::PAD_L,
            y: list_top,
            width: inner_w,
            height: list_h,
        };
        f.render_widget(Paragraph::new(list_lines).scroll((scroll, 0)), list_area);
    }
}

#[cfg(test)]
mod tests {
    use super::super::Santui;

    #[test]
    fn filtered_items_empty_query_returns_all() {
        let app = Santui::new();
        let items = app.filtered_items("");
        assert_eq!(items.len(), super::super::CMD_ITEMS.len());
    }

    #[test]
    fn filtered_items_matches_label() {
        let app = Santui::new();
        let items = app.filtered_items("theme");
        assert_eq!(items.len(), 1);
        if let super::super::ItemIndex::Builtin(idx) = items[0] {
            assert_eq!(super::super::CMD_ITEMS[idx].label, "Switch theme");
        } else {
            panic!("expected Builtin item");
        }
    }

    #[test]
    fn filtered_items_matches_case_insensitive() {
        let app = Santui::new();
        let items = app.filtered_items("THEME");
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn filtered_items_no_match() {
        let app = Santui::new();
        let items = app.filtered_items("xyznonexistent");
        assert!(items.is_empty());
    }
}
