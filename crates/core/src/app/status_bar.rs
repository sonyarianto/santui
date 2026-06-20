use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::Theme;

/// Standalone status-bar widget, extracted from the monolithic Santui render.
///
/// Use [`StatusBar::render`] to draw it on a single-row area at the bottom of
/// the terminal.
#[derive(Debug)]
pub(super) struct StatusBar<'a> {
    pub theme: &'a Theme,
    pub palette_open: bool,
    pub theme_picker_open: bool,
    pub about_open: bool,
    /// True when a plugin is the active (foreground) view.
    pub plugin_active: bool,
    /// Key-binding hints published by the active plugin.
    pub active_plugin_hints: &'a [(String, String)],
}

impl StatusBar<'_> {
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let dim = Style::default().fg(self.theme.text_muted);
        let key = Style::default().fg(self.theme.text);

        let line: Line = if self.palette_open {
            Line::from(vec![
                Span::styled("↑", key),
                Span::styled(" up • ", dim),
                Span::styled("↓", key),
                Span::styled(" down • ", dim),
                Span::styled("↵", key),
                Span::styled(" enter • ", dim),
                Span::styled("esc", key),
                Span::styled(" close", dim),
            ])
        } else if self.theme_picker_open {
            Line::from(vec![
                Span::styled("↑", key),
                Span::styled(" up • ", dim),
                Span::styled("↓", key),
                Span::styled(" down • ", dim),
                Span::styled("↵", key),
                Span::styled(" select • ", dim),
                Span::styled("esc", key),
                Span::styled(" back", dim),
            ])
        } else if self.about_open {
            Line::from(vec![Span::styled("esc", key), Span::styled(" close", dim)])
        } else if self.plugin_active {
            let mut spans: Vec<Span> = Vec::new();
            for (i, (hint_key, hint_desc)) in self.active_plugin_hints.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" • ", dim));
                }
                spans.push(Span::styled(hint_key.clone(), key));
                spans.push(Span::styled(format!(" {hint_desc}"), dim));
            }
            if !self.active_plugin_hints.is_empty() {
                spans.push(Span::styled(" • ", dim));
            }
            spans.push(Span::styled("ctrl+p", key));
            spans.push(Span::styled(" commands • ", dim));
            spans.push(Span::styled("esc", key));
            spans.push(Span::styled(" back • ", dim));
            spans.push(Span::styled("q", key));
            spans.push(Span::styled(" quit", dim));
            Line::from(spans)
        } else {
            Line::from(vec![
                Span::styled("ctrl+p", key),
                Span::styled(" commands • ", dim),
                Span::styled("?", key),
                Span::styled(" about • ", dim),
                Span::styled("q", key),
                Span::styled(" quit", dim),
            ])
        };

        let p = Paragraph::new(line);
        f.render_widget(p, area);

        let version = Line::from(vec![
            Span::styled("Santui ", key),
            Span::styled(super::VERSION, dim),
        ]);
        let version_para = Paragraph::new(version).alignment(Alignment::Right);
        f.render_widget(version_para, area);
    }
}
