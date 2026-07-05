use crate::protocol::{RenderCmd, ThemeData, BORDER_ALL};

// ── Palette component (Ctrl+P style overlay) ──

/// Pre-computed palette rectangle dimensions.
pub struct PaletteRect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    /// Inner x (content offset, typically x + 2).
    pub ix: u16,
    /// Inner width (content width).
    pub iw: u16,
}

/// Compute a centered palette rectangle that fits `content_rows` lines.
/// Width matches the host Ctrl+P palette: outer width between 30 and 60.
pub fn palette_rect(area_w: u16, area_h: u16, content_rows: u16) -> PaletteRect {
    let max = area_w.saturating_sub(2);
    let ow = if max < 30 { max } else { max.clamp(30, 60) };
    let iw = ow.saturating_sub(4);
    let oh = content_rows;
    let ox = (area_w - ow) / 2;
    let oy = (area_h - oh) / 2;
    let ix = ox + 2;
    PaletteRect {
        x: ox,
        y: oy,
        w: ow,
        h: oh,
        ix,
        iw,
    }
}

/// Draw the palette backdrop (full overlay) and background rect.
pub fn palette_bg(cmds: &mut Vec<RenderCmd>, theme: &ThemeData, r: &PaletteRect) {
    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
        bg: theme.background_overlay,
    });
    cmds.push(RenderCmd::Rect {
        x: r.x,
        y: r.y,
        w: r.w,
        h: r.h,
        bg: theme.background_panel,
    });
}

/// Draw the palette title bar: bold title on the left, dimmed "esc" on the right.
pub fn palette_title(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    r: &PaletteRect,
    y_off: u16,
    title: &str,
) {
    let y = r.y + y_off;
    cmds.push(RenderCmd::Text {
        x: r.ix,
        y,
        text: title.into(),
        fg: Some(theme.text),
        bg: Some(theme.background_panel),
        bold: true,
        modifiers: 0,
    });
    // "esc" right-aligned, dimmed (matches host palette)
    cmds.push(RenderCmd::Text {
        x: r.ix + r.iw.saturating_sub(3),
        y,
        text: "esc".into(),
        fg: Some(theme.text_muted),
        bg: Some(theme.background_panel),
        bold: false,
        modifiers: 0,
    });
}

/// Draw a palette category header (bold accent).
pub fn palette_category(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    r: &PaletteRect,
    y_off: u16,
    label: &str,
) {
    cmds.push(RenderCmd::Text {
        x: r.ix,
        y: r.y + y_off,
        text: format!("{:<iw$}", label, iw = r.iw as usize),
        fg: Some(theme.accent),
        bg: Some(theme.background_panel),
        bold: true,
        modifiers: 0,
    });
}

/// Draw a palette item with selection highlighting.
pub fn palette_item(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    r: &PaletteRect,
    y_off: u16,
    label: &str,
    selected: bool,
) {
    cmds.push(RenderCmd::Text {
        x: r.ix,
        y: r.y + y_off,
        text: format!("{:<iw$}", label, iw = r.iw as usize),
        fg: if selected {
            Some(theme.inverted_text)
        } else {
            Some(theme.text)
        },
        bg: if selected {
            Some(theme.highlight)
        } else {
            Some(theme.background_panel)
        },
        bold: selected,
        modifiers: 0,
    });
}

// ── Panel component ──

/// Draw a full-box panel with title integrated into the top border (native ratatui style).
/// Content should be placed at `x + 2, y + 1` (inside the border).
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
    cmds.push(RenderCmd::Border {
        x,
        y,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title.trim().into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
}

/// Truncate a string to fit within `max_len` characters, appending "…" if truncated.
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.len() > max_len && max_len > 1 {
        let t: String = text.chars().take(max_len.saturating_sub(1)).collect();
        format!("{t}…")
    } else {
        text.to_string()
    }
}

/// Render text at (x, y), truncated to `max_w` cells.
pub fn text_at(
    cmds: &mut Vec<RenderCmd>,
    x: u16,
    y: u16,
    text: &str,
    fg: [u8; 3],
    bg: Option<[u8; 3]>,
    max_w: u16,
) {
    let display = truncate(text, max_w as usize);
    cmds.push(RenderCmd::Text {
        x,
        y,
        text: display,
        fg: Some(fg),
        bg,
        bold: false,
        modifiers: 0,
    });
}
