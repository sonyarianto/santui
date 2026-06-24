use crate::protocol::{RenderCmd, TextStyle};
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{self, ListState, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

fn to_style(s: &TextStyle) -> Style {
    let mut style = Style::default();
    if let Some([r, g, b]) = s.fg {
        style = style.fg(Color::Rgb(r, g, b));
    }
    if let Some([r, g, b]) = s.bg {
        style = style.bg(Color::Rgb(r, g, b));
    }
    if s.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

pub fn render_commands(f: &mut Frame, area: Rect, commands: &[RenderCmd]) {
    for cmd in commands {
        match cmd {
            RenderCmd::Paragraph {
                x,
                y,
                w,
                h,
                text,
                style,
                wrap,
            } => {
                let p_area =
                    Rect::new(area.x.saturating_add(*x), area.y.saturating_add(*y), *w, *h);
                let mut p = Paragraph::new(text.as_str()).style(to_style(style));
                if *wrap {
                    p = p.wrap(Wrap { trim: false });
                }
                f.render_widget(p, p_area);
            }
            RenderCmd::List {
                x,
                y,
                w,
                h,
                items,
                selected,
                style,
                highlight_style,
            } => {
                let list_area =
                    Rect::new(area.x.saturating_add(*x), area.y.saturating_add(*y), *w, *h);
                let list_items: Vec<widgets::ListItem> = items
                    .iter()
                    .map(|item| widgets::ListItem::new(item.as_str()))
                    .collect();
                let list = widgets::List::new(list_items)
                    .style(to_style(style))
                    .highlight_style(to_style(highlight_style))
                    .highlight_symbol("");
                let mut state = ListState::default();
                state.select(*selected);
                f.render_stateful_widget(list, list_area, &mut state);
            }
            RenderCmd::Table {
                x,
                y,
                w,
                h,
                header,
                header_style,
                rows,
                column_widths,
                selected,
                style,
                highlight_style,
            } => {
                let table_area =
                    Rect::new(area.x.saturating_add(*x), area.y.saturating_add(*y), *w, *h);
                let widths: Vec<Constraint> = column_widths
                    .iter()
                    .map(|&cw| Constraint::Length(cw))
                    .collect();
                let header_row =
                    Row::new(header.iter().map(|s| s.as_str())).style(to_style(header_style));
                let table_rows: Vec<Row> = rows
                    .iter()
                    .map(|row| Row::new(row.iter().map(|c| c.as_str())))
                    .collect();
                let table = Table::new(table_rows, &widths)
                    .header(header_row)
                    .style(to_style(style))
                    .row_highlight_style(to_style(highlight_style))
                    .highlight_symbol("");
                let mut state = TableState::default();
                state.select(*selected);
                f.render_stateful_widget(table, table_area, &mut state);
            }
            RenderCmd::Clear { x, y, w, h } => {
                let buf = f.buffer_mut();
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
                let buf = f.buffer_mut();
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
            RenderCmd::Rect { x, y, w, h, bg } => {
                let buf = f.buffer_mut();
                let cx = area.x.saturating_add(*x);
                let cy = area.y.saturating_add(*y);
                let cw = (*w).min(buf.area().width.saturating_sub(cx));
                let ch = (*h).min(buf.area().height.saturating_sub(cy));
                let bg_style = Style::reset().bg(Color::Rgb(bg[0], bg[1], bg[2]));
                let fill = " ".repeat(cw as usize);
                for row in cy..cy.saturating_add(ch) {
                    buf.set_string(cx, row, &fill, bg_style);
                }
            }
            RenderCmd::Border { x, y, w, h, fg } => {
                let buf = f.buffer_mut();
                let cx = area.x.saturating_add(*x);
                let cy = area.y.saturating_add(*y);
                let max_w = buf.area().width;
                let max_h = buf.area().height;
                let bw = (*w).min(max_w.saturating_sub(cx));
                let bh = (*h).min(max_h.saturating_sub(cy));
                if bw < 2 || bh < 2 {
                    return;
                }
                let fg_style = Style::reset().fg(Color::Rgb(fg[0], fg[1], fg[2]));
                let inner_w = bw.saturating_sub(2) as usize;

                // Top: ┌───┐
                let top = format!("┌{}┐", "─".repeat(inner_w));
                buf.set_string(cx, cy, &top, fg_style);

                // Sides: │    │
                for row in (cy + 1)..(cy + bh - 1) {
                    buf.set_string(cx, row, "│", fg_style);
                    buf.set_string(cx + bw - 1, row, "│", fg_style);
                }

                // Bottom: └───┘
                let bottom = format!("└{}┘", "─".repeat(inner_w));
                buf.set_string(cx, cy + bh - 1, &bottom, fg_style);
            }
        }
    }
}
