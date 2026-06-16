use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

impl super::Santui {
    pub(super) fn render_dim_overlay(&self, f: &mut Frame, content: Rect) {
        let dim = Style::default()
            .fg(self.theme.text_muted)
            .add_modifier(Modifier::DIM);
        let fill: Vec<Line> = (0..content.height)
            .map(|_| Line::from(Span::styled(" ".repeat(content.width as usize), dim)))
            .collect();
        f.render_widget(Clear, content);
        f.render_widget(Paragraph::new(fill), content);
    }

    pub(super) fn render_splash(&self, f: &mut Frame, area: Rect) {
        let t = &self.theme;
        let ver = super::VERSION;

        let logo: Vec<Line> = [
            "███████╗ █████╗ ███╗   ██╗████████╗██╗   ██╗██╗",
            "██╔════╝██╔══██╗████╗  ██║╚══██╔══╝██║   ██║██║",
            "███████╗███████║██╔██╗ ██║   ██║   ██║   ██║██║",
            "╚════██║██╔══██║██║╚██╗██║   ██║   ██║   ██║██║",
            "███████║██║  ██║██║ ╚████║   ██║   ╚██████╔╝██║",
            "╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝",
        ]
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(t.logo))))
        .collect::<Vec<_>>();

        let logo = logo
            .into_iter()
            .chain([
                Line::from(Span::styled(
                    "modular TUI platform",
                    Style::default().fg(t.text_muted),
                )),
                Line::from(Span::styled(
                    format!("v{ver}"),
                    Style::default().fg(t.text_muted),
                )),
            ])
            .collect::<Vec<_>>();

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(8),
                Constraint::Fill(1),
            ])
            .split(area);

        let p = Paragraph::new(logo).alignment(Alignment::Center);
        f.render_widget(p, vert[1]);
    }

    pub(super) fn render_about(&self, f: &mut Frame, area: Rect) {
        let t = &self.theme;
        let ver = super::VERSION;

        let text: Vec<Line> = [
            "███████╗ █████╗ ███╗   ██╗████████╗██╗   ██╗██╗",
            "██╔════╝██╔══██╗████╗  ██║╚══██╔══╝██║   ██║██║",
            "███████╗███████║██╔██╗ ██║   ██║   ██║   ██║██║",
            "╚════██║██╔══██║██║╚██╗██║   ██║   ██║   ██║██║",
            "███████║██║  ██║██║ ╚████║   ██║   ╚██████╔╝██║",
            "╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝",
        ]
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(t.logo))))
        .collect();

        let text = text
            .into_iter()
            .chain([
                Line::from(Span::styled(
                    "modular TUI platform",
                    Style::default().fg(t.text_muted),
                )),
                Line::from(Span::styled(
                    format!("v{ver}"),
                    Style::default().fg(t.text_muted),
                )),
                Line::from(""),
                Line::from("Copyright \u{00a9} Sony AK  <sony@sony-ak.com>"),
                Line::from(""),
                Line::from(Span::styled(
                    "https://santuiapp.vercel.app",
                    Style::default().fg(t.text_muted),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press esc to go back",
                    Style::default().fg(t.text_muted),
                )),
            ])
            .collect::<Vec<_>>();

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(16),
                Constraint::Fill(1),
            ])
            .split(area);

        let p = Paragraph::new(text).alignment(Alignment::Center);
        f.render_widget(p, vert[1]);
    }

    pub(super) fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let t = &self.theme;
        let dim = Style::default().fg(t.text_muted);
        let key = Style::default().fg(t.text);

        let line: Line = if self.palette.is_some() {
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
        } else if self.show_theme_picker {
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
        } else if self.show_about {
            Line::from(vec![Span::styled("esc", key), Span::styled(" close", dim)])
        } else if let Some(idx) = self.active_plugin {
            let plugin_hints = self.plugins[idx].status_hints();
            let mut spans: Vec<Span> = Vec::new();
            for (i, (hint_key, hint_desc)) in plugin_hints.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" • ", dim));
                }
                spans.push(Span::styled(hint_key.clone(), key));
                spans.push(Span::styled(format!(" {hint_desc}"), dim));
            }
            if !plugin_hints.is_empty() {
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
