use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

/// Holds the plugin-registry screen state and rendering logic.
#[derive(Debug)]
pub(crate) struct RegistryScreen {
    /// Whether the registry overlay is open.
    pub(super) open: bool,
    /// Flat-list cursor position.
    pub(super) cursor: usize,
    /// Scroll offset (in lines).
    pub(super) scroll: u16,
    /// Status / info message shown in the header.
    pub(super) status: String,
}

impl RegistryScreen {
    pub(super) fn new() -> Self {
        RegistryScreen {
            open: false,
            cursor: 0,
            scroll: 0,
            status: String::new(),
        }
    }

    /// Adjust `self.scroll` so that the cursor is visible.
    pub(super) fn ensure_scroll_visible(&mut self, available_count: usize) {
        let content_h = 40u16;
        let list_h = super::max_list_h(content_h);
        let cursor = self.cursor.min(available_count.saturating_sub(1)) as u16;
        if cursor < self.scroll {
            self.scroll = cursor;
        } else if cursor >= self.scroll + list_h {
            self.scroll = cursor.saturating_sub(list_h.saturating_sub(1));
        }
    }

    /// Render the plugin-registry overlay.
    pub(super) fn render(
        &self,
        f: &mut Frame,
        area: Rect,
        theme: &crate::theme::Theme,
        registry: &Option<santui_registry::Registry>,
    ) {
        let pw = super::pal_w(area.width);
        let inner_w = pw.saturating_sub(super::PAD_L * 2);

        let available = registry.as_ref().map(|r| r.available.len()).unwrap_or(0);
        let list_h = (available as u16)
            .min(super::max_list_h(area.height))
            .max(1);
        let ideal_pal = super::PAD_T + super::HEADER_H + list_h + super::PAD_B + 2;
        let pal_h = ideal_pal.min(area.height);
        let x = (area.width.saturating_sub(pw)) / 2;
        let y = area.y + (area.height.saturating_sub(pal_h)) / 2;
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

        // Title
        let pad_w = inner_w.saturating_sub(12);
        let mut title_spans = vec![Span::styled(
            "Plugin registry",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )];
        if pad_w > 0 {
            title_spans.push(Span::styled(" ".repeat(pad_w as usize), Style::default()));
        }
        title_spans.push(Span::styled("esc", Style::default().fg(theme.text_muted)));

        let header_lines = vec![
            Line::from(title_spans),
            Line::from(""),
            Line::from(Span::styled(
                &self.status,
                Style::default().fg(theme.text_muted),
            )),
            Line::from(""),
        ];

        let header_area = Rect {
            x: pal_area.x + super::PAD_L,
            y: pal_area.y + super::PAD_T,
            width: inner_w,
            height: super::HEADER_H,
        };
        f.render_widget(Paragraph::new(header_lines), header_area);

        // Plugin list
        let mut list_lines: Vec<Line> = Vec::new();
        if let Some(ref reg) = registry {
            if reg.available.is_empty() {
                list_lines.push(Line::from(Span::styled(
                    "No plugins available",
                    Style::default().fg(theme.text_muted),
                )));
            } else {
                for (i, plugin) in reg.available.iter().enumerate() {
                    let is_installed = reg.installed.iter().any(|p| {
                        p.path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.trim_end_matches(".exe"))
                            == Some(&plugin.id)
                    });
                    let is_enabled = reg.installed.iter().any(|p| {
                        p.path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.trim_end_matches(".exe"))
                            == Some(&plugin.id)
                            && p.enabled
                    });
                    let hovered = i == self.cursor;
                    let status = if is_enabled {
                        " ON "
                    } else if is_installed {
                        " OFF"
                    } else {
                        "    "
                    };
                    let prefix = "   ";
                    let text_fg = if hovered {
                        theme.inverted_text
                    } else {
                        theme.text
                    };
                    let mut style = Style::default().fg(text_fg);
                    if hovered {
                        style = style.bg(theme.highlight).add_modifier(Modifier::BOLD);
                    }
                    let name_display = format!("{}{}  {}", prefix, plugin.name, status);
                    list_lines.push(Line::from(Span::styled(
                        format!("{:<width$}", name_display, width = inner_w as usize),
                        style,
                    )));
                    if hovered {
                        let desc_style = Style::default().fg(theme.text_muted);
                        list_lines.push(Line::from(Span::styled(
                            format!("  {}", plugin.description),
                            desc_style,
                        )));
                    }
                }
            }
        }

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
