use crate::protocol::RenderCmd;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;

pub fn render_commands(f: &mut Frame, area: Rect, commands: &[RenderCmd]) {
    let buf = f.buffer_mut();
    for cmd in commands {
        match cmd {
            RenderCmd::Clear { x, y, w, h } => {
                let cx = area.x.saturating_add(*x);
                let cy = area.y.saturating_add(*y);
                let cw = (*w).min(buf.area().width.saturating_sub(cx));
                let ch = (*h).min(buf.area().height.saturating_sub(cy));
                let fill = " ".repeat(cw as usize);
                for row in cy..cy.saturating_add(ch) {
                    buf.set_string(cx, row, &fill, Style::reset());
                }
            }
            RenderCmd::Text {
                x,
                y,
                text,
                fg,
                bg,
                bold,
            } => {
                let cx = area.x.saturating_add(*x);
                let cy = area.y.saturating_add(*y);
                let mut style = Style::reset();
                if let Some([r, g, b]) = fg {
                    style = style.fg(Color::Rgb(*r, *g, *b));
                }
                if let Some([r, g, b]) = bg {
                    style = style.bg(Color::Rgb(*r, *g, *b));
                }
                if *bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                buf.set_string(cx, cy, text, style);
            }
        }
    }
}
