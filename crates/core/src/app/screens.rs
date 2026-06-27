use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl super::Santui {
    pub(super) fn render_starfield(&self, f: &mut Frame, area: Rect) {
        if area.width < 10 || area.height < 5 {
            return;
        }
        let buf = f.buffer_mut();
        for star in &self.starfield.stars {
            let sx = area.x
                + (star.x as u32 * area.width as u32 / 1009)
                    .min(area.width.saturating_sub(1) as u32) as u16;
            let sy = area.y
                + (star.y as u32 * area.height as u32 / 1009)
                    .min(area.height.saturating_sub(1) as u32) as u16;
            let cycle = self
                .starfield
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
    }

    pub(super) fn render_splash(&mut self, f: &mut Frame, area: Rect) {
        self.render_starfield(f, area);
        let t = &self.app_state.theme;
        let ver = super::VERSION;

        if self.cached_logo.is_none() {
            let mut lines: Vec<Line> = [
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

            lines.push(Line::from(Span::styled(
                "your terminal home base",
                Style::default().fg(t.text_muted),
            )));
            lines.push(Line::from(Span::styled(
                format!("v{ver}"),
                Style::default().fg(t.text_muted),
            )));
            self.cached_logo = Some(lines);
        }

        let Some(logo) = self.cached_logo.as_ref() else {
            return;
        };

        // ── Carousel bar ──
        let carousel = self.plugin_manager.carousel_items();
        let carousel_lines: Vec<Line> = if carousel.is_empty() {
            vec![]
        } else if let Some(sel) = self.app_state.home_selected {
            let item = &carousel[sel];
            let label = format!(" ◀ {} ▶ ", item.name);
            let styled = Line::from(Span::styled(label, Style::default().fg(t.accent)));
            vec![styled]
        } else {
            let hint = Line::from(Span::styled(
                " ← →  to browse plugins    ENTER  to open",
                Style::default().fg(t.text_muted),
            ));
            vec![hint]
        };

        let carousel_h = if carousel_lines.is_empty() { 0 } else { 2 };
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(8),
                Constraint::Length(carousel_h),
                Constraint::Fill(1),
            ])
            .split(area);

        let p = Paragraph::new(logo.as_slice()).alignment(Alignment::Center);
        f.render_widget(p, vert[1]);

        if !carousel_lines.is_empty() {
            let p = Paragraph::new(carousel_lines).alignment(Alignment::Center);
            f.render_widget(p, vert[2]);
        }
    }

    pub(super) fn render_about(&self, f: &mut Frame, area: Rect) {
        self.render_starfield(f, area);
        let t = &self.app_state.theme;
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
                    "your terminal home base",
                    Style::default().fg(t.text_muted),
                )),
                Line::from(Span::styled(
                    format!("v{ver}"),
                    Style::default().fg(t.text_muted),
                )),
                Line::from(""),
                Line::from("Copyright \u{00a9} Sony AK https://github.com/sonyarianto"),
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
}
