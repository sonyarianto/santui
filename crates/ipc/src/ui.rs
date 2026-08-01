use crate::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};

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
    popup_backdrop(cmds, theme, r.x, r.y, r.w, r.h);
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

/// Options controlling how a panel is drawn.
#[derive(Clone, Copy, Debug)]
pub struct PanelOpts<'a> {
    /// Whether the panel has input focus (bright border).
    pub focused: bool,
    /// Dim the border while unfocused (`focused` wins when both are set).
    pub dim_unfocused: bool,
    /// Keyboard hints rendered in the bottom border area.
    pub footer: Option<&'a [(&'a str, &'a str)]>,
    /// Border drawn in `theme.highlight` to mark the active selection
    /// (wins over `focused` / `dim_unfocused`).
    pub selected: bool,
    /// Optional background fill for the panel.
    pub bg: Option<[u8; 3]>,
}

impl Default for PanelOpts<'_> {
    fn default() -> Self {
        Self {
            focused: true,
            dim_unfocused: false,
            footer: None,
            selected: false,
            bg: None,
        }
    }
}

/// Draw a full-box panel with optional title integrated into the top border
/// (native ratatui style). `title: None` draws a clean border box.
/// Content should be placed at `x + 2, y + 1` (inside the border).
#[allow(clippy::too_many_arguments)]
pub fn draw_panel(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    title: Option<&str>,
    opts: PanelOpts<'_>,
) {
    if w < 3 || h < 2 {
        return;
    }
    let border_fg = if opts.selected {
        theme.highlight
    } else if opts.focused || !opts.dim_unfocused {
        theme.border
    } else {
        theme.text_muted
    };
    cmds.push(RenderCmd::Border {
        x,
        y,
        w,
        h,
        fg: border_fg,
        bg: opts.bg,
        borders: BORDER_ALL,
        title: title.map(|t| t.trim().to_string()),
        title_fg: Some(border_fg),
        title_dash_fg: Some(border_fg),
        border_type: None,
    });

    if let Some(hints) = opts.footer {
        let max_chars = w.saturating_sub(3) as usize;
        hints_row(cmds, theme, x + 2, y + h - 2, hints, max_chars);
    }
}

/// Draw one row of `key`/`desc` hint pairs using the standard palette
/// footer style: keys in `theme.text`, descriptions and ` • ` separators
/// in `theme.text_muted`. Content is truncated to `max_chars`.
pub fn hints_row(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    x: u16,
    y: u16,
    hints: &[(&str, &str)],
    max_chars: usize,
) {
    let mut cx = x;
    let mut remaining = max_chars;
    for (i, (key, desc)) in hints.iter().enumerate() {
        if remaining == 0 {
            break;
        }
        let k: String = key.chars().take(remaining).collect();
        if !k.is_empty() {
            let kw = k.chars().count();
            cmds.push(RenderCmd::Text {
                x: cx,
                y,
                text: k,
                fg: Some(theme.text),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            cx += kw as u16;
            remaining -= kw;
        }
        if remaining == 0 {
            break;
        }
        if !desc.is_empty() {
            let sep = if i + 1 < hints.len() {
                " \u{2022} "
            } else {
                ""
            };
            let space_needed = 1 + desc.chars().count() + sep.chars().count();
            if space_needed <= remaining {
                let span = format!(" {desc}{sep}");
                cmds.push(RenderCmd::Text {
                    x: cx,
                    y,
                    text: span,
                    fg: Some(theme.text_muted),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
                cx += space_needed as u16;
                remaining -= space_needed;
            } else {
                let d: String = desc.chars().take(remaining.saturating_sub(1)).collect();
                if !d.is_empty() {
                    cmds.push(RenderCmd::Text {
                        x: cx,
                        y,
                        text: format!(" {d}"),
                        fg: Some(theme.text_muted),
                        bg: None,
                        bold: false,
                        modifiers: 0,
                    });
                    cx += (1 + d.chars().count()) as u16;
                    remaining = 0;
                }
            }
        }
    }
}

/// Draw the standard palette footer: one blank row, the `hints` row, and
/// another blank row, anchored to the bottom of `r`.
pub fn palette_footer(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    r: &PaletteRect,
    hints: &[(&str, &str)],
) {
    let y = r.y + r.h - 2;
    hints_row(cmds, theme, r.ix, y, hints, r.iw as usize);
}

/// Truncate a string to fit within `max_len` characters, appending "..." if truncated.
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.chars().count() > max_len && max_len > 3 {
        let t: String = text.chars().take(max_len.saturating_sub(3)).collect();
        format!("{t}...")
    } else if text.chars().count() > max_len {
        text.chars().take(max_len).collect()
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

// ── Shared UI primitives ──

/// Blinking cursor character (█ toggling on a 6-tick cycle) for text inputs.
pub fn blink_cursor(tick_counter: u64) -> char {
    if tick_counter % 6 < 3 {
        '█'
    } else {
        ' '
    }
}

/// X coordinate to right-align `text` within an area of width `area_w`
/// (one cell margin from the right edge).
pub fn right_align_x(area_w: u16, text: &str) -> u16 {
    area_w.saturating_sub(2u16.saturating_add(text.chars().count() as u16))
}

/// Scroll percentage (0-100) for a scrollable list; 0 when not scrolled.
pub fn scroll_pct(scroll: usize, len: usize, visible: usize) -> u8 {
    if len > visible.max(1) && scroll > 0 {
        let max_scroll = len.saturating_sub(visible.max(1));
        (scroll * 100).checked_div(max_scroll).unwrap_or(0).min(100) as u8
    } else {
        0
    }
}

/// Index of `selected` relative to the visible window, or None when scrolled out.
pub fn vis_selected(selected: usize, scroll: usize, visible_count: usize) -> Option<usize> {
    if selected >= scroll && selected < scroll + visible_count {
        Some(selected - scroll)
    } else {
        None
    }
}

/// "♥ " prefix for favorited rows, two spaces otherwise (keeps columns aligned).
pub fn fav_prefix(favorite: bool) -> &'static str {
    if favorite {
        "♥ "
    } else {
        "  "
    }
}

/// Table text styles: muted bold header, plain body, inverted highlight.
#[derive(Clone, Copy)]
pub struct TableStyles {
    pub header: TextStyle,
    pub body: TextStyle,
    pub highlight: TextStyle,
}

/// Standard table text styles derived from the theme.
pub fn table_styles(theme: &ThemeData) -> TableStyles {
    TableStyles {
        header: TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        body: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
    }
}

/// Dim the whole screen as a popup backdrop.
pub fn dim_overlay(cmds: &mut Vec<RenderCmd>, theme: &ThemeData) {
    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
        bg: theme.background_overlay,
    });
}

/// Popup backdrop: dim the whole screen and draw a background rect.
pub fn popup_backdrop(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
) {
    dim_overlay(cmds, theme);
    cmds.push(RenderCmd::Rect {
        x,
        y,
        w,
        h,
        bg: theme.background_panel,
    });
}

/// Red heart marker at the start of each row that is a favorite.
pub fn heart_overlay(
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    table_top: u16,
    vis_selected: Option<usize>,
    count: usize,
    is_favorite: impl Fn(usize) -> bool,
) {
    for i in 0..count {
        if is_favorite(i) {
            let bg = if vis_selected == Some(i) {
                Some(theme.highlight)
            } else {
                None
            };
            cmds.push(RenderCmd::Text {
                x: 2,
                y: table_top + 1 + i as u16,
                text: "♥".into(),
                fg: Some([255, 60, 60]),
                bg,
                bold: false,
                modifiers: 0,
            });
        }
    }
}

/// Number of rows that fit in a results table given the total area height.
pub fn max_visible_tracks(h: u16) -> usize {
    h.saturating_sub(5) as usize
}

/// Push a `Text` render command with explicit bold styling.
pub fn push_text(
    cmds: &mut Vec<RenderCmd>,
    x: u16,
    y: u16,
    text: impl Into<String>,
    fg: [u8; 3],
    bold: bool,
) {
    cmds.push(RenderCmd::Text {
        x,
        y,
        text: text.into(),
        fg: Some(fg),
        bg: None,
        bold,
        modifiers: 0,
    });
}

/// Prefix `" > "` when `active` matches `field`, `"   "` otherwise.
pub fn focus_line(active: bool, label: &str, value: &str) -> String {
    format!("{} {label}: {value}", if active { ">" } else { " " })
}

/// Keep `scroll` clamped around `selected` for a list of height `area_h`,
/// mirroring the host list behaviour (5 rows reserved for header/footer).
pub fn update_scroll(scroll: &mut u16, selected: usize, area_h: u16) {
    let list_h = area_h.saturating_sub(5) as usize;
    if selected < *scroll as usize {
        *scroll = selected as u16;
    }
    if selected >= *scroll as usize + list_h {
        *scroll = (selected.saturating_sub(list_h).saturating_add(1)) as u16;
    }
}

/// Move `scroll` up so `selected` stays visible.
pub fn scroll_up(scroll: &mut usize, selected: usize) {
    if selected < *scroll {
        *scroll = selected;
    }
}

/// Move `scroll` down so `selected` stays visible (5 rows reserved for header/footer).
pub fn scroll_down(scroll: &mut usize, selected: usize, area_h: u16) {
    let max_visible = max_visible_tracks(area_h);
    if selected >= *scroll + max_visible {
        *scroll = selected.saturating_sub(max_visible.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::{draw_panel, hints_row, max_visible_tracks, PanelOpts};
    use crate::protocol::RenderCmd;
    use crate::test::theme;

    #[test]
    fn hints_row_uses_standard_footer_colors() {
        let mut cmds = Vec::new();
        hints_row(
            &mut cmds,
            &theme(),
            0,
            0,
            &[("↑↓", "navigate"), ("↵", "select")],
            100,
        );
        let parts: Vec<(&str, u16, [u8; 3])> = cmds
            .iter()
            .map(|c| match c {
                RenderCmd::Text { text, x, fg, .. } => (text.as_str(), *x, fg.unwrap()),
                _ => panic!("expected text"),
            })
            .collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], ("↑↓", 0, theme().text));
        assert_eq!(parts[1], (" navigate • ", 2, theme().text_muted));
        assert_eq!(parts[2], ("↵", 14, theme().text));
        assert_eq!(parts[3], (" select", 15, theme().text_muted));
    }

    #[test]
    fn hints_row_truncates_desc_when_narrow() {
        let mut cmds = Vec::new();
        hints_row(&mut cmds, &theme(), 0, 0, &[("↑↓", "navigate")], 6);
        let parts: Vec<(&str, u16)> = cmds
            .iter()
            .map(|c| match c {
                RenderCmd::Text { text, x, .. } => (text.as_str(), *x),
                _ => panic!("expected text"),
            })
            .collect();
        assert_eq!(parts, vec![("↑↓", 0), (" nav", 2)]);
    }

    #[test]
    fn hints_row_skips_last_separator() {
        let mut cmds = Vec::new();
        hints_row(
            &mut cmds,
            &theme(),
            0,
            0,
            &[("a", "add"), ("d", "delete"), ("esc", "cancel")],
            100,
        );
        let texts: Vec<&str> = cmds
            .iter()
            .filter_map(|c| match c {
                RenderCmd::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts,
            vec!["a", " add • ", "d", " delete • ", "esc", " cancel"]
        );
    }

    #[test]
    fn max_visible_tracks_calculation() {
        assert_eq!(max_visible_tracks(24), 19);
        assert_eq!(max_visible_tracks(10), 5);
        assert_eq!(max_visible_tracks(5), 0);
    }

    #[test]
    fn draw_panel_title_none_draws_clean_border() {
        let mut cmds = Vec::new();
        draw_panel(&mut cmds, &theme(), 0, 0, 26, 7, None, PanelOpts::default());
        let borders: Vec<&RenderCmd> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { .. }))
            .collect();
        assert_eq!(borders.len(), 1);
        if let RenderCmd::Border {
            title,
            title_fg,
            bg,
            ..
        } = borders[0]
        {
            assert_eq!(title.as_deref(), None);
            assert_eq!(*title_fg, Some(theme().border));
            assert_eq!(*bg, None);
        } else {
            panic!("expected border");
        }
    }

    #[test]
    fn draw_panel_selected_uses_highlight_border() {
        let mut cmds = Vec::new();
        draw_panel(
            &mut cmds,
            &theme(),
            0,
            0,
            26,
            7,
            None,
            PanelOpts {
                selected: true,
                ..PanelOpts::default()
            },
        );
        if let RenderCmd::Border { fg, .. } = &cmds[0] {
            assert_eq!(*fg, theme().highlight);
        } else {
            panic!("expected border");
        }
    }

    #[test]
    fn draw_panel_unfocused_dimmed_uses_muted_border() {
        let mut cmds = Vec::new();
        draw_panel(
            &mut cmds,
            &theme(),
            0,
            0,
            26,
            7,
            None,
            PanelOpts {
                focused: false,
                dim_unfocused: true,
                ..PanelOpts::default()
            },
        );
        if let RenderCmd::Border { fg, .. } = &cmds[0] {
            assert_eq!(*fg, theme().text_muted);
        } else {
            panic!("expected border");
        }
    }

    #[test]
    fn draw_panel_bg_fills_panel() {
        let mut cmds = Vec::new();
        let bg = [9, 9, 9];
        draw_panel(
            &mut cmds,
            &theme(),
            0,
            0,
            30,
            4,
            Some("Rename"),
            PanelOpts {
                bg: Some(bg),
                ..PanelOpts::default()
            },
        );
        if let RenderCmd::Border { bg: got, title, .. } = &cmds[0] {
            assert_eq!(*got, Some(bg));
            assert_eq!(title.as_deref(), Some("Rename"));
        } else {
            panic!("expected border");
        }
    }

    #[test]
    fn draw_panel_too_small_draws_nothing() {
        let mut cmds = Vec::new();
        draw_panel(&mut cmds, &theme(), 0, 0, 2, 2, None, PanelOpts::default());
        assert!(cmds.is_empty());
    }
}
