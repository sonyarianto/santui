use santui_ipc::protocol::{RenderCmd, ThemeData, BORDER_ALL};

use crate::api::{weather_description, weather_symbol, HourlyPoint};
use crate::state::{FetchState, Screen, WeatherState};

pub fn render_ui(state: &WeatherState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];

    match &state.screen {
        Screen::Overview => cmds.extend(render_overview(state, theme, w, h)),
        Screen::Hourly => cmds.extend(render_hourly(state, theme, w, h)),
        Screen::Daily => cmds.extend(render_daily(state, theme, w, h)),
        Screen::LocationSearch => {
            cmds.extend(render_overview(state, theme, w, h));
            cmds.extend(render_location_search(state, theme, w, h));
        }
    }

    cmds
}

fn render_overview(state: &WeatherState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let location_label = state
        .settings
        .location
        .as_ref()
        .map(|l| format!("{}, {}", l.name, l.country))
        .unwrap_or_else(|| "Weather".into());

    let updated_str = state.data.as_ref().map(|d| {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(d.fetched_at);
        if secs < 60 {
            "updated just now".into()
        } else {
            format!("updated {}m ago", secs / 60)
        }
    });

    let title = match &updated_str {
        Some(u) => format!("{} — {}", location_label, u),
        None => location_label,
    };

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    if state.settings.location.is_none() {
        cmds.push(RenderCmd::Text {
            x: w / 2 - 12,
            y: h / 2,
            text: "No location set.".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: w / 2 - 14,
            y: h / 2 + 1,
            text: "Press l to set your location.".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        return cmds;
    }

    match &state.fetch_state {
        FetchState::Fetching => {
            cmds.push(RenderCmd::Text {
                x: w / 2 - 10,
                y: h / 2,
                text: "⟳ Fetching weather...".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            return cmds;
        }
        FetchState::Error(e) => {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: h / 2,
                text: format!("⚠ {}", e),
                fg: Some(theme.error),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            // hints handled by status_hints
            return cmds;
        }
        _ => {}
    }

    if let Some(ref data) = state.data {
        let mut row = 1;

        let symbol = weather_symbol(data.current.weather_code);
        let desc = weather_description(data.current.weather_code);
        let temp_unit = match state.settings.units {
            crate::state::Units::Celsius => "°C",
            crate::state::Units::Fahrenheit => "°F",
        };

        cmds.push(RenderCmd::Text {
            x: 2,
            y: row,
            text: format!(
                " {}  {}  {:.0}{}",
                symbol, desc, data.current.temp, temp_unit
            ),
            fg: Some(theme.text),
            bg: None,
            bold: true,
            modifiers: 0,
        });
        row += 1;

        cmds.push(RenderCmd::Text {
            x: 2,
            y: row,
            text: format!(" feels like {:.0}{}", data.current.feels_like, temp_unit),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        row += 2;

        cmds.push(RenderCmd::Text {
            x: 2,
            y: row,
            text: format!(
                "💧 {}%  🌬 {:.0} km/h {:03}°  🌧 {:.1} mm",
                data.current.humidity,
                data.current.wind_speed,
                data.current.wind_dir,
                data.current.precip_mm
            ),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        row += 2;

        if !data.hourly.is_empty() {
            let hourly_str: String = data
                .hourly
                .iter()
                .take(5)
                .map(|h| {
                    format!(
                        "{:02}:00 {}{:.0}",
                        h.hour,
                        weather_symbol(h.weather_code),
                        h.temp
                    )
                })
                .collect::<Vec<_>>()
                .join("  ");
            cmds.push(RenderCmd::Text {
                x: 2,
                y: row,
                text: format!("Hourly  {}", hourly_str),
                fg: Some(theme.text),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            row += 1;
        }

        if !data.daily.is_empty() {
            let daily_str: String = data
                .daily
                .iter()
                .map(|d| {
                    let day = d.date.split('-').nth(2).unwrap_or("");
                    format!(
                        "{} {}{:.0}/{:.0}",
                        day,
                        weather_symbol(d.weather_code),
                        d.temp_max,
                        d.temp_min
                    )
                })
                .collect::<Vec<_>>()
                .join("  ");
            cmds.push(RenderCmd::Text {
                x: 2,
                y: row,
                text: format!("7-Day  {}", daily_str),
                fg: Some(theme.text),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    }

    cmds
}

fn render_hourly(state: &WeatherState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let location = state
        .settings
        .location
        .as_ref()
        .map(|l| l.name.as_str())
        .unwrap_or("Weather");

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(format!("Hourly Forecast — {}", location)),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    if let Some(ref data) = state.data {
        let scroll = state.hourly_scroll;
        let max_cols = (w.saturating_sub(4) / 5) as usize;
        let visible: Vec<&HourlyPoint> = data.hourly.iter().skip(scroll).take(max_cols).collect();

        if !visible.is_empty() {
            let row1: String = visible.iter().map(|h| format!("{:>4} ", h.hour)).collect();
            let row2: String = visible
                .iter()
                .map(|h| format!(" {}  ", weather_symbol(h.weather_code)))
                .collect();
            let row3: String = visible
                .iter()
                .map(|h| format!("{:>3} ", h.temp as u8))
                .collect();
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 1,
                text: row1,
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 2,
                text: row2,
                fg: Some(theme.text),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 3,
                text: row3,
                fg: Some(theme.accent),
                bg: None,
                bold: true,
                modifiers: 0,
            });
            let precip_color = |p: u8| -> [u8; 3] {
                if p < 30 {
                    theme.success
                } else if p < 70 {
                    theme.highlight
                } else {
                    theme.error
                }
            };
            for (i, h) in visible.iter().enumerate() {
                cmds.push(RenderCmd::Text {
                    x: 2 + (i as u16 * 5),
                    y: 4,
                    text: format!("{:>3}%", h.precip_prob),
                    fg: Some(precip_color(h.precip_prob)),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
            }
        }
    }

    cmds
}

fn render_daily(state: &WeatherState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let location = state
        .settings
        .location
        .as_ref()
        .map(|l| l.name.as_str())
        .unwrap_or("Weather");

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(format!("7-Day Forecast — {}", location)),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    if let Some(ref data) = state.data {
        let temp_unit = match state.settings.units {
            crate::state::Units::Celsius => "°C",
            crate::state::Units::Fahrenheit => "°F",
        };

        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: format!(
                "{:<6} {:<12} {:>8} {:>8} {:>10} {:>10}",
                "Day", "Cond", "High", "Low", "Precip", "Wind"
            ),
            fg: Some(theme.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        });

        for (i, day) in data.daily.iter().enumerate() {
            let y = 2 + i as u16;
            let is_selected = i == state.daily_cursor;
            let day_name = day_name_from_date(&day.date);
            let line = format!(
                "{:<6} {}{:<8} {:>6}{} {:>6}{} {:>7} mm {:>7} km/h",
                day_name,
                weather_symbol(day.weather_code),
                weather_description(day.weather_code),
                day.temp_max as u8,
                temp_unit,
                day.temp_min as u8,
                temp_unit,
                day.precip_mm,
                day.wind_max as u8,
            );

            cmds.push(RenderCmd::Text {
                x: 2,
                y,
                text: line,
                fg: if is_selected {
                    Some(theme.inverted_text)
                } else {
                    Some(theme.text)
                },
                bg: if is_selected {
                    Some(theme.accent)
                } else {
                    None
                },
                bold: is_selected,
                modifiers: 0,
            });
        }
    }

    cmds
}

fn render_location_search(
    state: &WeatherState,
    theme: &ThemeData,
    w: u16,
    h: u16,
) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let popup_w = 60u16.min(w.saturating_sub(4));
    let result_count = state.search_results.len();
    let popup_h = (12u16)
        .min(result_count.saturating_add(4) as u16)
        .min(h.saturating_sub(2));
    let popup_x = (w - popup_w) / 2;
    let popup_y = (h - popup_h) / 2;

    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
        bg: theme.background_overlay,
    });

    cmds.push(RenderCmd::Border {
        x: popup_x,
        y: popup_y,
        w: popup_w,
        h: popup_h,
        fg: theme.border,
        bg: Some(theme.background_overlay),
        borders: BORDER_ALL,
        title: Some("Set Location".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let input_text = format!("> {}", state.search_query);
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: popup_y + 1,
        text: santui_ipc::ui::truncate(&input_text, popup_w.saturating_sub(4) as usize),
        fg: Some(theme.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    if state.search_fetching {
        cmds.push(RenderCmd::Text {
            x: popup_x + 2,
            y: popup_y + 2,
            text: "⟳ Searching...".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else {
        let list_y = popup_y + 2;
        let max_visible = popup_h.saturating_sub(4) as usize;
        for (i, result) in state.search_results.iter().enumerate().take(max_visible) {
            let y = list_y + i as u16;
            let is_selected = i == state.search_cursor;
            let location_name = match &result.admin1 {
                Some(admin) => format!("{}, {}, {}", result.name, admin, result.country),
                None => format!("{}, {}", result.name, result.country),
            };
            let coords = format!("{:.4}, {:.4}", result.latitude, result.longitude);
            let line = format!(
                " {}  {}",
                santui_ipc::ui::truncate(
                    &location_name,
                    popup_w.saturating_sub(coords.len() as u16 + 8) as usize
                ),
                coords
            );

            cmds.push(RenderCmd::Text {
                x: popup_x + 2,
                y,
                text: line,
                fg: if is_selected {
                    Some(theme.accent)
                } else {
                    Some(theme.text)
                },
                bg: None,
                bold: is_selected,
                modifiers: 0,
            });
        }
    }

    cmds
}

fn day_name_from_date(date: &str) -> String {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(_y), Ok(_m), Ok(d)) = (
            parts[0].parse::<i32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        ) {
            let names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
            let day_of_week = ((d as f64
                + (13.0 * (_m as f64 + 1.0) / 5.0 + _y as f64 + _y as f64 / 4.0 - _y as f64 / 100.0
                    + _y as f64 / 400.0) as i32 as f64) as i32
                % 7) as usize;
            return names[day_of_week % 7].to_string();
        }
    }
    date.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> ThemeData {
        ThemeData {
            text: [200; 3],
            text_muted: [100; 3],
            accent: [180; 3],
            highlight: [220; 3],
            logo: [255; 3],
            background: [0; 3],
            background_panel: [20; 3],
            background_overlay: [10; 3],
            border: [150; 3],
            success: [80; 3],
            error: [255; 3],
            inverted_text: [255; 3],
        }
    }

    #[test]
    fn renders_no_location_prompt_when_no_location() {
        let state = WeatherState::default();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_prompt = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("No location set")),
        );
        assert!(has_prompt);
    }

    #[test]
    fn renders_fetching_indicator() {
        let mut state = WeatherState::default();
        state.settings.location = Some(crate::state::SavedLocation {
            name: "Tokyo".into(),
            country: "Japan".into(),
            latitude: 35.68,
            longitude: 139.69,
            timezone: "Asia/Tokyo".into(),
        });
        state.fetch_state = FetchState::Fetching;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_fetching = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Fetching")));
        assert!(has_fetching);
    }

    #[test]
    fn renders_current_temp_and_symbol() {
        let mut state = WeatherState::default();
        state.settings.location = Some(crate::state::SavedLocation {
            name: "Tokyo".into(),
            country: "Japan".into(),
            latitude: 35.68,
            longitude: 139.69,
            timezone: "Asia/Tokyo".into(),
        });
        state.data = Some(crate::api::WeatherData {
            current: crate::api::CurrentWeather {
                temp: 28.0,
                feels_like: 31.0,
                humidity: 72,
                precip_mm: 0.0,
                wind_speed: 14.0,
                wind_dir: 45,
                weather_code: 2,
            },
            hourly: vec![],
            daily: vec![],
            fetched_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
        state.fetch_state = FetchState::Done;
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_temp = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("28")));
        assert!(has_temp);
    }

    #[test]
    fn renders_error_state() {
        let mut state = WeatherState::default();
        state.settings.location = Some(crate::state::SavedLocation::default());
        state.fetch_state = FetchState::Error("network error".into());
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_error = cmds.iter().any(
            |c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("network error")),
        );
        assert!(has_error);
    }

    #[test]
    fn renders_location_search_overlay() {
        let mut state = WeatherState::default();
        state.screen = Screen::LocationSearch;
        state.search_query = "Tokyo".into();
        let cmds = render_ui(&state, &test_theme(), 80, 24);
        let has_overlay = cmds.iter().any(|c| {
            matches!(c, RenderCmd::Border { ref title, .. } if title.as_deref() == Some("Set Location"))
        });
        assert!(has_overlay);
    }
}
