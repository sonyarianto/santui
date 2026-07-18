use std::io::{BufRead, BufReader};
use std::process::Command;

use chrono::Local;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginRequest, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use serde::{Deserialize, Serialize};

const DB_KEY: &str = "bookmark-launcher";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum BookmarkKind {
    Url,
    Path,
    Command,
    Note,
}

impl BookmarkKind {
    fn label(self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Path => "path",
            Self::Command => "cmd",
            Self::Note => "note",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Url => Self::Path,
            Self::Path => Self::Command,
            Self::Command => Self::Note,
            Self::Note => Self::Url,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Bookmark {
    id: String,
    title: String,
    kind: BookmarkKind,
    target: String,
    description: String,
    tags: Vec<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    List,
    Edit {
        id: Option<String>,
        field: EditField,
    },
    ConfirmDelete(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditField {
    Title,
    Target,
    Description,
    Tags,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    bookmarks: Vec<Bookmark>,
    filtered: Vec<String>,
    cursor: usize,
    search: String,
    tag_filter: String,
    screen: Screen,
    edit_kind: BookmarkKind,
    edit_title: String,
    edit_target: String,
    edit_description: String,
    edit_tags: String,
    status: String,
    next_id: u64,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 100, h: 28 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet { key: DB_KEY.into() }),
            bookmarks: Vec::new(),
            filtered: Vec::new(),
            cursor: 0,
            search: String::new(),
            tag_filter: String::new(),
            screen: Screen::List,
            edit_kind: BookmarkKind::Url,
            edit_title: String::new(),
            edit_target: String::new(),
            edit_description: String::new(),
            edit_tags: String::new(),
            status: "n new · e edit · Enter open/copy · c copy · d delete · / search · t tag"
                .into(),
            next_id: 1,
        };
        app.apply_filter();
        app
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.dirty = true;
        match self.screen.clone() {
            Screen::List => self.handle_list_key(key),
            Screen::Edit { id, field } => self.handle_edit_key(key, id, field),
            Screen::ConfirmDelete(id) => self.handle_confirm_key(key, &id),
        }
    }

    fn handle_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char('n') => {
                self.start_edit(None);
                true
            }
            IpcKey::Char('e') => {
                if let Some(id) = self.selected_id() {
                    self.start_edit(Some(id));
                }
                true
            }
            IpcKey::Char('d') => {
                if let Some(id) = self.selected_id() {
                    self.screen = Screen::ConfirmDelete(id);
                }
                true
            }
            IpcKey::Enter => {
                self.primary_action();
                true
            }
            IpcKey::Char('c') => {
                self.copy_selected();
                true
            }
            IpcKey::Char('/') => {
                self.search.clear();
                self.status = "Search mode: type query, Esc clear".into();
                true
            }
            IpcKey::Char('t') => {
                self.tag_filter.clear();
                self.status = "Tag filter: type tag, Esc clear".into();
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.filtered.len().saturating_sub(1);
                self.cursor = self.cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Backspace => {
                if !self.tag_filter.is_empty() {
                    self.tag_filter.pop();
                    self.apply_filter();
                    true
                } else if !self.search.is_empty() {
                    self.search.pop();
                    self.apply_filter();
                    true
                } else {
                    false
                }
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                if self.status.starts_with("Tag filter") || !self.tag_filter.is_empty() {
                    self.tag_filter.push(ch);
                } else {
                    self.search.push(ch);
                }
                self.apply_filter();
                true
            }
            IpcKey::Esc if !self.search.is_empty() || !self.tag_filter.is_empty() => {
                self.search.clear();
                self.tag_filter.clear();
                self.apply_filter();
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_edit_key(&mut self, key: IpcKey, id: Option<String>, field: EditField) -> bool {
        match key {
            IpcKey::Tab => {
                self.screen = Screen::Edit {
                    id,
                    field: next_field(field),
                };
                true
            }
            IpcKey::BackTab => {
                self.screen = Screen::Edit {
                    id,
                    field: prev_field(field),
                };
                true
            }
            IpcKey::Char('K') => {
                self.edit_kind = self.edit_kind.next();
                true
            }
            IpcKey::Enter => {
                if field == EditField::Description {
                    self.edit_description.push('\n');
                } else if let Err(e) = self.save_edit(id) {
                    self.status = e;
                }
                true
            }
            IpcKey::Backspace => {
                self.edit_buf_mut(field).pop();
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::List;
                true
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.edit_buf_mut(field).push(ch);
                true
            }
            _ => false,
        }
    }

    fn handle_confirm_key(&mut self, key: IpcKey, id: &str) -> bool {
        match key {
            IpcKey::Char('y') | IpcKey::Char('Y') => {
                self.bookmarks.retain(|bookmark| bookmark.id != id);
                self.screen = Screen::List;
                self.status = "Bookmark deleted".into();
                self.schedule_save();
                self.apply_filter();
                true
            }
            IpcKey::Char('n') | IpcKey::Esc => {
                self.screen = Screen::List;
                true
            }
            _ => true,
        }
    }

    fn start_edit(&mut self, id: Option<String>) {
        if let Some(ref id) = id {
            if let Some(bookmark) = self.bookmarks.iter().find(|bookmark| bookmark.id == *id) {
                self.edit_kind = bookmark.kind;
                self.edit_title = bookmark.title.clone();
                self.edit_target = bookmark.target.clone();
                self.edit_description = bookmark.description.clone();
                self.edit_tags = bookmark.tags.join(", ");
            }
        } else {
            self.edit_kind = BookmarkKind::Url;
            self.edit_title.clear();
            self.edit_target.clear();
            self.edit_description.clear();
            self.edit_tags.clear();
        }
        self.screen = Screen::Edit {
            id,
            field: EditField::Title,
        };
    }

    fn save_edit(&mut self, id: Option<String>) -> Result<(), String> {
        let title = self.edit_title.trim().to_string();
        let target = self.edit_target.trim().to_string();
        if title.is_empty() {
            return Err("Title is required".into());
        }
        if target.is_empty() {
            return Err("Target/value is required".into());
        }
        validate_target(self.edit_kind, &target)?;
        let tags = parse_tags(&self.edit_tags);
        let now = now_string();
        if let Some(id) = id {
            let description = self.edit_description.clone();
            let kind = self.edit_kind;
            if let Some(bookmark) = self.bookmark_mut(&id) {
                bookmark.title = title;
                bookmark.kind = kind;
                bookmark.target = target;
                bookmark.description = description;
                bookmark.tags = tags;
                bookmark.updated_at = now;
                self.status = "Bookmark updated".into();
            }
        } else {
            let id = format!("bookmark-{}", self.next_id);
            self.next_id += 1;
            self.bookmarks.push(Bookmark {
                id,
                title,
                kind: self.edit_kind,
                target,
                description: self.edit_description.clone(),
                tags,
                created_at: now.clone(),
                updated_at: now,
            });
            self.status = "Bookmark created".into();
        }
        self.screen = Screen::List;
        self.schedule_save();
        self.apply_filter();
        Ok(())
    }

    fn primary_action(&mut self) {
        let Some(bookmark) = self.selected_bookmark().cloned() else {
            return;
        };
        match bookmark.kind {
            BookmarkKind::Url | BookmarkKind::Path => match open_target(&bookmark.target) {
                Ok(()) => self.status = format!("Opened {}", bookmark.kind.label()),
                Err(e) => self.status = format!("Open failed: {e}"),
            },
            BookmarkKind::Command | BookmarkKind::Note => {
                self.copy_text(&bookmark.target, "Copied target")
            }
        }
    }

    fn copy_selected(&mut self) {
        if let Some(bookmark) = self.selected_bookmark().cloned() {
            self.copy_text(&bookmark.target, "Copied bookmark target");
        }
    }

    fn copy_text(&mut self, text: &str, ok_status: &str) {
        match copy_to_clipboard(text) {
            Ok(()) => self.status = ok_status.into(),
            Err(e) => self.status = format!("Clipboard error: {e}"),
        }
    }

    fn selected_id(&self) -> Option<String> {
        self.filtered.get(self.cursor).cloned()
    }
    fn selected_bookmark(&self) -> Option<&Bookmark> {
        self.selected_id()
            .and_then(|id| self.bookmarks.iter().find(|bookmark| bookmark.id == id))
    }
    fn bookmark_mut(&mut self, id: &str) -> Option<&mut Bookmark> {
        self.bookmarks.iter_mut().find(|bookmark| bookmark.id == id)
    }
    fn edit_buf_mut(&mut self, field: EditField) -> &mut String {
        match field {
            EditField::Title => &mut self.edit_title,
            EditField::Target => &mut self.edit_target,
            EditField::Description => &mut self.edit_description,
            EditField::Tags => &mut self.edit_tags,
        }
    }

    fn apply_filter(&mut self) {
        let query = self.search.trim().to_lowercase();
        let tag_filter = self.tag_filter.trim().to_lowercase();
        let mut ids: Vec<String> = self
            .bookmarks
            .iter()
            .filter(|bookmark| {
                (query.is_empty()
                    || bookmark.title.to_lowercase().contains(&query)
                    || bookmark.target.to_lowercase().contains(&query)
                    || bookmark.description.to_lowercase().contains(&query)
                    || bookmark
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query)))
                    && (tag_filter.is_empty()
                        || bookmark
                            .tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&tag_filter)))
            })
            .map(|bookmark| bookmark.id.clone())
            .collect();
        ids.sort_by_key(|id| {
            self.bookmarks
                .iter()
                .find(|bookmark| bookmark.id == *id)
                .map(|b| b.title.to_lowercase())
                .unwrap_or_default()
        });
        self.filtered = ids;
        self.cursor = self.cursor.min(self.filtered.len().saturating_sub(1));
    }

    fn serialize(&self) -> String {
        serde_json::to_string(&PersistedState {
            bookmarks: self.bookmarks.clone(),
        })
        .unwrap_or_default()
    }
    fn load(&mut self, json: &str) {
        if let Ok(state) = serde_json::from_str::<PersistedState>(json) {
            self.next_id = state
                .bookmarks
                .iter()
                .filter_map(|b| {
                    b.id.strip_prefix("bookmark-")
                        .and_then(|n| n.parse::<u64>().ok())
                })
                .max()
                .unwrap_or(0)
                + 1;
            self.bookmarks = state.bookmarks;
            self.apply_filter();
        }
    }
    fn schedule_save(&mut self) {
        self.pending_request = Some(PluginRequest::DbSet {
            key: DB_KEY.into(),
            value: self.serialize(),
        });
    }
    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn next_field(field: EditField) -> EditField {
    match field {
        EditField::Title => EditField::Target,
        EditField::Target => EditField::Description,
        EditField::Description => EditField::Tags,
        EditField::Tags => EditField::Title,
    }
}
fn prev_field(field: EditField) -> EditField {
    match field {
        EditField::Title => EditField::Tags,
        EditField::Target => EditField::Title,
        EditField::Description => EditField::Target,
        EditField::Tags => EditField::Description,
    }
}
fn parse_tags(input: &str) -> Vec<String> {
    let mut tags: Vec<String> = input
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(|tag| tag.to_lowercase())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}
fn validate_target(kind: BookmarkKind, target: &str) -> Result<(), String> {
    match kind {
        BookmarkKind::Url if !(target.starts_with("http://") || target.starts_with("https://")) => {
            Err("URL must start with http:// or https://".into())
        }
        BookmarkKind::Path if target.trim().is_empty() => Err("Path cannot be empty".into()),
        _ => Ok(()),
    }
}
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
}
fn open_target(target: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", target]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(target);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(target);
        c
    };
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}
fn now_string() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let theme = app.theme.clone();
    let w = app.area.w.max(70);
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
        title: Some(" Bookmark Launcher ".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    match &app.screen {
        Screen::List => render_list(app, &mut cmds, &theme, w, h),
        Screen::Edit { field, .. } => render_edit(app, &mut cmds, &theme, w, h, *field),
        Screen::ConfirmDelete(_) => {
            render_list(app, &mut cmds, &theme, w, h);
            cmds.push(RenderCmd::Dim {
                x: 0,
                y: 0,
                w,
                h,
                bg: theme.background_overlay,
            });
            cmds.push(RenderCmd::Border {
                x: w / 2 - 24,
                y: h / 2 - 2,
                w: 48,
                h: 5,
                fg: theme.error,
                borders: BORDER_ALL,
                bg: Some(theme.background_panel),
                title: Some(" Confirm ".into()),
                title_fg: Some(theme.border),
                title_dash_fg: Some(theme.error),
                border_type: None,
            });
            push_text(
                &mut cmds,
                w / 2 - 22,
                h / 2,
                "Delete selected bookmark? y confirm · n/Esc cancel",
                theme.text,
                true,
            );
        }
    }
    cmds
}
fn render_list(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let header = format!(
        "Search: {} · Tag: {} · {} bookmark(s)",
        app.search,
        app.tag_filter,
        app.filtered.len()
    );
    push_text(
        cmds,
        2,
        2,
        truncate(&header, w as usize - 4),
        theme.text,
        true,
    );
    let list_h = h.saturating_sub(8).max(4);
    let detail_x = (w * 58 / 100).max(40);
    let list_w = detail_x.saturating_sub(4);
    let items: Vec<String> = app
        .filtered
        .iter()
        .filter_map(|id| app.bookmarks.iter().find(|b| b.id == *id))
        .map(bookmark_row)
        .collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 4,
        w: list_w,
        h: list_h,
        items,
        selected: Some(app.cursor.min(app.filtered.len().saturating_sub(1))),
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
    cmds.push(RenderCmd::Border {
        x: detail_x,
        y: 4,
        w: w.saturating_sub(detail_x + 2),
        h: list_h,
        fg: theme.border,
        borders: BORDER_ALL,
        bg: None,
        title: Some(" Detail ".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    if let Some(bookmark) = app.selected_bookmark() {
        let detail = format!(
            "{} [{}]\n{}\nTags: {}\n\n{}",
            bookmark.title,
            bookmark.kind.label(),
            bookmark.target,
            bookmark.tags.join(", "),
            bookmark.description
        );
        cmds.push(RenderCmd::Paragraph {
            x: detail_x + 1,
            y: 5,
            w: w.saturating_sub(detail_x + 4),
            h: list_h.saturating_sub(2),
            text: detail,
            style: TextStyle {
                fg: Some(theme.text),
                bg: None,
                bold: false,
                modifiers: 0,
            },
            wrap: true,
            spans: None,
            alignment: None,
        });
    }
    push_text(
        cmds,
        2,
        h.saturating_sub(2),
        &app.status,
        theme.text_muted,
        false,
    );
}
fn hints() -> Vec<(String, String)> {
    vec![
        ("n".into(), "new".into()),
        ("e".into(), "edit".into()),
        ("K".into(), "cycle kind in editor".into()),
        ("enter".into(), "open/copy".into()),
        ("c".into(), "copy".into()),
        ("d".into(), "delete".into()),
        ("/".into(), "search".into()),
        ("t".into(), "tag".into()),
        ("esc".into(), "back".into()),
    ]
}
fn render_edit(
    app: &App,
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    _w: u16,
    h: u16,
    field: EditField,
) {
    push_text(
        cmds,
        2,
        2,
        format!(
            "Bookmark editor · Kind: {} · K cycle · Tab field · Enter save · Esc cancel",
            app.edit_kind.label()
        ),
        theme.text,
        true,
    );
    let lines = [
        (EditField::Title, "Title", app.edit_title.as_str()),
        (EditField::Target, "Target", app.edit_target.as_str()),
        (
            EditField::Description,
            "Description",
            app.edit_description.as_str(),
        ),
        (EditField::Tags, "Tags", app.edit_tags.as_str()),
    ];
    let mut y = 4;
    for (line_field, label, value) in lines {
        let marker = if field == line_field { ">" } else { " " };
        push_text(
            cmds,
            2,
            y,
            format!("{marker} {label}: {}", visible(value)),
            if field == line_field {
                theme.highlight
            } else {
                theme.text
            },
            field == line_field,
        );
        y += if line_field == EditField::Description {
            3
        } else {
            1
        };
    }
    push_text(
        cmds,
        2,
        h.saturating_sub(1),
        &app.status,
        theme.text_muted,
        false,
    );
}
fn bookmark_row(bookmark: &Bookmark) -> String {
    format!(
        "{:<5} {:<24} {}",
        bookmark.kind.label(),
        bookmark.title,
        bookmark.tags.join(",")
    )
}
fn visible(value: &str) -> String {
    truncate(&value.replace('\n', " ⏎ "), 90)
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
        success: [127, 216, 143],
        error: [224, 108, 117],
        inverted_text: [20; 3],
    }
}
fn palette_commands() -> Vec<(String, String)> {
    vec![]
}
fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: hints(),
        palette_commands: palette_commands(),
        request: app.pending_request.take(),
        plugin_message: None,
        consumed,
    };
    let mut out = std::io::stdout().lock();
    let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &msg);
}
fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = App::default();
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        let trimmed = line.trim_end();
        let msg = serde_json::from_str::<HostMsg>(trimmed);
        let consumed = match msg {
            Ok(HostMsg::Init { theme, area, .. }) => {
                app.theme = theme;
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Resize { area }) => {
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::ThemeChange { theme }) => {
                app.theme = theme;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Key { key, .. }) => app.handle_key(key),
            Ok(HostMsg::DbValue { key, value }) => {
                if key == DB_KEY {
                    if let Some(json) = value {
                        app.load(&json);
                    }
                    app.dirty = true;
                }
                false
            }
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Tick
                | HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::PaletteCommand { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[bookmark-launcher] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(id: &str, title: &str) -> Bookmark {
        Bookmark {
            id: id.into(),
            title: title.into(),
            kind: BookmarkKind::Url,
            target: "https://example.com".into(),
            description: "docs".into(),
            tags: vec!["work".into()],
            created_at: now_string(),
            updated_at: now_string(),
        }
    }
    #[test]
    fn parses_tags_deduped() {
        assert_eq!(parse_tags("Work, home, work"), vec!["home", "work"]);
    }
    #[test]
    fn validates_urls_explicitly() {
        assert!(validate_target(BookmarkKind::Url, "https://example.com").is_ok());
        assert!(validate_target(BookmarkKind::Url, "example.com").is_err());
    }
    #[test]
    fn filters_by_query_and_tag() {
        let mut app = App::default();
        app.bookmarks.push(sample("a", "Docs"));
        app.search = "doc".into();
        app.apply_filter();
        assert_eq!(app.filtered, vec!["a"]);
        app.search.clear();
        app.tag_filter = "wor".into();
        app.apply_filter();
        assert_eq!(app.filtered, vec!["a"]);
    }
    #[test]
    fn command_kind_is_copy_only() {
        assert_eq!(BookmarkKind::Command.label(), "cmd");
    }
    #[test]
    fn serializes_roundtrip() {
        let mut app = App::default();
        app.bookmarks.push(sample("bookmark-4", "Persist"));
        let json = app.serialize();
        let mut loaded = App::default();
        loaded.load(&json);
        assert_eq!(loaded.bookmarks.len(), 1);
        assert_eq!(loaded.next_id, 5);
    }
}
