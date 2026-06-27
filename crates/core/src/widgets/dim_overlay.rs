use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

pub struct DimOverlay {
    pub style: Style,
}

impl Widget for DimOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dim_bg = self.style.bg;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    let mut s = cell.style();
                    if let Some(bg) = dim_bg {
                        // Cell::set_style normalises None → Some(Reset), so
                        // check for either.
                        if s.bg.is_none_or(|c| c == ratatui::style::Color::Reset) {
                            s.bg = Some(bg);
                        } else {
                            s.bg = s.bg.map(|c| dim_color(c, 0.45));
                        }
                    }
                    s.fg = s.fg.map(|c| dim_color(c, 0.45));
                    cell.set_style(s);
                }
            }
        }
    }
}

fn dim_color(c: ratatui::style::Color, factor: f64) -> ratatui::style::Color {
    match c {
        ratatui::style::Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(
            (r as f64 * factor) as u8,
            (g as f64 * factor) as u8,
            (b as f64 * factor) as u8,
        ),
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    fn style_fg(fg: Color) -> Style {
        Style {
            fg: Some(fg),
            bg: Some(Color::Reset),
            add_modifier: Modifier::empty(),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        }
    }

    fn style_fg_bg(fg: Color, bg: Color) -> Style {
        Style {
            fg: Some(fg),
            bg: Some(bg),
            add_modifier: Modifier::empty(),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        }
    }

    #[test]
    fn dim_overlay_applies_overlay_bg_to_reset_cells() {
        let area = Rect::new(0, 0, 3, 1);
        let mut buf = Buffer::with_lines(vec!["abc"]);

        DimOverlay {
            style: Style::default().bg(Color::Rgb(10, 20, 30)),
        }
        .render(area, &mut buf);

        // Default cells have fg/bg = Some(Reset) → treated as "no explicit bg"
        let cell = buf.cell((0, 0)).unwrap();
        assert_eq!(cell.style().fg, Some(Color::Reset)); // fg Reset stays Reset
        assert_eq!(cell.style().bg, Some(Color::Rgb(10, 20, 30))); // overlay bg applied
    }

    #[test]
    fn dim_overlay_dims_existing_rgb_fg_and_bg() {
        let area = Rect::new(0, 0, 3, 1);
        let mut buf = Buffer::with_lines(vec!["abc"]);
        buf[(0u16, 0u16)].set_style(style_fg_bg(
            Color::Rgb(200, 100, 50),
            Color::Rgb(50, 100, 200),
        ));

        DimOverlay {
            style: Style::default().bg(Color::Rgb(10, 20, 30)),
        }
        .render(area, &mut buf);

        let cell = buf.cell((0, 0)).unwrap();
        assert_eq!(cell.style().fg, Some(Color::Rgb(90, 45, 22)));
        assert_eq!(cell.style().bg, Some(Color::Rgb(22, 45, 90)));
    }

    #[test]
    fn dim_overlay_outside_area_untouched() {
        let area = Rect::new(0, 0, 2, 1);
        let mut buf = Buffer::with_lines(vec!["abc"]);
        buf[(0u16, 0u16)].set_style(style_fg(Color::Rgb(200, 100, 50)));
        buf[(2u16, 0u16)].set_style(style_fg(Color::Rgb(200, 100, 50)));

        DimOverlay {
            style: Style::default().bg(Color::Rgb(10, 20, 30)),
        }
        .render(area, &mut buf);

        assert_eq!(
            buf.cell((0, 0)).unwrap().style().fg,
            Some(Color::Rgb(90, 45, 22))
        );
        assert_eq!(
            buf.cell((2, 0)).unwrap().style().fg,
            Some(Color::Rgb(200, 100, 50))
        );
    }

    #[test]
    fn dim_overlay_empty_area_no_panic() {
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::with_lines(vec!["abc"]);
        DimOverlay {
            style: Style::default().bg(Color::Rgb(10, 20, 30)),
        }
        .render(area, &mut buf);
    }

    #[test]
    fn dim_overlay_no_overlay_bg_only_dims_fg() {
        let area = Rect::new(0, 0, 3, 1);
        let mut buf = Buffer::with_lines(vec!["abc"]);
        buf[(0u16, 0u16)].set_style(style_fg(Color::Rgb(200, 100, 50)));

        DimOverlay {
            style: Style::default(), // no bg → dim_bg is None
        }
        .render(area, &mut buf);

        let cell = buf.cell((0, 0)).unwrap();
        assert_eq!(cell.style().fg, Some(Color::Rgb(90, 45, 22)));
        assert_eq!(cell.style().bg, Some(Color::Reset));
    }
}
