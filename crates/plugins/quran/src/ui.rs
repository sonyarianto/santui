use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};
use santui_ipc::ui;
use santui_ipc::ui::push_text;
use unicode_width::UnicodeWidthStr;

use crate::types::{self, Ayah, DisplayMode, Edition, Picker, Screen};
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
    }
    if let Some(picker) = app.picker {
        render_picker_panel(app, picker, &mut cmds, &theme, w, h);
    }
    cmds
}

fn render_picker_panel(
    app: &App,
    picker: Picker,
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    w: u16,
    h: u16,
) {
    let popup_w = (w * 2 / 5).max(20);
    let popup_x = w.saturating_sub(popup_w);
    if popup_x < 4 || h < 10 {
        return;
    }

    ui::popup_backdrop(cmds, theme, popup_x, 0, popup_w, h);

    let footer: &[(&str, &str)] = &[
        ("\u{2191}\u{2193}", "navigate"),
        ("\u{21B5}", "select"),
        ("esc", "close"),
    ];
    ui::draw_panel(
        cmds,
        theme,
        popup_x,
        0,
        popup_w,
        h,
        picker_label(picker),
        ui::PanelOpts {
            focused: true,
            footer: Some(footer),
            dim_unfocused: false,
        },
    );

    let inner_w = popup_w.saturating_sub(4) as usize;
    let options = app.picker_options(picker);

    if app.editions_loading {
        push_text(
            cmds,
            popup_x + 2,
            2,
            santui_ipc::ui::truncate("Loading editions...", inner_w),
            theme.text_muted,
            false,
        );
        return;
    }
    if options.is_empty() {
        push_text(
            cmds,
            popup_x + 2,
            2,
            santui_ipc::ui::truncate("Editions unavailable", inner_w),
            theme.text_muted,
            false,
        );
        return;
    }

    let current = match picker {
        Picker::Translation => &app.prefs.translation_edition,
        Picker::Reciter => &app.prefs.reciter,
    };
    let items: Vec<String> = options
        .iter()
        .map(|e| {
            let name = e.display_name();
            if &e.identifier == current {
                format!("\u{25CF} {}", santui_ipc::ui::truncate(&name, inner_w - 2))
            } else {
                format!("  {}", santui_ipc::ui::truncate(&name, inner_w - 2))
            }
        })
        .collect();
    let list_h = h.saturating_sub(5).max(4);
    cmds.push(RenderCmd::List {
        x: popup_x + 2,
        y: 2,
        w: inner_w as u16,
        h: list_h,
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
}

fn picker_label(picker: Picker) -> &'static str {
    match picker {
        Picker::Translation => "Translation",
        Picker::Reciter => "Reciter",
    }
}

fn translation_label(app: &App) -> String {
    app.translations
        .iter()
        .find(|e| e.identifier == app.prefs.translation_edition)
        .map(Edition::display_name)
        .unwrap_or_else(|| app.prefs.translation_edition.clone())
}

fn reciter_label(app: &App) -> String {
    app.reciters
        .iter()
        .find(|e| e.identifier == app.prefs.reciter)
        .map(Edition::display_name)
        .unwrap_or_else(|| app.prefs.reciter.clone())
}

/// Render `segments` sequentially on one row starting at `x0`, truncating once
/// `max_x` is reached. Returns the x position after the last rendered segment.
/// Each segment is `(text, fg, bold)`.
fn push_segments(
    cmds: &mut Vec<RenderCmd>,
    x0: u16,
    y: u16,
    max_x: u16,
    segments: &[(String, [u8; 3], bool)],
) -> u16 {
    let mut cx = x0;
    for (text, fg, bold) in segments {
        if cx >= max_x {
            break;
        }
        let t = ui::truncate(text, (max_x - cx) as usize);
        if !t.is_empty() {
            let n = UnicodeWidthStr::width(t.as_str()) as u16;
            push_text(cmds, cx, y, t, *fg, *bold);
            cx += n;
        }
    }
    cx
}

fn render_surah_list_header(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16) {
    let mut segments = vec![
        ("Surahs: ".to_string(), theme.text_muted, false),
        (app.surahs.len().to_string(), theme.text, false),
        (" • ".to_string(), theme.text_muted, false),
        ("Translation: ".to_string(), theme.text_muted, false),
        (translation_label(app), theme.text, false),
        (" • ".to_string(), theme.text_muted, false),
        ("Reciter: ".to_string(), theme.text_muted, false),
        (reciter_label(app), theme.text, false),
    ];
    if app.search_mode {
        segments.push((" • ".to_string(), theme.text_muted, false));
        segments.push(("Search: ".to_string(), theme.text_muted, false));
        segments.push((app.search.clone(), theme.text, false));
    }
    let end_x = push_segments(cmds, 2, 1, w.saturating_sub(2), segments.as_slice());
    if app.search_mode && app.cursor_visible {
        cmds.push(RenderCmd::Text {
            x: end_x.min(w.saturating_sub(1)),
            y: 1,
            text: "█".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }
}

fn render_surah_list(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let list = app.filtered_surahs();
    render_surah_list_header(app, cmds, theme, w);
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
    if !app.fetching && !app.status.is_empty() {
        push_text(
            cmds,
            2,
            h.saturating_sub(1),
            santui_ipc::ui::truncate(&status_line(app), inner_w),
            theme.text_muted,
            false,
        );
    }
}

fn render_reader(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let Some(content) = app.current_content() else {
        push_text(cmds, 2, 3, "No surah loaded", theme.error, true);
        return;
    };
    let segments = vec![
        (content.summary.english_name.clone(), theme.text, true),
        (format!(" ({})", content.summary.name), theme.text, true),
        (" - ".to_string(), theme.text_muted, false),
        (
            content.summary.english_translation.clone(),
            theme.text,
            true,
        ),
        (" - ".to_string(), theme.text_muted, false),
        ("mode: ".to_string(), theme.text_muted, false),
        (
            app.prefs.display_mode.label().to_string(),
            theme.text,
            false,
        ),
        (" - ".to_string(), theme.text_muted, false),
        ("audio: ".to_string(), theme.text_muted, false),
        (app.audio_state.label().to_string(), theme.text, false),
    ];
    push_segments(cmds, 2, 1, w.saturating_sub(2), segments.as_slice());
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
    parts.join(" · ")
}

pub fn hints(screen: Screen, picker: Option<Picker>, surahs_loaded: bool) -> Vec<(String, String)> {
    if picker.is_some() {
        return vec![
            ("\u{2191}\u{2193}".into(), "navigate".into()),
            ("\u{21B5}".into(), "select".into()),
            ("esc".into(), "close".into()),
        ];
    }
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
            ("e".into(), "translation".into()),
            ("r".into(), "repeat".into()),
            ("esc".into(), "list".into()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Edition;

    fn edition(id: &str, name: &str) -> Edition {
        Edition {
            identifier: id.into(),
            name: name.into(),
            english_name: name.into(),
            language: "en".into(),
            format: "text".into(),
            kind: "translation".into(),
        }
    }

    #[test]
    fn header_labels_use_display_names() {
        let mut app = App::default();
        app.translations = vec![edition("en.sahih", "Saheeh International")];
        app.reciters = vec![edition("ar.alafasy", "Alafasy")];
        app.prefs.translation_edition = "en.sahih".into();
        app.prefs.reciter = "ar.alafasy".into();
        assert_eq!(translation_label(&app), "Saheeh International");
        assert_eq!(reciter_label(&app), "Alafasy");
    }

    #[test]
    fn header_labels_fall_back_to_identifier() {
        let mut app = App::default();
        app.prefs.translation_edition = "en.unknown".into();
        app.prefs.reciter = "ar.unknown".into();
        assert_eq!(translation_label(&app), "en.unknown");
        assert_eq!(reciter_label(&app), "ar.unknown");
    }

    #[test]
    fn push_segments_renders_sequentially_and_truncates() {
        let theme = ThemeData {
            text: [255; 3],
            text_muted: [128; 3],
            accent: [255; 3],
            highlight: [255; 3],
            logo: [255; 3],
            background: [255; 3],
            background_panel: [255; 3],
            background_overlay: [255; 3],
            border: [255; 3],
            success: [255; 3],
            error: [255; 3],
            inverted_text: [255; 3],
        };
        let mut cmds = Vec::new();
        let end_x = push_segments(
            &mut cmds,
            2,
            1,
            20,
            &[
                ("Surahs: ".to_string(), theme.text_muted, false),
                ("01234567890123456789".to_string(), theme.text, true),
            ],
        );
        assert!(end_x <= 20);
        let rendered: String = cmds
            .iter()
            .map(|c| match c {
                RenderCmd::Text { text, .. } => text.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(rendered, "Surahs: 0123456...");
        assert_eq!(end_x, 20);
    }

    #[test]
    fn push_segments_counts_arabic_diacritics_as_zero_width() {
        let theme = ThemeData {
            text: [255; 3],
            text_muted: [128; 3],
            accent: [255; 3],
            highlight: [255; 3],
            logo: [255; 3],
            background: [255; 3],
            background_panel: [255; 3],
            background_overlay: [255; 3],
            border: [255; 3],
            success: [255; 3],
            error: [255; 3],
            inverted_text: [255; 3],
        };
        let mut cmds = Vec::new();
        let end_x = push_segments(
            &mut cmds,
            2,
            1,
            60,
            &[
                ("Aal-i-Imraan (".to_string(), theme.text, true),
                ("سُورَةُ آلِ عِمۡرَانَ".to_string(), theme.text, true),
                (") - The Family of Imraan".to_string(), theme.text, true),
            ],
        );
        let rendered: String = cmds
            .iter()
            .map(|c| match c {
                RenderCmd::Text { text, .. } => text.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(
            rendered,
            "Aal-i-Imraan (سُورَةُ آلِ عِمۡرَانَ) - The Family of Imraan"
        );
        assert_eq!(end_x, 2 + UnicodeWidthStr::width(rendered.as_str()) as u16);
    }
}
