use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{List, ListItem, ListState};

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn sample_items() -> Vec<DisplayItem<'static>> {
        vec![
            DisplayItem {
                category: "System",
                label: "About",
            },
            DisplayItem {
                category: "System",
                label: "Switch theme",
            },
            DisplayItem {
                category: "Auth",
                label: "Sign in with Google",
            },
            DisplayItem {
                category: "Auth",
                label: "Sign in with GitHub",
            },
            DisplayItem {
                category: "Plugins",
                label: "Install plugin",
            },
        ]
    }

    fn default_highlight() -> Style {
        Style::default().bg(Color::Rgb(100, 100, 200))
    }
    fn default_group() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }
    fn default_text() -> Style {
        Style::default()
    }

    fn state_with_all_items(items: &[DisplayItem]) -> FilteredListState {
        let mut s = FilteredListState::new();
        s.set_query("".to_string(), items);
        s
    }

    // --- construction ---

    #[test]
    fn new_creates_empty_state() {
        let s = FilteredListState::new();
        assert!(s.query.is_empty());
        assert_eq!(s.cursor, 0);
        assert!(s.filtered_indices.is_empty());
        assert!(s.groups.is_empty());
        assert_eq!(s.total_lines, 0);
        assert!(s.dirty);
    }

    #[test]
    fn set_query_empty_returns_all() {
        let items = sample_items();
        let s = state_with_all_items(&items);
        assert_eq!(s.filtered_indices.len(), 5);
    }

    #[test]
    fn set_query_filters_case_insensitive() {
        let items = sample_items();
        let mut s = FilteredListState::new();
        s.set_query("sign".to_string(), &items);
        assert_eq!(s.filtered_indices.len(), 2);
    }

    #[test]
    fn set_query_no_match_yields_empty() {
        let items = sample_items();
        let mut s = FilteredListState::new();
        s.set_query("zzzzz".to_string(), &items);
        assert!(s.filtered_indices.is_empty());
    }

    #[test]
    fn set_query_idempotent_does_not_rebuild() {
        let items = sample_items();
        let mut s = FilteredListState::new();
        s.set_query("sign".to_string(), &items);
        s.dirty = false;
        s.set_query("sign".to_string(), &items);
        assert!(!s.dirty);
    }

    // --- cursor ---

    #[test]
    fn move_up_wraps_to_end() {
        let items = sample_items();
        let mut s = state_with_all_items(&items);
        s.move_up();
        assert_eq!(s.cursor, 4);
    }

    #[test]
    fn move_down_wraps_to_start() {
        let items = sample_items();
        let mut s = state_with_all_items(&items);
        s.cursor = 4;
        s.move_down();
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn move_up_down_steps() {
        let items = sample_items();
        let mut s = state_with_all_items(&items);
        s.move_down();
        assert_eq!(s.cursor, 1);
        s.move_down();
        assert_eq!(s.cursor, 2);
        s.move_up();
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn selected_item_returns_original_index() {
        let items = sample_items();
        let mut s = state_with_all_items(&items);
        // After sorting by category: Auth[0,1], Plugins[2], System[3,4]
        assert_eq!(s.selected_item(), Some(2)); // first Auth: "Sign in with Google"
        s.cursor = 1;
        assert_eq!(s.selected_item(), Some(3)); // second Auth: "Sign in with GitHub"
        s.cursor = 3;
        assert_eq!(s.selected_item(), Some(0)); // first System: "About"
    }

    #[test]
    fn selected_item_none_when_empty() {
        let items = sample_items();
        let mut s = FilteredListState::new();
        s.set_query("zzzzz".to_string(), &items);
        assert_eq!(s.selected_item(), None);
    }

    // --- push/pop chars ---

    #[test]
    fn push_char_appends_and_filters() {
        let items = sample_items();
        let mut s = FilteredListState::new();
        s.push_char('s', &items);
        assert_eq!(s.query, "s");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn pop_char_removes_last() {
        let items = sample_items();
        let mut s = FilteredListState::new();
        s.push_char('s', &items);
        s.push_char('i', &items);
        assert_eq!(s.query, "si");
        s.pop_char(&items);
        assert_eq!(s.query, "s");
    }

    // --- filtered_is_empty ---

    #[test]
    fn filtered_is_empty_true_when_no_results() {
        let items = sample_items();
        let mut s = FilteredListState::new();
        s.set_query("zzzzz".to_string(), &items);
        assert!(s.filtered_is_empty());
    }

    #[test]
    fn filtered_is_empty_false_when_all_items() {
        let items = sample_items();
        let s = state_with_all_items(&items);
        assert!(!s.filtered_is_empty());
    }

    // --- groups ---

    #[test]
    fn groups_computed_correctly() {
        let items = sample_items();
        let s = state_with_all_items(&items);
        let groups = s.groups();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].name, "Auth");
        assert_eq!(groups[0].count, 2);
        assert_eq!(groups[1].name, "Plugins");
        assert_eq!(groups[1].count, 1);
        assert_eq!(groups[2].name, "System");
        assert_eq!(groups[2].count, 2);
    }

    #[test]
    fn total_lines_includes_gaps_and_headers() {
        let items = sample_items();
        let s = state_with_all_items(&items);
        // 3 groups: Auth header(1)+items(2) + gap(1)+Plugins header(1)+items(1) + gap(1)+System header(1)+items(2)
        // = 3 + 3 + 4 = 10
        assert_eq!(s.total_lines(), 10);
    }

    // --- cursor_to_line ---

    #[test]
    fn cursor_to_line_skips_headers_and_gaps() {
        let items = sample_items();
        let mut s = state_with_all_items(&items);
        // Item cursor 0 (first Auth) → line after header "Auth" = line 1
        assert_eq!(s.cursor_to_line(), 1);
        // Item cursor 1 (second Auth) → line after first item = line 2
        s.cursor = 1;
        assert_eq!(s.cursor_to_line(), 2);
        // Item cursor 2 (Plugins) → Auth header(1) + items(2) + gap(1) + Plugins header(1) = line 5
        s.cursor = 2;
        assert_eq!(s.cursor_to_line(), 5);
    }

    // --- render_list ---

    #[test]
    fn render_list_empty_query_shows_no_results_msg() {
        let items = sample_items();
        let s = FilteredListState::new();
        let (list, state) = s.render_list(
            &items,
            default_highlight(),
            default_group(),
            default_text(),
            "nothing",
        );
        assert_eq!(state.selected(), None);
        let _ = list;
    }

    #[test]
    fn render_list_returns_state_with_selected_position() {
        let items = sample_items();
        let mut s = state_with_all_items(&items);
        s.cursor = 3; // first System item "About" → cursor_to_line = 8
        let (_list, state) = s.render_list(
            &items,
            default_highlight(),
            default_group(),
            default_text(),
            "",
        );
        assert_eq!(state.selected(), Some(8));
    }

    #[test]
    fn render_list_no_match_shows_no_results_msg() {
        let items = sample_items();
        let mut s = FilteredListState::new();
        s.set_query("zzzzz".to_string(), &items);
        let (_list, state) = s.render_list(
            &items,
            default_highlight(),
            default_group(),
            default_text(),
            "no match",
        );
        assert_eq!(state.selected(), None);
    }

    #[test]
    fn render_list_gap_between_groups() {
        let items = sample_items();
        let mut s = state_with_all_items(&items);
        // cursor 2 → Plugins "Install plugin" → cursor_to_line = 5
        s.cursor = 2;
        let (_list, state) = s.render_list(
            &items,
            default_highlight(),
            default_group(),
            default_text(),
            "",
        );
        assert_eq!(state.selected(), Some(5));
    }
}

pub struct DisplayItem<'a> {
    pub category: &'a str,
    pub label: &'a str,
}

#[derive(Debug)]
pub struct Group {
    pub name: String,
    pub start: usize,
    pub count: usize,
}

#[derive(Debug)]
pub struct FilteredListState {
    pub query: String,
    pub cursor: usize,
    filtered_indices: Vec<usize>,
    groups: Vec<Group>,
    dirty: bool,
    /// Total line count for the flat rendering (headers + items + gaps).
    total_lines: usize,
}

impl Default for FilteredListState {
    fn default() -> Self {
        Self::new()
    }
}

impl FilteredListState {
    pub fn new() -> Self {
        FilteredListState {
            query: String::new(),
            cursor: 0,
            filtered_indices: Vec::new(),
            groups: Vec::new(),
            dirty: true,
            total_lines: 0,
        }
    }

    pub fn set_query(&mut self, query: String, items: &[DisplayItem]) {
        if self.query == query && !self.dirty {
            return;
        }
        self.query = query;
        self.cursor = 0;
        self.dirty = true;
        self.rebuild(items);
    }

    pub fn push_char(&mut self, c: char, items: &[DisplayItem]) {
        self.query.push(c);
        self.cursor = 0;
        self.dirty = true;
        self.rebuild(items);
    }

    pub fn pop_char(&mut self, items: &[DisplayItem]) {
        self.query.pop();
        self.cursor = 0;
        self.dirty = true;
        self.rebuild(items);
    }

    fn rebuild(&mut self, items: &[DisplayItem]) {
        let q = self.query.to_lowercase();
        self.filtered_indices.clear();
        for (i, item) in items.iter().enumerate() {
            if self.query.is_empty() || item.label.to_lowercase().contains(&q) {
                self.filtered_indices.push(i);
            }
        }
        self.filtered_indices
            .sort_by(|&a, &b| items[a].category.cmp(items[b].category));

        self.groups.clear();
        let mut current_cat = String::new();
        let mut start = 0;
        for (offset, &idx) in self.filtered_indices.iter().enumerate() {
            let cat = items[idx].category;
            if cat != current_cat.as_str() {
                if !current_cat.is_empty() {
                    self.groups.push(Group {
                        name: current_cat,
                        start,
                        count: offset - start,
                    });
                }
                current_cat = cat.to_string();
                start = offset;
            }
        }
        if !current_cat.is_empty() {
            self.groups.push(Group {
                name: current_cat,
                start,
                count: self.filtered_indices.len() - start,
            });
        }

        self.total_lines = self.compute_total_lines();
        self.dirty = false;
    }

    fn compute_total_lines(&self) -> usize {
        if self.filtered_indices.is_empty() {
            if self.query.is_empty() {
                0
            } else {
                1
            }
        } else {
            let mut lines = 0;
            for (i, g) in self.groups.iter().enumerate() {
                if i > 0 {
                    lines += 1; // gap between groups
                }
                lines += 1; // group header
                lines += g.count; // items
            }
            lines
        }
    }

    pub fn filtered_is_empty(&self) -> bool {
        !self.query.is_empty() && self.filtered_indices.is_empty()
    }

    pub fn move_up(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.cursor = if self.cursor == 0 {
                self.filtered_indices.len() - 1
            } else {
                self.cursor - 1
            };
        }
    }

    pub fn move_down(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.cursor = if self.cursor + 1 >= self.filtered_indices.len() {
                0
            } else {
                self.cursor + 1
            };
        }
    }

    pub fn selected_item(&self) -> Option<usize> {
        self.filtered_indices.get(self.cursor).copied()
    }

    /// Map an item cursor (0..filtered_indices.len()) to a flat line index
    /// in the rendered output (including headers + gaps).
    pub fn cursor_to_line(&self) -> u16 {
        let mut line: u16 = 0;
        let mut remaining = self.cursor;
        for (i, g) in self.groups.iter().enumerate() {
            if i > 0 {
                line += 1; // gap
            }
            line += 1; // header
            if remaining < g.count {
                line += remaining as u16;
                return line;
            }
            remaining -= g.count;
            line += g.count as u16;
        }
        line
    }

    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    pub fn filtered_indices(&self) -> &[usize] {
        &self.filtered_indices
    }

    pub fn total_lines(&self) -> u16 {
        self.total_lines as u16
    }

    pub fn total_items(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_clean(&mut self) {
        self.dirty = false;
    }

    /// Render the filtered list into a `List` widget with category headers.
    /// Returns `ListState` for the caller to use with `f.render_stateful_widget`.
    /// The caller should set `list_state.selected` to the appropriate flat position
    /// BEFORE rendering (use `cursor_to_line` to compute it).
    pub fn render_list<'a>(
        &self,
        items: &'a [DisplayItem],
        highlight_style: Style,
        group_style: Style,
        text_style: Style,
        no_results_msg: &'a str,
    ) -> (List<'a>, ListState) {
        let mut list_items: Vec<ListItem<'a>> = Vec::new();

        if self.filtered_is_empty() {
            list_items.push(
                ListItem::new(Line::from(ratatui::text::Span::styled(
                    no_results_msg,
                    Style::default().add_modifier(Modifier::ITALIC),
                )))
                .style(text_style),
            );
            let mut state = ListState::default();
            state.select(None);
            let list = List::new(list_items).highlight_style(highlight_style);
            return (list, state);
        }

        for (i, g) in self.groups.iter().enumerate() {
            if i > 0 {
                list_items.push(ListItem::new(Line::from("")));
            }
            list_items.push(
                ListItem::new(Line::from(ratatui::text::Span::styled(
                    g.name.clone(),
                    group_style,
                )))
                .style(group_style),
            );
            for offset in 0..g.count {
                let idx = self.filtered_indices[g.start + offset];
                list_items.push(
                    ListItem::new(Line::from(ratatui::text::Span::styled(
                        items[idx].label.to_string(),
                        text_style,
                    )))
                    .style(text_style),
                );
            }
        }

        let mut state = ListState::default();
        let selected_line = self.cursor_to_line();
        if !self.filtered_indices.is_empty() {
            state.select(Some(selected_line as usize));
        }

        let list = List::new(list_items)
            .highlight_style(highlight_style)
            .highlight_symbol("");

        (list, state)
    }
}
