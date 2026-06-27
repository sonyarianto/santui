use ratatui::layout::Rect;

pub fn centered_rect(parent: Rect, min_w: u16, ideal_w: u16, height: u16) -> Rect {
    let width = ideal_w
        .min(parent.width.saturating_sub(2))
        .max(min_w)
        .min(parent.width);
    let x = parent.x + (parent.width.saturating_sub(width)) / 2;
    let y = parent.y + (parent.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_basic() {
        let r = centered_rect(Rect::new(0, 0, 100, 50), 30, 40, 20);
        assert_eq!((r.x, r.y, r.width, r.height), (30, 15, 40, 20));
    }

    #[test]
    fn centered_rect_clamps_ideal_width_to_parent_minus_2() {
        let r = centered_rect(Rect::new(0, 0, 50, 50), 10, 100, 10);
        assert_eq!(r.width, 48);
    }

    #[test]
    fn centered_rect_enforces_min_width() {
        let r = centered_rect(Rect::new(0, 0, 100, 50), 80, 10, 20);
        assert_eq!(r.width, 80);
    }

    #[test]
    fn centered_rect_caps_min_width_to_parent() {
        let r = centered_rect(Rect::new(0, 0, 5, 50), 10, 20, 10);
        assert_eq!(r.width, 5);
    }

    #[test]
    fn centered_rect_with_parent_offset() {
        let r = centered_rect(Rect::new(10, 5, 100, 50), 30, 40, 20);
        assert_eq!((r.x, r.y), (40, 20));
    }
}
