use crate::plugin::PluginCmdItem;
use crate::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

/// Holds the command-palette interaction state and rendering logic.
///
/// The palette is an overlay opened via `Ctrl+P` that lists built-in
/// commands, registry-installed plugins, and plugin-registered commands.
#[derive(Debug)]
pub(super) struct PaletteWidget {
    /// Current search query typed by the user.
    pub(super) query: String,
    /// Flat-list cursor position.
    pub(super) cursor: usize,
    /// Scroll offset (in lines) for the filtered list.
    pub(super) scroll: u16,
}

impl PaletteWidget {
    pub(super) fn new() -> Self {
        PaletteWidget {
            query: String::new(),
            cursor: 0,
            scroll: 0,
        }
    }

    /// Return indices of all items matching `self.query`, grouped into
    /// built-in, dynamic (registry), and plugin-command categories.
    pub(super) fn filtered_items(
        &self,
        builtin_items: &[(super::BuiltinId, &'static str, &'static str)],
        dynamic_items: &[(String, String, String)],
        cmds: &[(usize, usize, PluginCmdItem)],
    ) -> Vec<super::ItemIndex> {
        let q = self.query.to_lowercase();
        let mut results = Vec::new();
        // Built-in items
        for (i, (_id, _cat, label)) in builtin_items.iter().enumerate() {
            if self.query.is_empty() || label.to_lowercase().contains(&q) {
                results.push(super::ItemIndex::Builtin(i));
            }
        }
        // Dynamic (registry) items
        for (i, (_cat, _id, name)) in dynamic_items.iter().enumerate() {
            if self.query.is_empty() || name.to_lowercase().contains(&q) {
                results.push(super::ItemIndex::Dynamic(i));
            }
        }
        // Plugin-registered commands
        for (i, (_plugin_idx, _local_idx, cmd)) in cmds.iter().enumerate() {
            if self.query.is_empty() || cmd.label.to_lowercase().contains(&q) {
                results.push(super::ItemIndex::PluginCmd(i));
            }
        }
        // Sort by category so items with the same category are grouped
        // together regardless of source (builtin, dynamic, plugin command).
        results.sort_by(|a, b| {
            let cat_a = match a {
                super::ItemIndex::Builtin(i) => builtin_items[*i].1,
                super::ItemIndex::Dynamic(i) => dynamic_items[*i].0.as_str(),
                super::ItemIndex::PluginCmd(i) => cmds[*i].2.category.as_str(),
            };
            let cat_b = match b {
                super::ItemIndex::Builtin(i) => builtin_items[*i].1,
                super::ItemIndex::Dynamic(i) => dynamic_items[*i].0.as_str(),
                super::ItemIndex::PluginCmd(i) => cmds[*i].2.category.as_str(),
            };
            cat_a.cmp(cat_b)
        });
        results
    }

    /// Adjust `self.scroll` so that the cursor is visible in the list.
    /// Accepts a pre-computed `filtered` slice to avoid re-filtering.
    pub(super) fn ensure_cursor_visible(
        &mut self,
        content_h: u16,
        filtered: &[super::ItemIndex],
        builtin_items: &[(super::BuiltinId, &'static str, &'static str)],
        dynamic_items: &[(String, String, String)],
        cmds: &[(usize, usize, PluginCmdItem)],
    ) {
        let no_results = !self.query.is_empty() && filtered.is_empty();
        let mut line: u16 = 0;
        if no_results {
            line += 1;
        }
        let mut cat = String::new();
        let mut first_cat = true;
        for (flat, &idx) in filtered.iter().enumerate() {
            let c = match idx {
                super::ItemIndex::Builtin(i) => builtin_items[i].1,
                super::ItemIndex::Dynamic(i) => dynamic_items[i].0.as_str(),
                super::ItemIndex::PluginCmd(i) => cmds[i].2.category.as_str(),
            };
            if c != cat {
                cat = c.to_string();
                if !first_cat {
                    line += 1;
                }
                first_cat = false;
                line += 1;
            }
            if flat == self.cursor {
                break;
            }
            line += 1;
        }
        let list_h = super::max_list_h(content_h);
        if line < self.scroll {
            self.scroll = line.saturating_sub(1);
        } else if line >= self.scroll + list_h {
            self.scroll = line.saturating_sub(list_h.saturating_sub(1));
        }
    }

    /// Render the command-palette overlay with pre-computed groups.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_with_groups(
        &self,
        f: &mut Frame,
        content: Rect,
        theme: &Theme,
        tick: u64,
        builtin_items: &[(super::BuiltinId, &'static str, &'static str)],
        dynamic_items: &[(String, String, String)],
        cmds: &[(usize, usize, PluginCmdItem)],
        groups: &[(String, Vec<super::ItemIndex>)],
    ) {
        let no_results = !self.query.is_empty() && groups.is_empty();
        let pw = super::pal_w(content.width);
        let inner_w = pw.saturating_sub(super::PAD_L * 2);

        let mut list_lines = Vec::new();

        if no_results {
            list_lines.push(Line::from(Span::styled(
                "No results found",
                Style::default().fg(theme.text_muted),
            )));
        }

        let mut flat_idx = 0;
        for (i, (cat, items)) in groups.iter().enumerate() {
            if i > 0 {
                list_lines.push(Line::from(Span::styled("", Style::default())));
            }
            list_lines.push(Line::from(Span::styled(
                format!("{:<width$}", cat, width = inner_w as usize),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )));
            for &idx in items {
                let sel = flat_idx == self.cursor;
                let label = match idx {
                    super::ItemIndex::Builtin(i) => builtin_items[i].2,
                    super::ItemIndex::Dynamic(i) => &dynamic_items[i].2,
                    super::ItemIndex::PluginCmd(i) => &cmds[i].2.label,
                };
                let style = if sel {
                    Style::default().fg(theme.inverted_text).bg(theme.highlight)
                } else {
                    Style::default().fg(theme.text)
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
            Paragraph::new(vec![]).style(Style::default().bg(theme.background_panel)),
            pal_area,
        );

        let mut header_lines = Vec::new();

        let pad_w = inner_w.saturating_sub(11);
        let mut title_spans = vec![Span::styled(
            "Commands",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )];
        if pad_w > 0 {
            title_spans.push(Span::styled(" ".repeat(pad_w as usize), Style::default()));
        }
        title_spans.push(Span::styled("esc", Style::default().fg(theme.text_muted)));
        header_lines.push(Line::from(title_spans));
        header_lines.push(Line::from(Span::styled("", Style::default())));

        let cursor_on = (tick / 5).is_multiple_of(2);

        if self.query.is_empty() {
            let first_style = if cursor_on {
                Style::default().fg(theme.inverted_text).bg(theme.highlight)
            } else {
                Style::default().fg(theme.text_muted)
            };
            header_lines.push(Line::from(vec![
                Span::styled("S", first_style),
                Span::styled("earch", Style::default().fg(theme.text_muted)),
            ]));
        } else {
            let cursor_style = if cursor_on {
                Style::default().fg(theme.inverted_text).bg(theme.highlight)
            } else {
                Style::default()
                    .fg(theme.background_panel)
                    .bg(theme.background_panel)
            };
            header_lines.push(Line::from(vec![
                Span::styled(self.query.clone(), Style::default().fg(theme.text)),
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
        f.render_widget(
            Paragraph::new(list_lines).scroll((self.scroll, 0)),
            list_area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::PaletteWidget;
    use crate::app::{all_builtins, BuiltinId, ItemIndex};

    fn builtin_fixture() -> Vec<(BuiltinId, String, String)> {
        all_builtins()
            .into_iter()
            .map(|(id, cat, label)| (id, cat.to_string(), label.to_string()))
            .collect()
    }

    #[test]
    fn filtered_items_empty_query_returns_all() {
        let pal = PaletteWidget::new();
        let bi = builtin_fixture();
        let items = pal.filtered_items(&bi, &[], &[]);
        assert_eq!(items.len(), bi.len());
    }

    #[test]
    fn filtered_items_matches_label() {
        let bi = builtin_fixture();
        let pal = PaletteWidget {
            query: "theme".into(),
            cursor: 0,
            scroll: 0,
        };
        let items = pal.filtered_items(&bi, &[], &[]);
        assert_eq!(items.len(), 1);
        if let ItemIndex::Builtin(idx) = items[0] {
            assert_eq!(bi[idx].2, "Switch theme");
        } else {
            panic!("expected Builtin item");
        }
    }

    #[test]
    fn filtered_items_matches_case_insensitive() {
        let bi = builtin_fixture();
        let pal = PaletteWidget {
            query: "THEME".into(),
            cursor: 0,
            scroll: 0,
        };
        let items = pal.filtered_items(&bi, &[], &[]);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn filtered_items_no_match() {
        let bi = builtin_fixture();
        let pal = PaletteWidget {
            query: "xyznonexistent".into(),
            cursor: 0,
            scroll: 0,
        };
        let items = pal.filtered_items(&bi, &[], &[]);
        assert!(items.is_empty());
    }
}
