use ratatui::style::Color;

fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

fn darken(hex: u32, factor: u8) -> Color {
    let r = ((hex >> 16) & 0xFF) as u16;
    let g = ((hex >> 8) & 0xFF) as u16;
    let b = (hex & 0xFF) as u16;
    let f = factor as u16;
    Color::Rgb(
        (r * f / 100) as u8,
        (g * f / 100) as u8,
        (b * f / 100) as u8,
    )
}

fn muted(neutral: u32, ink: u32) -> Color {
    let nr = (neutral >> 16) & 0xFF;
    let ng = (neutral >> 8) & 0xFF;
    let nb = neutral & 0xFF;
    let ir = (ink >> 16) & 0xFF;
    let ig = (ink >> 8) & 0xFF;
    let ib = ink & 0xFF;
    let r = ((nr as u16 * 60 + ir as u16 * 40) / 100) as u8;
    let g = ((ng as u16 * 60 + ig as u16 * 40) / 100) as u8;
    let b = ((nb as u16 * 60 + ib as u16 * 40) / 100) as u8;
    Color::Rgb(r, g, b)
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub accent: Color,
    pub highlight: Color,
    pub logo: Color,
    pub text: Color,
    pub text_muted: Color,
    pub background: Color,
    pub background_panel: Color,
    pub background_overlay: Color,
    pub border: Color,
    pub success: Color,
    pub error: Color,
    pub inverted_text: Color,
}

struct ThemeDef {
    name: &'static str,
    neutral: u32,
    ink: u32,
    primary: u32,
    accent: u32,
    success: u32,
    error: u32,
}

const THEMES: &[ThemeDef] = &[
    ThemeDef {
        name: "OpenCode",
        neutral: 0x0a0a0a,
        ink: 0xeeeeee,
        primary: 0xfab283,
        accent: 0x9d7cd8,
        success: 0x7fd88f,
        error: 0xe06c75,
    },
    ThemeDef {
        name: "Santui",
        neutral: 0x141414,
        ink: 0xffffff,
        primary: 0xffb900,
        accent: 0x9d7cd8,
        success: 0x7fd88f,
        error: 0xe06c75,
    },
    ThemeDef {
        name: "AMOLED",
        neutral: 0x000000,
        ink: 0xffffff,
        primary: 0xb388ff,
        accent: 0xff4081,
        success: 0x00ff88,
        error: 0xff1744,
    },
    ThemeDef {
        name: "Aura",
        neutral: 0x15141b,
        ink: 0xedecee,
        primary: 0xa277ff,
        accent: 0xff6767,
        success: 0x61ffca,
        error: 0xff6767,
    },
    ThemeDef {
        name: "Ayu",
        neutral: 0x0f1419,
        ink: 0xd6dae0,
        primary: 0x3fb7e3,
        accent: 0xf2856f,
        success: 0x78d05c,
        error: 0xf58572,
    },
    ThemeDef {
        name: "Carbonfox",
        neutral: 0x393939,
        ink: 0xf2f4f8,
        primary: 0x33b1ff,
        accent: 0xff8389,
        success: 0x42be65,
        error: 0xff8389,
    },
    ThemeDef {
        name: "Catppuccin Frappe",
        neutral: 0x303446,
        ink: 0xc6d0f5,
        primary: 0x8da4e2,
        accent: 0xf4b8e4,
        success: 0xa6d189,
        error: 0xe78284,
    },
    ThemeDef {
        name: "Catppuccin Macchiato",
        neutral: 0x24273a,
        ink: 0xcad3f5,
        primary: 0x8aadf4,
        accent: 0xf5bde6,
        success: 0xa6da95,
        error: 0xed8796,
    },
    ThemeDef {
        name: "Catppuccin",
        neutral: 0x1e1e2e,
        ink: 0xcdd6f4,
        primary: 0xb4befe,
        accent: 0xf38ba8,
        success: 0xa6d189,
        error: 0xf38ba8,
    },
    ThemeDef {
        name: "Cobalt2",
        neutral: 0x193549,
        ink: 0xffffff,
        primary: 0x0088ff,
        accent: 0x2affdf,
        success: 0x9eff80,
        error: 0xff0088,
    },
    ThemeDef {
        name: "Cursor",
        neutral: 0x181818,
        ink: 0xe4e4e4,
        primary: 0x88c0d0,
        accent: 0x88c0d0,
        success: 0x3fa266,
        error: 0xe34671,
    },
    ThemeDef {
        name: "Dracula",
        neutral: 0x1d1e28,
        ink: 0xf8f8f2,
        primary: 0xbd93f9,
        accent: 0xff79c6,
        success: 0x50fa7b,
        error: 0xff5555,
    },
    ThemeDef {
        name: "Everforest",
        neutral: 0x2d353b,
        ink: 0xd3c6aa,
        primary: 0xa7c080,
        accent: 0xd699b6,
        success: 0xa7c080,
        error: 0xe67e80,
    },
    ThemeDef {
        name: "Flexoki",
        neutral: 0x100f0f,
        ink: 0xcecdc3,
        primary: 0xda702c,
        accent: 0x8b7ec8,
        success: 0x879a39,
        error: 0xd14d41,
    },
    ThemeDef {
        name: "GitHub",
        neutral: 0x0d1117,
        ink: 0xc9d1d9,
        primary: 0x58a6ff,
        accent: 0x39c5cf,
        success: 0x3fb950,
        error: 0xf85149,
    },
    ThemeDef {
        name: "Gruvbox",
        neutral: 0x282828,
        ink: 0xebdbb2,
        primary: 0x83a598,
        accent: 0xfb4934,
        success: 0xb8bb26,
        error: 0xfb4934,
    },
    ThemeDef {
        name: "Kanagawa",
        neutral: 0x1f1f28,
        ink: 0xdcd7ba,
        primary: 0x7e9cd8,
        accent: 0xd27e99,
        success: 0x98bb6c,
        error: 0xe82424,
    },
    ThemeDef {
        name: "Lucent Orng",
        neutral: 0x2a1a15,
        ink: 0xeeeeee,
        primary: 0xec5b2b,
        accent: 0xfff7f1,
        success: 0x6ba1e6,
        error: 0xe06c75,
    },
    ThemeDef {
        name: "Material",
        neutral: 0x263238,
        ink: 0xeeffff,
        primary: 0x82aaff,
        accent: 0x89ddff,
        success: 0xc3e88d,
        error: 0xf07178,
    },
    ThemeDef {
        name: "Matrix",
        neutral: 0x0a0e0a,
        ink: 0x62ff94,
        primary: 0x2eff6a,
        accent: 0xc770ff,
        success: 0x62ff94,
        error: 0xff4b4b,
    },
    ThemeDef {
        name: "Mercury",
        neutral: 0x171721,
        ink: 0xdddde5,
        primary: 0x8da4f5,
        accent: 0x8da4f5,
        success: 0x77c599,
        error: 0xfc92b4,
    },
    ThemeDef {
        name: "Monokai",
        neutral: 0x272822,
        ink: 0xf8f8f2,
        primary: 0xae81ff,
        accent: 0xf92672,
        success: 0xa6e22e,
        error: 0xf92672,
    },
    ThemeDef {
        name: "Night Owl",
        neutral: 0x011627,
        ink: 0xd6deeb,
        primary: 0x82aaff,
        accent: 0xf78c6c,
        success: 0xc5e478,
        error: 0xef5350,
    },
    ThemeDef {
        name: "Nord",
        neutral: 0x2e3440,
        ink: 0xe5e9f0,
        primary: 0x88c0d0,
        accent: 0xd57780,
        success: 0xa3be8c,
        error: 0xbf616a,
    },
    ThemeDef {
        name: "OC-2",
        neutral: 0x1f1f1f,
        ink: 0xf1ece8,
        primary: 0xfab283,
        accent: 0xfab283,
        success: 0x12c905,
        error: 0xfc533a,
    },
    ThemeDef {
        name: "One Dark",
        neutral: 0x282c34,
        ink: 0xabb2bf,
        primary: 0x61afef,
        accent: 0x56b6c2,
        success: 0x98c379,
        error: 0xe06c75,
    },
    ThemeDef {
        name: "One Dark Pro",
        neutral: 0x1e222a,
        ink: 0xabb2bf,
        primary: 0x61afef,
        accent: 0xe06c75,
        success: 0x98c379,
        error: 0xe06c75,
    },
    ThemeDef {
        name: "Orng",
        neutral: 0x0a0a0a,
        ink: 0xeeeeee,
        primary: 0xec5b2b,
        accent: 0xfff7f1,
        success: 0x6ba1e6,
        error: 0xe06c75,
    },
    ThemeDef {
        name: "Osaka Jade",
        neutral: 0x111c18,
        ink: 0xc1c497,
        primary: 0x2dd5b7,
        accent: 0x549e6a,
        success: 0x549e6a,
        error: 0xff5345,
    },
    ThemeDef {
        name: "Palenight",
        neutral: 0x292d3e,
        ink: 0xa6accd,
        primary: 0x82aaff,
        accent: 0x89ddff,
        success: 0xc3e88d,
        error: 0xf07178,
    },
    ThemeDef {
        name: "Rose Pine",
        neutral: 0x191724,
        ink: 0xe0def4,
        primary: 0x9ccfd8,
        accent: 0xebbcba,
        success: 0x31748f,
        error: 0xeb6f92,
    },
    ThemeDef {
        name: "Shades of Purple",
        neutral: 0x1a102b,
        ink: 0xf5f0ff,
        primary: 0xc792ff,
        accent: 0xff7ac6,
        success: 0x7be0b0,
        error: 0xff7ac6,
    },
    ThemeDef {
        name: "Solarized",
        neutral: 0x002b36,
        ink: 0x93a1a1,
        primary: 0x6c71c4,
        accent: 0xd33682,
        success: 0x859900,
        error: 0xdc322f,
    },
    ThemeDef {
        name: "Synthwave '84",
        neutral: 0x262335,
        ink: 0xffffff,
        primary: 0x36f9f6,
        accent: 0xb084eb,
        success: 0x72f1b8,
        error: 0xfe4450,
    },
    ThemeDef {
        name: "Tokyonight",
        neutral: 0x1a1b26,
        ink: 0xc0caf5,
        primary: 0x7aa2f7,
        accent: 0xff9e64,
        success: 0x9ece6a,
        error: 0xf7768e,
    },
    ThemeDef {
        name: "Vercel",
        neutral: 0x000000,
        ink: 0xededed,
        primary: 0x0070f3,
        accent: 0x8e4ec6,
        success: 0x46a758,
        error: 0xe5484d,
    },
    ThemeDef {
        name: "Vesper",
        neutral: 0x101010,
        ink: 0xffffff,
        primary: 0xffc799,
        accent: 0xff8080,
        success: 0x99ffe4,
        error: 0xff8080,
    },
    ThemeDef {
        name: "Zenburn",
        neutral: 0x3f3f3f,
        ink: 0xdcdccc,
        primary: 0x8cd0d3,
        accent: 0x93e0e3,
        success: 0x7f9f7f,
        error: 0xcc9393,
    },
];

impl Theme {
    pub fn all() -> Vec<(&'static str, Self)> {
        THEMES
            .iter()
            .map(|d| {
                (
                    d.name,
                    Self {
                        accent: rgb(d.accent),
                        highlight: rgb(d.primary),
                        logo: rgb(d.primary),
                        text: rgb(d.ink),
                        text_muted: muted(d.neutral, d.ink),
                        background: Color::Reset,
                        background_panel: rgb(d.neutral),
                        background_overlay: darken(d.neutral, 40),
                        border: rgb(d.primary),
                        success: rgb(d.success),
                        error: rgb(d.error),
                        inverted_text: rgb(d.neutral),
                    },
                )
            })
            .collect()
    }
}

impl Default for Theme {
    fn default() -> Self {
        let d = &THEMES[1];
        Self {
            accent: rgb(d.accent),
            highlight: rgb(d.primary),
            logo: rgb(d.primary),
            text: rgb(d.ink),
            text_muted: muted(d.neutral, d.ink),
            background: Color::Reset,
            background_panel: rgb(d.neutral),
            background_overlay: darken(d.neutral, 40),
            border: rgb(d.primary),
            success: rgb(d.success),
            error: rgb(d.error),
            inverted_text: rgb(d.neutral),
        }
    }
}
