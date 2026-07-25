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
    let header = format!(
        "Surahs: {} · Search: {} · Translation: {} · Reciter: {}",
        app.surahs.len(),
        app.search,
        app.prefs.translation_edition,
        app.prefs.reciter
    );
    push_text(
        cmds,
        2,
        2,
        truncate(&header, w as usize - 4),
        theme.text,
        true,
    );
    let list_h = h.saturating_sub(7).max(4);
    let items: Vec<String> = list
        .iter()
        .map(|s| {
            format!(
                "{:>3}. {:<24} {:<24} {} ayahs",
                s.number, s.english_name, s.english_translation, s.ayah_count
            )
        })
        .collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 4,
        w: w.saturating_sub(4),
        h: list_h,
        items,
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
    });
    push_text(
        cmds,
        2,
        h.saturating_sub(2),
        status_line(app),
        theme.text_muted,
        false,
    );
}

fn render_reader(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let Some(content) = app.current_content() else {
        push_text(cmds, 2, 4, "No surah loaded", theme.error, true);
        return;
    };
    let header = format!(
        "{} ({}) · {} · mode: {} · audio: {}",
        content.summary.english_name,
        content.summary.name,
        content.summary.english_translation,
        app.prefs.display_mode.label(),
        app.audio_state.label()
    );
    push_text(
        cmds,
        2,
        2,
        truncate(&header, w as usize - 4),
        theme.text,
        true,
    );
    let list_h = h.saturating_sub(7).max(4);
    let items: Vec<String> = content
        .ayahs
        .iter()
        .skip(app.scroll)
        .take(list_h as usize)
        .map(|ayah| ayah_row(ayah, app.prefs.display_mode, w.saturating_sub(8) as usize))
        .collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 4,
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
    push_text(
        cmds,
        2,
        h.saturating_sub(2),
        status_line(app),
        theme.text_muted,
        false,
    );
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
        2,
        format!("Choose {title} · Enter select · Esc cancel"),
        theme.text,
        true,
    );
    let items: Vec<String> = options.iter().map(|s| (*s).to_string()).collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 4,
        w: w.saturating_sub(4),
        h: h.saturating_sub(7),
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
    format!("{:>3}. {}", ayah.number, truncate(&text, width))
}

fn status_line(app: &App) -> String {
    let repeat = if app.repeat_ayah {
        "repeat on"
    } else {
        "repeat off"
    };
    let fetching = if app.fetching { " · fetching" } else { "" };
    format!("{} · {}{}", app.status, repeat, fetching)
}

pub fn hints(screen: Screen) -> Vec<(String, String)> {
    match screen {
        Screen::SurahList => vec![
            ("enter".into(), "read".into()),
            ("/".into(), "search".into()),
            ("e".into(), "translation".into()),
            ("r".into(), "reciter".into()),
            ("R".into(), "refresh".into()),
            ("pgup/pgdn".into(), "scroll".into()),
            ("esc".into(), "back".into()),
        ],
        Screen::Reader => vec![
            ("j/k".into(), "scroll".into()),
            ("space".into(), "ayah".into()),
            ("a".into(), "play surah".into()),
            ("x".into(), "stop".into()),
            ("t".into(), "mode".into()),
            ("r".into(), "repeat".into()),
            ("esc".into(), "list".into()),
        ],
        Screen::TranslationPicker | Screen::ReciterPicker => vec![
            ("up/down".into(), "navigate".into()),
            ("enter".into(), "select".into()),
            ("esc".into(), "back".into()),
        ],
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars.saturating_sub(1) {
            out.push('…');
            return out;
        }
        out.push(ch);
    }
    out
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
