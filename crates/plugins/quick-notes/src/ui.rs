use crate::state::{NotesState, Screen};
use santui_ipc::protocol::{RenderCmd, ThemeData};

pub fn render_ui(state: &NotesState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    match &state.screen {
        Screen::View(idx) => render_view(state, theme, w, h, *idx),
        Screen::Edit(idx) => render_edit(state, theme, w, h, *idx),
        Screen::NewTitle => render_new_title(state, theme, w, h),
        Screen::Rename(_) => render_rename(state, theme, w, h),
        Screen::ConfirmDelete(idx) => render_confirm_delete(state, theme, w, h, *idx),
        Screen::List => render_list(state, theme, w, h),
    }
}

fn render_list(state: &NotesState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let title = format!(" Quick Notes \u{2014} {} notes ", state.notes.len());
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h: 1,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: Some(title),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    // Search bar
    let search_display = format!("> search: {}_", state.search_query);
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: search_display,
        fg: Some(theme.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    // Separator
    let sep = "\u{2500}".repeat(w.saturating_sub(4) as usize);
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: sep,
        fg: Some(theme.border),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let visible_count = (h as usize).saturating_sub(4);
    for i in 0..visible_count {
        let fi = i;
        if let Some(&idx) = state.filtered_indices.get(fi) {
            if let Some(note) = state.notes.get(idx) {
                let marker = if fi == state.list_cursor {
                    "\u{25b6}"
                } else {
                    " "
                };
                let line = format!("{} {}", marker, note.title);
                let fg = if fi == state.list_cursor {
                    Some(theme.accent)
                } else {
                    Some(theme.text)
                };
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: 4 + i as u16,
                    text: line,
                    fg,
                    bg: None,
                    bold: fi == state.list_cursor,
                    modifiers: 0,
                });
            }
        }
    }

    if state.notes.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 5,
            text: "No notes. Press 'n' to create one.".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds
}

fn render_view(
    state: &NotesState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    idx: usize,
) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let note = &state.notes[idx];

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h: 1,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: Some(format!(" {} ", note.title)),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let body_height = h.saturating_sub(3) as usize;
    let lines: Vec<&str> = note.body.lines().skip(state.scroll_offset).collect();
    for (i, line) in lines.iter().take(body_height).enumerate() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 2 + i as u16,
            text: line.to_string(),
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds
}

fn render_edit(
    state: &NotesState,
    theme: &ThemeData,
    w: u16,
    h: u16,
    idx: usize,
) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let note = &state.notes[idx];

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h: 1,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: Some(format!(" Editing: {} ", note.title)),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    let body_height = h.saturating_sub(3) as usize;
    let lines: Vec<&str> = state.edit_buf.lines().skip(state.scroll_offset).collect();
    for (i, line) in lines.iter().take(body_height).enumerate() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 2 + i as u16,
            text: line.to_string(),
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds
}

fn render_new_title(state: &NotesState, theme: &ThemeData, w: u16, _h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w,
        h: _h,
        bg: theme.background_overlay,
    });

    let popup_w = 40u16;
    let popup_x = (w.saturating_sub(popup_w)) / 2;

    cmds.push(RenderCmd::Border {
        x: popup_x,
        y: 3,
        w: popup_w,
        h: 5,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: Some(" New Note ".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: 4,
        text: format!("Title: {}_", state.title_buf),
        fg: Some(theme.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: 6,
        text: "      enter create   esc cancel".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

fn render_rename(state: &NotesState, theme: &ThemeData, w: u16, _h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w,
        h: _h,
        bg: theme.background_overlay,
    });

    let popup_w = 40u16;
    let popup_x = (w.saturating_sub(popup_w)) / 2;

    cmds.push(RenderCmd::Border {
        x: popup_x,
        y: 3,
        w: popup_w,
        h: 5,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: Some(" Rename Note ".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: 4,
        text: format!("Title: {}_", state.title_buf),
        fg: Some(theme.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: 6,
        text: "      enter save   esc cancel".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

fn render_confirm_delete(
    state: &NotesState,
    theme: &ThemeData,
    w: u16,
    _h: u16,
    idx: usize,
) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Dim {
        x: 0,
        y: 0,
        w,
        h: _h,
        bg: theme.background_overlay,
    });

    let popup_w = 50u16;
    let popup_x = (w.saturating_sub(popup_w)) / 2;
    let note_title = state
        .notes
        .get(idx)
        .map(|n| n.title.as_str())
        .unwrap_or("?");

    cmds.push(RenderCmd::Border {
        x: popup_x,
        y: 3,
        w: popup_w,
        h: 6,
        fg: theme.border,
        borders: 15,
        bg: Some(theme.background_panel),
        title: Some(" Delete Note? ".into()),
        title_fg: Some(theme.error),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: 4,
        text: format!("Delete \"{}\"?", note_title),
        fg: Some(theme.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: 5,
        text: "This cannot be undone.".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: popup_x + 2,
        y: 7,
        text: "           y confirm    n / esc cancel".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::NotesState;

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
    fn renders_note_list() {
        let state = NotesState::default();
        let commands = render_list(&state, &default_theme(), 80, 24);
        assert!(!commands.is_empty());
    }

    #[test]
    fn renders_empty_state_message() {
        let state = NotesState::default();
        let commands = render_list(&state, &default_theme(), 80, 24);
        let has_empty = commands
            .iter()
            .any(|cmd| matches!(cmd, RenderCmd::Text { text, .. } if text.contains("No notes")));
        assert!(has_empty);
    }

    #[test]
    fn renders_search_bar() {
        let mut state = NotesState::default();
        state.search_query = "test".into();
        let commands = render_list(&state, &default_theme(), 80, 24);
        let has_search = commands
            .iter()
            .any(|cmd| matches!(cmd, RenderCmd::Text { text, .. } if text.contains("test")));
        assert!(has_search);
    }

    #[test]
    fn renders_view_screen() {
        let mut state = NotesState::default();
        state.notes.push(crate::state::Note {
            id: 1,
            title: "Hello".into(),
            body: "world".into(),
            updated_at: 0,
        });
        let commands = render_view(&state, &default_theme(), 80, 24, 0);
        assert!(!commands.is_empty());
    }

    #[test]
    fn renders_edit_screen() {
        let mut state = NotesState::default();
        state.notes.push(crate::state::Note {
            id: 1,
            title: "Hello".into(),
            body: "world".into(),
            updated_at: 0,
        });
        state.edit_buf = "editing".into();
        let commands = render_edit(&state, &default_theme(), 80, 24, 0);
        assert!(!commands.is_empty());
    }

    #[test]
    fn renders_new_title_overlay() {
        let mut state = NotesState::default();
        state.title_buf = "New".into();
        let commands = render_new_title(&state, &default_theme(), 80, 24);
        assert!(!commands.is_empty());
    }

    #[test]
    fn renders_confirm_delete_overlay() {
        let mut state = NotesState::default();
        state.notes.push(crate::state::Note {
            id: 1,
            title: "Hello".into(),
            body: "world".into(),
            updated_at: 0,
        });
        let commands = render_confirm_delete(&state, &default_theme(), 80, 24, 0);
        assert!(!commands.is_empty());
    }
}
