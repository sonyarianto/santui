use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
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

    pub(super) fn render_starfield(&self, f: &mut Frame, area: Rect) {
        if area.width < 10 || area.height < 5 {
            return;
        }
        let buf = f.buffer_mut();
        for star in &self.stars {
            let sx = area.x
                + (star.x as u32 * area.width as u32 / 1009)
                    .min(area.width.saturating_sub(1) as u32) as u16;
            let sy = area.y
                + (star.y as u32 * area.height as u32 / 1009)
                    .min(area.height.saturating_sub(1) as u32) as u16;
            let cycle = self
                .tick
                .wrapping_mul(star.freq as u64)
                .wrapping_add(star.phase as u64)
                % 480;
            let twinkle = if cycle < 240 { cycle } else { 480 - cycle };
            let t = twinkle as u16 * 200 / 240;
            let raw = star.mag as u16 * 60 + t;
            let v = raw.min(255) as u8;
            let cell = &mut buf[(sx, sy)];
            let (ch, r, g, b) = if v < 60 {
                ('.', 60, 60, 70)
            } else if v < 140 {
                ('.', v, v, (v as f64 * 1.15) as u8)
            } else if v < 200 {
                ('\u{00b7}', v, v, (v as f64 * 1.1).min(255.0) as u8)
            } else if v < 240 {
                ('*', 255, 250, 240)
            } else {
                ('\u{2726}', 255, 245, 230)
            };
            let warm = star.tint < 80;
            let cool = star.tint > 180;
            if v >= 100 {
                if warm {
                    cell.set_fg(Color::Rgb(
                        r,
                        (g as f64 * 0.85) as u8,
                        (b as f64 * 0.7) as u8,
                    ));
                } else if cool {
                    cell.set_fg(Color::Rgb(
                        (r as f64 * 0.8) as u8,
                        (g as f64 * 0.85) as u8,
                        b,
                    ));
                } else {
                    cell.set_fg(Color::Rgb(r, g, b));
                }
            } else {
                cell.set_fg(Color::Rgb(r, g, b));
            }
            cell.set_char(ch);
            cell.set_bg(Color::Reset);
        }
        if let Some(ref s) = self.shooting {
            let len = (10 + (s.age % 6) as usize).min(area.width.min(area.height) as usize);
            let sx = area.x + (s.x * area.width as f64) as u16;
            let sy = area.y + (s.y * area.height as f64) as u16;
            for i in 0..len {
                let px = (sx as i16 - (s.dx * i as f64 * 1.2) as i16)
                    .max(area.x as i16)
                    .min((area.x + area.width - 1) as i16) as u16;
                let py = (sy as i16 - (s.dy * i as f64 * 1.2) as i16)
                    .max(area.y as i16)
                    .min((area.y + area.height - 1) as i16) as u16;
                if px == sx && py == sy && i > 0 {
                    continue;
                }
                let t = i as f64 / len as f64;
                let vv = (220.0 * (1.0 - t * t * t)) as u8;
                let cell = &mut buf[(px, py)];
                cell.set_char(match i {
                    0 => '\u{2726}',
                    1 => '*',
                    2 => '\u{00b7}',
                    _ => '.',
                });
                cell.set_fg(if i < 3 {
                    Color::Rgb(180, 190, 255)
                } else {
                    let fade = vv.max(30);
                    Color::Rgb(fade, fade, (fade as f64 * 1.2).min(255.0) as u8)
                });
                cell.set_bg(Color::Reset);
            }
        }
    }

    pub(super) fn render_splash(&self, f: &mut Frame, area: Rect) {
        self.render_starfield(f, area);
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
                    "my terminal home base",
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
                    "my terminal home base",
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
