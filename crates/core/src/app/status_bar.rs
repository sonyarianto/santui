use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::auth::User;
use crate::theme::Theme;

/// Standalone status-bar widget, extracted from the monolithic Santui render.
///
/// Use [`StatusBar::render`] to draw it on a single-row area at the bottom of
/// the terminal.
#[derive(Debug)]
pub(super) struct StatusBar<'a> {
    pub theme: &'a Theme,
    pub about_open: bool,
    /// True when a plugin is the active (foreground) view.
    pub plugin_active: bool,
    /// Key-binding hints published by the active plugin.
    pub active_plugin_hints: &'a [(String, String)],
    /// Currently signed-in user, if any.
    pub user: Option<&'a User>,
    /// Config parse error to display, if any.
    pub config_error: Option<&'a str>,
    /// Auth flow message (e.g. "GitHub: enter code ABCD-1234").
    pub auth_message: Option<&'a str>,
    /// Names of crashed plugins to display in red.
    pub plugin_errors: &'a [String],
}

impl StatusBar<'_> {
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let dim = Style::default().fg(self.theme.text_muted);
        let key = Style::default().fg(self.theme.text);

        let line: Line = if self.about_open {
            Line::from(vec![Span::styled("esc", key), Span::styled(" close", dim)])
        } else if self.plugin_active {
            let cap = self.active_plugin_hints.len() * 3 + 6;
            let mut spans: Vec<Span> = Vec::with_capacity(cap);
            for (i, (hint_key, hint_desc)) in self.active_plugin_hints.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" • ", dim));
                }
                spans.push(Span::styled(hint_key.as_str(), key));
                spans.push(Span::styled(" ", dim));
                spans.push(Span::styled(hint_desc.as_str(), dim));
            }
            if !self.active_plugin_hints.is_empty() {
                spans.push(Span::styled(" • ", dim));
            }
            spans.push(Span::styled("esc", key));
            spans.push(Span::styled(" back", dim));
            Line::from(spans)
        } else {
            let mut spans = vec![
                Span::styled("ctrl+p", key),
                Span::styled(" commands • ", dim),
                Span::styled("?", key),
                Span::styled(" about • ", dim),
                Span::styled("q", key),
                Span::styled(" quit", dim),
            ];
            if !self.plugin_errors.is_empty() {
                spans.push(Span::styled(" • ", dim));
                spans.push(Span::styled("r", key));
                spans.push(Span::styled(" restart", dim));
            }
            Line::from(spans)
        };

        let p = Paragraph::new(line);
        f.render_widget(p, area);

        if let Some(err) = self.config_error {
            let err_span = Paragraph::new(Line::from(vec![Span::styled(
                err,
                Style::default().fg(self.theme.error),
            )]))
            .alignment(Alignment::Right);
            f.render_widget(err_span, area);
        } else if !self.plugin_errors.is_empty() {
            let msg = format!(
                "⚠ plugin crashed: {} — r to restart",
                self.plugin_errors.join(", ")
            );
            let err_span = Paragraph::new(Line::from(vec![Span::styled(
                msg,
                Style::default().fg(self.theme.error),
            )]))
            .alignment(Alignment::Right);
            f.render_widget(err_span, area);
        } else if let Some(msg) = self.auth_message {
            let msg_span = Paragraph::new(Line::from(vec![Span::styled(
                msg,
                Style::default().fg(self.theme.accent),
            )]))
            .alignment(Alignment::Right);
            f.render_widget(msg_span, area);
        } else {
            let mut right_spans: Vec<Span> = Vec::with_capacity(3);
            if let Some(u) = self.user {
                let provider_prefix = match u.provider.as_str() {
                    "github" => "github:",
                    "google" => "google:",
                    _ => "",
                };
                let display = if !u.email.is_empty() {
                    &u.email
                } else {
                    &u.name
                };
                right_spans.push(Span::styled(format!("{provider_prefix}{display}"), dim));
                right_spans.push(Span::styled(" ", dim));
            }
            right_spans.push(Span::styled("Santui ", key));
            right_spans.push(Span::styled(super::VERSION, dim));
            let right_para = Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right);
            f.render_widget(right_para, area);
        }
    }
}
