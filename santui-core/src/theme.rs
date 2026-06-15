use ratatui::style::Color;

#[derive(Clone, Debug)]
pub struct Theme {
    pub accent: Color,
    pub highlight: Color,
    pub logo: Color,
    pub text: Color,
    pub text_muted: Color,
    pub background: Color,
    pub background_panel: Color,
    pub border: Color,
    pub success: Color,
    pub error: Color,
}

impl Theme {
    pub fn nord() -> Self {
        Self {
            accent: Color::Rgb(136, 192, 208),
            highlight: Color::Rgb(208, 135, 112),
            logo: Color::Rgb(236, 239, 244),
            text: Color::Rgb(236, 239, 244),
            text_muted: Color::Rgb(76, 86, 106),
            background: Color::Reset,
            background_panel: Color::Rgb(46, 52, 64),
            border: Color::Rgb(136, 192, 208),
            success: Color::Rgb(163, 190, 140),
            error: Color::Rgb(191, 97, 106),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Color::Rgb(157, 124, 216),
            highlight: Color::Rgb(250, 178, 131),
            logo: Color::Rgb(255, 185, 0),
            text: Color::White,
            text_muted: Color::DarkGray,
            background: Color::Reset,
            background_panel: Color::Rgb(20, 20, 20),
            border: Color::Rgb(157, 124, 216),
            success: Color::Green,
            error: Color::Red,
        }
    }
}
