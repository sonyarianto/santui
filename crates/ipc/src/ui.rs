use crate::protocol::{RenderCmd, ThemeData};

/// Draw a panel with a left border and background fill.
pub fn draw_panel(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    title: &str,
) {
    if w < 3 || h < 2 {
        return;
    }
    // Fill the panel body
    cmds.push(RenderCmd::Rect {
        x: x + 1,
        y,
        w: w.saturating_sub(1),
        h,
        bg: theme.background_panel,
    });
    // Left border
    for row in y..(y + h) {
        cmds.push(RenderCmd::Text {
            x,
            y: row,
            text: "\u{2503}".into(),
            fg: Some(theme.border),
            bg: None,
            bold: false,
        });
    }
    // Title
    cmds.push(RenderCmd::Text {
        x: x + 2,
        y,
        text: title.trim().into(),
        fg: Some(theme.text),
        bg: Some(theme.background_panel),
        bold: true,
    });
}

/// Truncate a string to fit within `max_len` characters, appending "…" if truncated.
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.len() > max_len && max_len > 1 {
        let t: String = text.chars().take(max_len.saturating_sub(1)).collect();
        format!("{t}…")
    } else {
        format!("{:<width$}", text, width = max_len)
    }
}

/// Render text at (x, y) on the panel background, truncated to `max_w` cells.
pub fn text_at(
    cmds: &mut Vec<RenderCmd>,
    x: u16,
    y: u16,
    text: &str,
    fg: [u8; 3],
    bg: [u8; 3],
    max_w: u16,
) {
    let display = truncate(text, max_w as usize);
    cmds.push(RenderCmd::Text {
        x,
        y,
        text: display,
        fg: Some(fg),
        bg: Some(bg),
        bold: false,
    });
}
