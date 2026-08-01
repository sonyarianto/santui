use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};

use crate::types::{self, Ayah, DisplayMode, Screen};
use crate::App;

pub fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let theme = app.theme.clone();
    let w = app.area.w.max(76);
    let h = app.area.h.max(18);
    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: theme.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        borders: BORDER_ALL,
        bg: Some(theme.background_panel),
        title: Some(" Quran ".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    match app.screen {
        Screen::SurahList => render_surah_list(app, &mut cmds, &theme, w, h),
        Screen::Reader => render_reader(app, &mut cmds, &theme, w, h),
        Screen::TranslationPicker => render_picker(
            app,
            &mut cmds,
            &theme,
            w,
            h,
            "Translation",
            &types::translation_options(),
        ),
        Screen::ReciterPicker => render_picker(
            app,
            &mut cmds,
            &theme,
            w,
            h,
            "Reciter",
            &types::reciter_options(),
        ),
    }
    cmds
}

fn render_surah_list(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let list = app.filtered_surahs();
    if app.search_mode {
        let header = format!(
            "Surahs: {} • Translation: {} • Reciter: {} • Search: {}",
            app.surahs.len(),
            app.prefs.translation_edition,
            app.prefs.reciter,
            app.search,
        );
        let truncated = santui_ipc::ui::truncate(&header, w as usize - 4);
        push_text(cmds, 2, 1, &truncated, theme.text_muted, false);
        if app.cursor_visible {
            let cx = 2u16 + truncated.chars().count() as u16;
            cmds.push(RenderCmd::Text {
                x: cx.min(w.saturating_sub(1)),
                y: 1,
                text: "█".into(),
                fg: Some(theme.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    } else {
        let header = format!(
            "Surahs: {} • Translation: {} • Reciter: {}",
            app.surahs.len(),
            app.prefs.translation_edition,
            app.prefs.reciter,
        );
        push_text(
            cmds,
            2,
            1,
            santui_ipc::ui::truncate(&header, w as usize - 4),
            theme.text_muted,
            false,
        );
    }
    if app.fetching {
        push_text(
            cmds,
            w.saturating_sub(12),
            1,
            "Loading...",
            theme.text_muted,
            false,
        );
    }
    let inner_w = w.saturating_sub(4) as usize;
    let playing = match &app.audio_state {
        types::AudioState::Buffering { surah, ayah }
        | types::AudioState::Playing { surah, ayah }
        | types::AudioState::Paused { surah, ayah } => Some((*surah, *ayah)),
        _ => None,
    };
    let cols = [
        "No",
        "Arabic",
        "Name",
        "Translation",
        "Ayahs",
        "Last Active",
    ];
    let col_w = [
        4usize,
        23usize,
        15usize.min(inner_w.saturating_sub(67)),
        25usize.min(inner_w.saturating_sub(57)),
        4usize,
        11usize,
    ];
    let list_h = h.saturating_sub(4).max(4);
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|s| {
            let active = match playing {
                Some((surah, ayah)) if surah == s.number => format!("{}", ayah),
                _ => app
                    .prefs
                    .per_surah_ayah
                    .get(&s.number)
                    .map_or("—".into(), |a| format!("{}", a)),
            };
            vec![
                format!("{}", s.number),
                santui_ipc::ui::truncate(&s.name, col_w[1]),
                santui_ipc::ui::truncate(&s.english_name, col_w[2]),
                santui_ipc::ui::truncate(&s.english_translation, col_w[3]),
                format!("{}", s.ayah_count),
                active,
            ]
        })
        .collect();
    cmds.push(RenderCmd::Table {
        x: 2,
        y: 3,
        w: inner_w as u16,
        h: list_h,
        header: cols.iter().map(|c| (*c).to_string()).collect(),
        header_style: TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: col_w.iter().map(|&c| c as u16).collect(),
        selected: Some(app.selected_surah.min(list.len().saturating_sub(1))),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
        current_row: None,
        current_style: None,
        cell_styles: None,
    });
}

fn render_reader(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let Some(content) = app.current_content() else {
        push_text(cmds, 2, 3, "No surah loaded", theme.error, true);
        return;
    };
    let header = format!(
        "{} ({}) · {} · mode: {} · audio: {}",
        content.summary.english_name,
        content.summary.name,
        content.summary.english_translation,
        app.prefs.display_mode.label(),
        app.audio_state.label(),
    );
    push_text(
        cmds,
        2,
        1,
        santui_ipc::ui::truncate(&header, w as usize - 4),
        theme.text,
        true,
    );
    let list_h = h.saturating_sub(4).max(4);
    let items: Vec<String> = content
        .ayahs
        .iter()
        .skip(app.scroll)
        .take(list_h as usize)
        .map(|ayah| ayah_row(ayah, app.prefs.display_mode, w.saturating_sub(8) as usize))
        .collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 3,
        w: w.saturating_sub(4),
        h: list_h,
        items,
        selected: app.selected_ayah.checked_sub(app.scroll),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
    });
}

fn render_picker(
    app: &App,
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    w: u16,
    h: u16,
    title: &str,
    options: &[&str],
) {
    push_text(
        cmds,
        2,
        1,
        santui_ipc::ui::truncate(title, w as usize - 4),
        theme.text_muted,
        false,
    );
    let items: Vec<String> = options.iter().map(|s| (*s).to_string()).collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 3,
        w: w.saturating_sub(4),
        h: h.saturating_sub(5),
        items,
        selected: Some(app.picker_cursor.min(options.len().saturating_sub(1))),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
    });
    push_text(
        cmds,
        2,
        h.saturating_sub(1),
        status_line(app),
        theme.text_muted,
        false,
    );
}

fn ayah_row(ayah: &Ayah, mode: DisplayMode, width: usize) -> String {
    let text = match mode {
        DisplayMode::Arabic => ayah.arabic.clone(),
        DisplayMode::Translation => ayah.translation.clone(),
        DisplayMode::Both => format!("{}  /  {}", ayah.arabic, ayah.translation),
    };
    format!("{:>3}. {}", ayah.number, wrap_text(&text, width))
}

fn status_line(app: &App) -> String {
    let mut parts = Vec::new();
    if !app.status.is_empty() {
        parts.push(app.status.as_str());
    }
    if app.repeat_ayah {
        parts.push("repeat on");
    }
    if app.fetching {
        parts.push("fetching");
    }
    parts.join(" · ")
}

pub fn hints(
    screen: Screen,
    _search_mode: bool,
    surahs_loaded: bool,
    _fetching: bool,
) -> Vec<(String, String)> {
    match screen {
        Screen::SurahList => {
            let mut v = vec![
                ("\u{2191}\u{2193}".into(), "navigate".into()),
                ("\u{21B5}".into(), "open".into()),
                ("/".into(), "search".into()),
                ("e".into(), "translation".into()),
                ("r".into(), "reciter".into()),
                ("esc".into(), "back".into()),
            ];
            if surahs_loaded {
                v.insert(3, ("p".into(), "play".into()));
                v.insert(4, ("s".into(), "stop".into()));
            }
            v
        }
        Screen::Reader => vec![
            ("\u{2191}\u{2193}".into(), "scroll".into()),
            ("p".into(), "play/pause".into()),
            ("a".into(), "play all".into()),
            ("s".into(), "stop".into()),
            ("t".into(), "mode".into()),
            ("r".into(), "repeat".into()),
            ("esc".into(), "list".into()),
        ],
        Screen::TranslationPicker | Screen::ReciterPicker => vec![
            ("\u{2191}\u{2193}".into(), "navigate".into()),
            ("\u{21B5}".into(), "select".into()),
            ("esc".into(), "back".into()),
        ],
    }
}

fn wrap_text(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if len <= max_chars {
        return text.to_string();
    }
    let mut result = String::new();
    let mut line_start = 0;
    while line_start < len {
        let line_end = (line_start + max_chars).min(len);
        if line_end == len {
            let s: String = chars[line_start..line_end].iter().collect();
            result.push_str(&s);
            break;
        }
        let segment = &chars[line_start..line_end];
        if let Some(space_pos) = segment.iter().rposition(|&c| c == ' ') {
            let split_at = line_start + space_pos;
            let s: String = chars[line_start..split_at].iter().collect();
            result.push_str(&s);
            result.push('\n');
            line_start = split_at + 1;
        } else {
            let s: String = segment.iter().collect();
            result.push_str(&s);
            result.push('\n');
            line_start = line_end;
        }
    }
    result
}

fn push_text(
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
