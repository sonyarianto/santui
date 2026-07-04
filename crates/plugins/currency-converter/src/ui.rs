use crate::state::{CurrencyState, FetchState, InputMode};
use santui_ipc::protocol::{RenderCmd, ThemeData};

pub fn render_ui(state: &CurrencyState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    match &state.input_mode {
        InputMode::BrowseCurrencies => render_browse(state, theme, w, h),
        InputMode::Favorites => render_favorites(state, theme, w, h),
        _ => render_main(state, theme, w, h),
    }
}

fn divider(_theme: &ThemeData, w: u16) -> String {
    "\u{2550}".repeat(w.saturating_sub(2) as usize)
}

fn render_main(state: &CurrencyState, theme: &ThemeData, w: u16, _h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let amount = state.amount_input.clone();
    let parsed = state.parsed_amount;
    let source = &state.source_currency;
    let target = &state.target_currency;

    // Title bar
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h: 1,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: Some(" Currency Converter ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });

    // Amount field
    cmds.push(RenderCmd::Text {
        x: 3,
        y: 2,
        text: "Amount:".into(),
        fg: Some(theme.accent),
        bg: None,
        bold: true,
    });
    let amount_style = if matches!(state.input_mode, InputMode::Amount) {
        format!(
            "[ {} ]",
            if amount.is_empty() {
                "0".into()
            } else {
                amount.clone()
            }
        )
    } else {
        format!("  {}  ", if amount.is_empty() { "0" } else { &amount })
    };
    cmds.push(RenderCmd::Text {
        x: 12,
        y: 2,
        text: amount_style,
        fg: Some(theme.text),
        bg: if matches!(state.input_mode, InputMode::Amount) {
            Some(theme.background_overlay)
        } else {
            None
        },
        bold: matches!(state.input_mode, InputMode::Amount),
    });

    // Source currency
    cmds.push(RenderCmd::Text {
        x: 3,
        y: 3,
        text: "Source:".into(),
        fg: Some(theme.accent),
        bg: None,
        bold: true,
    });
    let source_style = if matches!(state.input_mode, InputMode::Source) {
        format!("[ {} ]", source)
    } else {
        format!("  {}  ", source)
    };
    cmds.push(RenderCmd::Text {
        x: 12,
        y: 3,
        text: source_style,
        fg: Some(theme.text),
        bg: if matches!(state.input_mode, InputMode::Source) {
            Some(theme.background_overlay)
        } else {
            None
        },
        bold: matches!(state.input_mode, InputMode::Source),
    });

    // Target currency
    cmds.push(RenderCmd::Text {
        x: 3,
        y: 4,
        text: "Target:".into(),
        fg: Some(theme.accent),
        bg: None,
        bold: true,
    });
    let target_style = if matches!(state.input_mode, InputMode::Target) {
        format!("[ {} ]", target)
    } else {
        format!("  {}  ", target)
    };
    cmds.push(RenderCmd::Text {
        x: 12,
        y: 4,
        text: target_style,
        fg: Some(theme.text),
        bg: if matches!(state.input_mode, InputMode::Target) {
            Some(theme.background_overlay)
        } else {
            None
        },
        bold: matches!(state.input_mode, InputMode::Target),
    });

    // Result
    cmds.push(RenderCmd::Border {
        x: 2,
        y: 6,
        w: w.saturating_sub(4),
        h: 3,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: None,
        title_fg: None,
        title_dash_fg: None,
    });

    match &state.fetch_state {
        FetchState::Fetching => {
            cmds.push(RenderCmd::Text {
                x: (w / 2).saturating_sub(8),
                y: 7,
                text: "\u{27f3} Fetching rates...".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
            });
        }
        FetchState::Error(e) => {
            cmds.push(RenderCmd::Text {
                x: (w / 2).saturating_sub(e.len() as u16 / 2),
                y: 7,
                text: e.clone(),
                fg: Some(theme.error),
                bg: None,
                bold: false,
            });
        }
        _ => {
            if let Some(amount) = parsed {
                if let Some(target_rate) = state.target_rate() {
                    let source_rate = state.source_to_usd_rate();
                    let result = crate::api::convert(amount, source_rate, target_rate);
                    let result_text = format!("{} {} = {:.4} {}", amount, source, result, target);
                    cmds.push(RenderCmd::Text {
                        x: (w / 2).saturating_sub(result_text.len() as u16 / 2),
                        y: 7,
                        text: result_text,
                        fg: Some(theme.success),
                        bg: None,
                        bold: true,
                    });
                }
            }
        }
    }

    // Last update line
    if !state.rates_last_update.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 3,
            y: 10,
            text: format!(
                "1 {} = ? {}  Last updated: {}",
                source, target, state.rates_last_update
            ),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
    }

    // Favorites section
    if !state.favorite_pairs.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 3,
            y: 12,
            text: divider(theme, w),
            fg: Some(theme.border),
            bg: None,
            bold: false,
        });
        cmds.push(RenderCmd::Text {
            x: 3,
            y: 13,
            text: "Favorites:".into(),
            fg: Some(theme.accent),
            bg: None,
            bold: true,
        });

        let y_start = 14u16;
        for (i, (s, t)) in state.favorite_pairs.iter().take(6).enumerate() {
            let rate_text = if let Some(rate) = state.rates.get(t) {
                let source_rate = if s == &state.rates_base {
                    1.0
                } else {
                    state.rates.get(s).copied().unwrap_or(1.0)
                };
                let v = crate::api::convert(1.0, source_rate, *rate);
                format!("{} -> {}  {:.4}", s, t, v)
            } else {
                format!("{} -> {}", s, t)
            };
            cmds.push(RenderCmd::Text {
                x: 3,
                y: y_start + i as u16,
                text: rate_text,
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
            });
        }
    }

    cmds
}

fn render_browse(state: &CurrencyState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    // Dim background
    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w,
        h,
        bg: theme.background_overlay,
    });

    let popup_w = 40u16;
    let popup_x = (w.saturating_sub(popup_w)) / 2;
    let popup_h = (state.browse_results.len() as u16 + 4).min(h - 2);

    cmds.push(RenderCmd::Border {
        x: popup_x,
        y: 1,
        w: popup_w,
        h: popup_h,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: Some(" Select Currency ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });

    // Search query
    let display_query = format!("> {}", state.browse_query);
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: 2,
        text: display_query,
        fg: Some(theme.text),
        bg: None,
        bold: false,
    });

    let start_idx = 0usize;
    let visible = (popup_h as usize).saturating_sub(3);
    let results: Vec<&str> = state
        .browse_results
        .iter()
        .skip(start_idx)
        .take(visible)
        .map(|s| s.as_str())
        .collect();

    for (i, code) in results.iter().enumerate() {
        let marker = if i == state.browse_cursor {
            "\u{25b6}"
        } else {
            " "
        };
        let line = format!("{} {}", marker, code);
        let fg = if i == state.browse_cursor {
            Some(theme.accent)
        } else {
            Some(theme.text)
        };
        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y: 3 + i as u16,
            text: line,
            fg,
            bg: None,
            bold: i == state.browse_cursor,
        });
    }

    cmds
}

fn render_favorites(state: &CurrencyState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w,
        h,
        bg: theme.background_overlay,
    });

    let popup_w = 50u16;
    let popup_x = (w.saturating_sub(popup_w)) / 2;
    let popup_h = (state.favorite_pairs.len() as u16 + 3).min(h - 2);

    cmds.push(RenderCmd::Border {
        x: popup_x,
        y: 1,
        w: popup_w,
        h: popup_h,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: Some(" Favorites ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });

    for (i, (s, t)) in state.favorite_pairs.iter().enumerate() {
        let marker = if i == state.fav_cursor {
            "\u{25b6}"
        } else {
            " "
        };
        let line = format!("{} {} -> {}", marker, s, t);
        let fg = if i == state.fav_cursor {
            Some(theme.accent)
        } else {
            Some(theme.text)
        };
        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y: 2 + i as u16,
            text: line,
            fg,
            bg: None,
            bold: i == state.fav_cursor,
        });
    }

    cmds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::CurrencyState;

    fn default_theme() -> ThemeData {
        ThemeData {
            text: [220; 3],
            text_muted: [140; 3],
            accent: [180; 3],
            highlight: [220; 3],
            logo: [255; 3],
            background: [0; 3],
            background_panel: [20; 3],
            background_overlay: [10; 3],
            border: [150; 3],
            success: [0; 3],
            error: [255; 3],
            inverted_text: [255; 3],
        }
    }

    #[test]
    fn renders_main_view() {
        let state = CurrencyState::default();
        let commands = render_main(&state, &default_theme(), 80, 24);
        assert!(!commands.is_empty());
    }

    #[test]
    fn renders_fetching_indicator() {
        let mut state = CurrencyState::default();
        state.fetch_state = FetchState::Fetching;
        let commands = render_main(&state, &default_theme(), 80, 24);
        let has_fetching = commands
            .iter()
            .any(|cmd| matches!(cmd, RenderCmd::Text { text, .. } if text.contains("Fetching")));
        assert!(has_fetching);
    }

    #[test]
    fn renders_browse_overlay() {
        let mut state = CurrencyState::default();
        state.input_mode = InputMode::BrowseCurrencies;
        state.rates.insert("USD".into(), 1.0);
        state.rates.insert("EUR".into(), 0.92);
        state.filter_currencies();
        let commands = render_ui(&state, &default_theme(), 80, 24);
        assert!(!commands.is_empty());
    }

    #[test]
    fn renders_favorites_overlay() {
        let mut state = CurrencyState::default();
        state.input_mode = InputMode::Favorites;
        state.favorite_pairs = vec![("USD".into(), "EUR".into())];
        let commands = render_ui(&state, &default_theme(), 80, 24);
        assert!(!commands.is_empty());
    }

    #[test]
    fn renders_error_state() {
        let mut state = CurrencyState::default();
        state.fetch_state = FetchState::Error("Network error".into());
        let commands = render_main(&state, &default_theme(), 80, 24);
        let has_error = commands.iter().any(
            |cmd| matches!(cmd, RenderCmd::Text { text, .. } if text.contains("Network error")),
        );
        assert!(has_error);
    }
}
