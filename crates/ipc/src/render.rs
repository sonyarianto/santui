use crate::protocol::{RenderCmd, TextStyle};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{self, Block, Borders, ListState, Paragraph, Row, Table, TableState, Wrap};
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

/// Clip a rectangle to the frame buffer bounds.
fn clipped(area: Rect, x: u16, y: u16, w: u16, h: u16) -> Rect {
    let cx = area.x.saturating_add(x);
    let cy = area.y.saturating_add(y);
    let cw = w.min(area.width.saturating_sub(cx.saturating_sub(area.x)));
    let ch = h.min(area.height.saturating_sub(cy.saturating_sub(area.y)));
    Rect::new(cx, cy, cw, ch)
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
                let p_area = clipped(area, *x, *y, *w, *h);
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
                let list_area = clipped(area, *x, *y, *w, *h);
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
                let table_area = clipped(area, *x, *y, *w, *h);
                let widths: Vec<ratatui::layout::Constraint> = column_widths
                    .iter()
                    .map(|&cw| ratatui::layout::Constraint::Length(cw))
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
                let rect = clipped(area, *x, *y, *w, *h);
                f.render_widget(widgets::Clear, rect);
            }
            RenderCmd::Text {
                x,
                y,
                text,
                fg,
                bg,
                bold,
            } => {
                let mut style = Style::default();
                if let Some([r, g, b]) = fg {
                    style = style.fg(Color::Rgb(*r, *g, *b));
                }
                if let Some([r, g, b]) = bg {
                    style = style.bg(Color::Rgb(*r, *g, *b));
                }
                if *bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                let rect = clipped(area, *x, *y, text.len() as u16, 1);
                f.render_widget(Paragraph::new(text.as_str()).style(style), rect);
            }
            RenderCmd::Rect { x, y, w, h, bg } => {
                let rect = clipped(area, *x, *y, *w, *h);
                let bg_color = Color::Rgb(bg[0], bg[1], bg[2]);
                f.render_widget(widgets::Clear, rect);
                f.render_widget(Block::default().style(Style::default().bg(bg_color)), rect);
            }
            RenderCmd::Border { x, y, w, h, fg } => {
                let rect = clipped(area, *x, *y, *w, *h);
                let fg_color = Color::Rgb(fg[0], fg[1], fg[2]);
                f.render_widget(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(fg_color)),
                    rect,
                );
            }
        }
    }
}
