use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl super::Santui {
    /// Render text character-by-character, skipping spaces, so the
    /// background (e.g. starfield) shows through between glyphs.
    fn draw_transparent_text(
        f: &mut Frame,
        area: Rect,
        lines: &[&str],
        color: ratatui::style::Color,
    ) {
        let buf = f.buffer_mut();
        for (row, line) in lines.iter().enumerate() {
            let y = area.y + row as u16;
            let w = line.chars().count() as u16;
            let x0 = area.x + (area.width.saturating_sub(w)) / 2;
            for (col, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let x = x0 + col as u16;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_fg(color);
                }
            }
        }
    }

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

        // ── Carousel bar ──
        let carousel = self.plugin_manager.carousel_items();
        let carousel_lines: Vec<Line> = if carousel.is_empty() {
            vec![]
        } else if let Some(sel) = self.app_state.home_selected {
            let item = &carousel[sel];
            let label = format!(" ◀ {} ▶ ", item.name);
            let styled = Line::from(Span::styled(label, Style::default().fg(t.accent)));
            vec![Line::from(""), styled]
        } else {
            let hint = Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled("← →", Style::default().fg(t.accent)),
                Span::styled(" to browse plugins    ", Style::default().fg(t.text_muted)),
                Span::styled("Enter", Style::default().fg(t.accent)),
                Span::styled(" to open", Style::default().fg(t.text_muted)),
            ]);
            vec![Line::from(""), hint]
        };

        let carousel_h = if carousel_lines.is_empty() { 0 } else { 2 };
        let logo_h: u16 = 6;
        let comment_h: u16 = 2; // tagline + version
        let content_h = logo_h + comment_h + carousel_h;
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(content_h),
                Constraint::Fill(1),
            ])
            .split(area);

        let logo_area = Rect {
            x: vert[1].x,
            y: vert[1].y,
            width: vert[1].width,
            height: logo_h,
        };
        Self::draw_transparent_text(
            f,
            logo_area,
            &[
                "███████╗ █████╗ ███╗   ██╗████████╗██╗   ██╗██╗",
                "██╔════╝██╔══██╗████╗  ██║╚══██╔══╝██║   ██║██║",
                "███████╗███████║██╔██╗ ██║   ██║   ██║   ██║██║",
                "╚════██║██╔══██║██║╚██╗██║   ██║   ██║   ██║██║",
                "███████║██║  ██║██║ ╚████║   ██║   ╚██████╔╝██║",
                "╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝",
            ],
            t.logo,
        );

        let comment_area = Rect {
            x: vert[1].x,
            y: vert[1].y + logo_h,
            width: vert[1].width,
            height: comment_h,
        };
        Self::draw_transparent_text(
            f,
            comment_area,
            &["your terminal home base", &format!("v{ver}")],
            t.text_muted,
        );

        if !carousel_lines.is_empty() {
            let carousel_rect = Rect {
                x: vert[1].x,
                y: vert[1].y + logo_h + comment_h,
                width: vert[1].width,
                height: carousel_h,
            };
            let p = Paragraph::new(carousel_lines).alignment(Alignment::Center);
            f.render_widget(p, carousel_rect);
        }
    }

    pub(super) fn render_about(&self, f: &mut Frame, area: Rect) {
        self.render_starfield(f, area);
        let t = &self.app_state.theme;
        let ver = super::VERSION;
        let year = 1970u64
            + std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                / 31_556_952;

        let info: Vec<Line> = vec![
            Line::from(""),
            Line::from(""),
            Line::from(""),
            Line::from(""),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "your terminal home base",
                Style::default().fg(t.text_muted),
            )),
            Line::from(Span::styled(
                format!("v{ver}"),
                Style::default().fg(t.text_muted),
            )),
            Line::from(""),
            Line::from(format!("\u{00a9} {year} Santui contributors")),
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
        ];

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(16),
                Constraint::Fill(1),
            ])
            .split(area);

        let logo_area = Rect {
            x: vert[1].x,
            y: vert[1].y,
            width: vert[1].width,
            height: 6,
        };
        Self::draw_transparent_text(
            f,
            logo_area,
            &[
                "███████╗ █████╗ ███╗   ██╗████████╗██╗   ██╗██╗",
                "██╔════╝██╔══██╗████╗  ██║╚══██╔══╝██║   ██║██║",
                "███████╗███████║██╔██╗ ██║   ██║   ██║   ██║██║",
                "╚════██║██╔══██║██║╚██╗██║   ██║   ██║   ██║██║",
                "███████║██║  ██║██║ ╚████║   ██║   ╚██████╔╝██║",
                "╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝",
            ],
            t.logo,
        );

        let p = Paragraph::new(info).alignment(Alignment::Center);
        f.render_widget(p, vert[1]);
    }

    pub(super) fn render_loading(&mut self, f: &mut Frame, area: Rect, plugin_name: &str) {
        self.render_starfield(f, area);
        let t = &self.app_state.theme;
        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(3),
                Constraint::Fill(1),
            ])
            .split(area);
        let lines = vec![Line::from(Span::styled(
            format!("Loading {plugin_name}..."),
            Style::default().fg(t.text_muted),
        ))];
        let p = Paragraph::new(lines).alignment(Alignment::Center);
        f.render_widget(p, vert[1]);
    }
}
