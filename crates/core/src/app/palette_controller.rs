use super::{BuiltinId, ItemIndex};
use crate::plugin::PluginCmdItem;
use crate::theme::Theme;
use crate::widgets::filtered_list::{DisplayItem, FilteredListState};
use crate::widgets::popup::centered_rect;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, StatefulWidget, Widget};
use ratatui::Frame;

/// Owns the command-palette overlay state and processes key events against
/// it, returning actions for the caller to execute.
pub(super) struct PaletteController {
    state: Option<FilteredListState>,
}

pub(super) enum PaletteAction {
    Execute(ItemIndex),
    None,
}

impl PaletteController {
    pub fn new() -> Self {
        Self { state: None }
    }

    pub fn open(&mut self) {
        self.state = Some(FilteredListState::new());
    }

    pub fn is_open(&self) -> bool {
        self.state.is_some()
    }

    /// Build a flat `DisplayItem` list and a parallel `ItemIndex` mapping
    /// from the three item sources.
    fn build_items<'a>(
        builtin_items: &'a [(BuiltinId, &'static str, &'static str)],
        dynamic_items: &'a [(String, String, String)],
        cmds: &'a [(usize, usize, PluginCmdItem)],
    ) -> (Vec<DisplayItem<'a>>, Vec<ItemIndex>) {
        let mut items = Vec::new();
        let mut mapping = Vec::new();
        for (i, (_id, cat, label)) in builtin_items.iter().enumerate() {
            items.push(DisplayItem {
                category: cat,
                label,
            });
            mapping.push(ItemIndex::Builtin(i));
        }
        for (i, (cat, _id, name)) in dynamic_items.iter().enumerate() {
            items.push(DisplayItem {
                category: cat.as_str(),
                label: name.as_str(),
            });
            mapping.push(ItemIndex::Dynamic(i));
        }
        for (i, (_p, _l, cmd)) in cmds.iter().enumerate() {
            items.push(DisplayItem {
                category: cmd.category.as_str(),
                label: cmd.label.as_str(),
            });
            mapping.push(ItemIndex::PluginCmd(i));
        }

        // Sort by category: Plugins, Auth, System, then alpha
        let mut indices: Vec<usize> = (0..items.len()).collect();
        let category_priority = |cat: &str| -> u8 {
            match cat {
                "Plugins" => 0,
                "Auth" => 1,
                "System" => 2,
                _ => 3,
            }
        };
        indices.sort_by(|&a, &b| {
            let pa = category_priority(items[a].category);
            let pb = category_priority(items[b].category);
            pa.cmp(&pb)
                .then_with(|| items[a].category.cmp(items[b].category))
        });
        let sorted_items: Vec<DisplayItem<'a>> = indices
            .iter()
            .map(|&i| DisplayItem {
                category: items[i].category,
                label: items[i].label,
            })
            .collect();
        let sorted_mapping: Vec<ItemIndex> = indices.iter().map(|&i| mapping[i]).collect();
        (sorted_items, sorted_mapping)
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,

        builtin_items: &[(BuiltinId, &'static str, &'static str)],
        dynamic_items: &[(String, String, String)],
        cmds: &[(usize, usize, PluginCmdItem)],
    ) -> PaletteAction {
        let Some(ref mut state) = self.state else {
            return PaletteAction::None;
        };
        let (items, _mapping) = Self::build_items(builtin_items, dynamic_items, cmds);

        match key.code {
            KeyCode::Char(c) if c == 'p' && key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state = None;
                return PaletteAction::None;
            }
            KeyCode::Char(_) if !key.modifiers.is_empty() => {}
            KeyCode::Char(c) => {
                state.push_char(c, &items);
            }
            KeyCode::Backspace => {
                state.pop_char(&items);
            }
            KeyCode::Up => {
                state.move_up();
            }
            KeyCode::Down => {
                state.move_down();
            }
            KeyCode::Enter => {
                if let Some(item_idx) = state.selected_item() {
                    let mapping = Self::build_items(builtin_items, dynamic_items, cmds).1;
                    let action = mapping[item_idx];
                    self.state = None;
                    return PaletteAction::Execute(action);
                }
                self.state = None;
                return PaletteAction::None;
            }
            KeyCode::Esc => {
                self.state = None;
                return PaletteAction::None;
            }
            _ => {}
        }
        PaletteAction::None
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        theme: &Theme,
        tick: u64,
        builtin_items: &[(BuiltinId, &'static str, &'static str)],
        dynamic_items: &[(String, String, String)],
        cmds: &[(usize, usize, PluginCmdItem)],
    ) {
        let Some(ref mut state) = self.state else {
            return;
        };

        let (items, _mapping) = Self::build_items(builtin_items, dynamic_items, cmds);

        if state.is_dirty() {
            state.set_query(state.query.clone(), &items);
        }

        let min_w = 30;
        let ideal_w = 60;
        let title_h = 4;
        let footer_h = 3;
        let list_h = if state.filtered_is_empty() {
            1
        } else {
            state.total_lines().max(1)
        };
        let popup_h = (title_h + list_h + footer_h)
            .min(area.height)
            .max(title_h + footer_h + 1);
        let popup_rect = centered_rect(area, min_w, ideal_w, popup_h);
        let inner_x = popup_rect.x.saturating_add(2);
        let inner_w = popup_rect.width.saturating_sub(4);

        render_palette_chrome(f.buffer_mut(), popup_rect, theme);

        // Header: title + search
        let header_area = Rect {
            x: inner_x,
            y: popup_rect.y + 1,
            width: inner_w,
            height: title_h,
        };
        render_palette_header(f.buffer_mut(), header_area, &state.query, tick, theme);

        // Filtered list
        let (list, mut list_state) = state.render_list(
            &items,
            Style::default().fg(theme.inverted_text).bg(theme.highlight),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(theme.text),
            "No results found",
        );
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
            StatefulWidget::render(list, list_area, f.buffer_mut(), &mut list_state);
        }

        // Footer: key hints
        let footer_area = Rect {
            x: inner_x,
            y: popup_rect.bottom().saturating_sub(footer_h),
            width: inner_w,
            height: footer_h,
        };
        render_palette_footer(f.buffer_mut(), footer_area, theme);
    }
}

impl Default for PaletteController {
    fn default() -> Self {
        Self::new()
    }
}

fn render_palette_chrome(buf: &mut Buffer, popup_rect: Rect, theme: &Theme) {
    Clear.render(popup_rect, buf);
    Paragraph::new(vec![])
        .style(Style::default().bg(theme.background_panel))
        .render(popup_rect, buf);
}

fn render_palette_header(buf: &mut Buffer, area: Rect, query: &str, tick: u64, theme: &Theme) {
    let cursor_on = (tick / 5).is_multiple_of(2);

    // Title bar: "Commands" + padding + "esc"
    let pad_w = area.width.saturating_sub(11) as usize;
    let title_spans = vec![
        Span::styled(
            "Commands",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ".repeat(pad_w), Style::default()),
        Span::styled("esc", Style::default().fg(theme.text_muted)),
    ];

    // Search input
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

fn render_palette_footer(buf: &mut Buffer, area: Rect, theme: &Theme) {
    let dim = Style::default().fg(theme.text_muted);
    let key = Style::default().fg(theme.text);
    let footer = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("↑", key),
            Span::styled(" up • ", dim),
            Span::styled("↓", key),
            Span::styled(" down • ", dim),
            Span::styled("↵", key),
            Span::styled(" select", dim),
        ]),
        Line::from(""),
    ];
    Paragraph::new(footer).render(area, buf);
}
