use std::io::{BufRead, BufReader, Write};

use chrono::{Datelike, Local, NaiveDate};
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginRequest, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use serde::{Deserialize, Serialize};

const DB_KEY: &str = "todo-task-manager";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Status {
    Todo,
    Done,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
enum Priority {
    Low,
    Normal,
    High,
}

impl Priority {
    fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: String,
    title: String,
    notes: String,
    status: Status,
    priority: Priority,
    due_date: Option<String>,
    tags: Vec<String>,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Active,
    Today,
    Overdue,
    Completed,
    Tagged,
}

impl View {
    fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Today => "Due Today",
            Self::Overdue => "Overdue",
            Self::Completed => "Completed",
            Self::Tagged => "Tag Filter",
        }
    }
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
    Notes,
    Due,
    Tags,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    tasks: Vec<Task>,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    tasks: Vec<Task>,
    filtered: Vec<String>,
    cursor: usize,
    view: View,
    search: String,
    tag_filter: String,
    screen: Screen,
    edit_title: String,
    edit_notes: String,
    edit_due: String,
    edit_tags: String,
    default_priority: Priority,
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
            tasks: Vec::new(),
            filtered: Vec::new(),
            cursor: 0,
            view: View::Active,
            search: String::new(),
            tag_filter: String::new(),
            screen: Screen::List,
            edit_title: String::new(),
            edit_notes: String::new(),
            edit_due: String::new(),
            edit_tags: String::new(),
            default_priority: Priority::Normal,
            status: "n new · e edit · Space done · d delete · / search · t tag · 1/2/3 priority"
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
            IpcKey::Char(' ') => {
                if let Some(id) = self.selected_id() {
                    self.toggle_done(&id);
                    self.schedule_save();
                }
                true
            }
            IpcKey::Char('d') => {
                if let Some(id) = self.selected_id() {
                    self.screen = Screen::ConfirmDelete(id);
                }
                true
            }
            IpcKey::Char('/') => {
                self.search.clear();
                self.status = "Search: type query, Backspace edit, Esc clear".into();
                true
            }
            IpcKey::Char('t') => {
                self.view = View::Tagged;
                self.tag_filter.clear();
                self.status = "Tag filter: type tag, Backspace edit, Esc active view".into();
                self.apply_filter();
                true
            }
            IpcKey::Char('1') | IpcKey::Char('2') | IpcKey::Char('3') => {
                if let Some(id) = self.selected_id() {
                    let priority = match key {
                        IpcKey::Char('1') => Priority::Low,
                        IpcKey::Char('3') => Priority::High,
                        _ => Priority::Normal,
                    };
                    if let Some(task) = self.task_mut(&id) {
                        task.priority = priority;
                        task.updated_at = now_string();
                        self.status = format!("Priority set to {}", priority.label());
                        self.schedule_save();
                        self.apply_filter();
                    }
                }
                true
            }
            IpcKey::Char('a') => {
                self.view = View::Active;
                self.apply_filter();
                true
            }
            IpcKey::Char('o') => {
                self.view = View::Overdue;
                self.apply_filter();
                true
            }
            IpcKey::Char('y') => {
                self.view = View::Today;
                self.apply_filter();
                true
            }
            IpcKey::Char('x') => {
                self.view = View::Completed;
                self.apply_filter();
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
                if self.view == View::Tagged && !self.tag_filter.is_empty() {
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
            IpcKey::Char(c) if !c.is_control() => {
                if self.view == View::Tagged {
                    self.tag_filter.push(c);
                } else {
                    self.search.push(c);
                }
                self.apply_filter();
                true
            }
            IpcKey::Esc
                if !self.search.is_empty()
                    || !self.tag_filter.is_empty()
                    || self.view != View::Active =>
            {
                self.search.clear();
                self.tag_filter.clear();
                self.view = View::Active;
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
            IpcKey::Enter => {
                if field == EditField::Notes {
                    self.edit_notes.push('\n');
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
            IpcKey::Char(c) if !c.is_control() => {
                self.edit_buf_mut(field).push(c);
                true
            }
            _ => false,
        }
    }

    fn handle_confirm_key(&mut self, key: IpcKey, id: &str) -> bool {
        match key {
            IpcKey::Char('y') | IpcKey::Char('Y') => {
                self.tasks.retain(|task| task.id != id);
                self.screen = Screen::List;
                self.status = "Task deleted".into();
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
            if let Some(task) = self.tasks.iter().find(|task| task.id == *id) {
                self.edit_title = task.title.clone();
                self.edit_notes = task.notes.clone();
                self.edit_due = task.due_date.clone().unwrap_or_default();
                self.edit_tags = task.tags.join(", ");
            }
        } else {
            self.edit_title.clear();
            self.edit_notes.clear();
            self.edit_due.clear();
            self.edit_tags.clear();
        }
        self.screen = Screen::Edit {
            id,
            field: EditField::Title,
        };
    }

    fn save_edit(&mut self, id: Option<String>) -> Result<(), String> {
        let title = self.edit_title.trim().to_string();
        if title.is_empty() {
            return Err("Title is required".into());
        }
        let due_date = parse_optional_date(&self.edit_due)?;
        let tags = parse_tags(&self.edit_tags);
        let now = now_string();
        if let Some(id) = id {
            let notes = self.edit_notes.clone();
            if let Some(task) = self.task_mut(&id) {
                task.title = title;
                task.notes = notes;
                task.due_date = due_date;
                task.tags = tags;
                task.updated_at = now;
                self.status = "Task updated".into();
            }
        } else {
            let id = format!("task-{}", self.next_id);
            self.next_id += 1;
            self.tasks.push(Task {
                id,
                title,
                notes: self.edit_notes.clone(),
                status: Status::Todo,
                priority: self.default_priority,
                due_date,
                tags,
                created_at: now.clone(),
                updated_at: now,
                completed_at: None,
            });
            self.status = "Task created".into();
        }
        self.screen = Screen::List;
        self.schedule_save();
        self.apply_filter();
        Ok(())
    }

    fn toggle_done(&mut self, id: &str) {
        if let Some(task) = self.task_mut(id) {
            task.updated_at = now_string();
            match task.status {
                Status::Todo => {
                    task.status = Status::Done;
                    task.completed_at = Some(task.updated_at.clone());
                    self.status = "Task completed".into();
                }
                Status::Done => {
                    task.status = Status::Todo;
                    task.completed_at = None;
                    self.status = "Task reopened".into();
                }
            }
        }
        self.apply_filter();
    }

    fn selected_id(&self) -> Option<String> {
        self.filtered.get(self.cursor).cloned()
    }

    fn task_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|task| task.id == id)
    }

    fn selected_task(&self) -> Option<&Task> {
        self.selected_id()
            .and_then(|id| self.tasks.iter().find(|task| task.id == id))
    }

    fn edit_buf_mut(&mut self, field: EditField) -> &mut String {
        match field {
            EditField::Title => &mut self.edit_title,
            EditField::Notes => &mut self.edit_notes,
            EditField::Due => &mut self.edit_due,
            EditField::Tags => &mut self.edit_tags,
        }
    }

    fn apply_filter(&mut self) {
        let today = today();
        let query = self.search.trim().to_lowercase();
        let tag_filter = self.tag_filter.trim().to_lowercase();
        let mut ids: Vec<String> = self
            .tasks
            .iter()
            .filter(|task| match self.view {
                View::Active => task.status == Status::Todo,
                View::Completed => task.status == Status::Done,
                View::Today => {
                    task.status == Status::Todo
                        && task.due_date.as_deref().is_some_and(|d| d == today)
                }
                View::Overdue => {
                    task.status == Status::Todo
                        && task.due_date.as_deref().is_some_and(|d| d < today.as_str())
                }
                View::Tagged => {
                    tag_filter.is_empty()
                        || task
                            .tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&tag_filter))
                }
            })
            .filter(|task| {
                query.is_empty()
                    || task.title.to_lowercase().contains(&query)
                    || task.notes.to_lowercase().contains(&query)
                    || task
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query))
            })
            .map(|task| task.id.clone())
            .collect();
        ids.sort_by_key(|id| sort_key(self.tasks.iter().find(|task| task.id == *id).unwrap()));
        self.filtered = ids;
        let max = self.filtered.len().saturating_sub(1);
        self.cursor = self.cursor.min(max);
    }

    fn serialize(&self) -> String {
        serde_json::to_string(&PersistedState {
            tasks: self.tasks.clone(),
        })
        .unwrap_or_default()
    }

    fn load(&mut self, json: &str) {
        if let Ok(state) = serde_json::from_str::<PersistedState>(json) {
            self.next_id = state
                .tasks
                .iter()
                .filter_map(|task| {
                    task.id
                        .strip_prefix("task-")
                        .and_then(|n| n.parse::<u64>().ok())
                })
                .max()
                .unwrap_or(0)
                + 1;
            self.tasks = state.tasks;
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
        EditField::Title => EditField::Notes,
        EditField::Notes => EditField::Due,
        EditField::Due => EditField::Tags,
        EditField::Tags => EditField::Title,
    }
}
fn prev_field(field: EditField) -> EditField {
    match field {
        EditField::Title => EditField::Tags,
        EditField::Notes => EditField::Title,
        EditField::Due => EditField::Notes,
        EditField::Tags => EditField::Due,
    }
}

fn parse_optional_date(input: &str) -> Result<Option<String>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .map_err(|_| "Due date must be YYYY-MM-DD".to_string())?;
    Ok(Some(trimmed.to_string()))
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

fn sort_key(task: &Task) -> (u8, String, String) {
    let priority = match task.priority {
        Priority::High => 0,
        Priority::Normal => 1,
        Priority::Low => 2,
    };
    (
        priority,
        task.due_date.clone().unwrap_or_else(|| "9999-12-31".into()),
        task.title.to_lowercase(),
    )
}

fn now_string() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}
fn today() -> String {
    let now = Local::now();
    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
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
        title: Some(" Todo Task Manager ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    match &app.screen {
        Screen::List => render_list(app, &mut cmds, &theme, w, h),
        Screen::Edit { field, .. } => render_edit(app, &mut cmds, &theme, w, h, *field),
        Screen::ConfirmDelete(_) => {
            render_list(app, &mut cmds, &theme, w, h);
            let msg = "Delete selected task? y confirm · n/Esc cancel";
            cmds.push(RenderCmd::Dim {
                x: 0,
                y: 0,
                w,
                h,
                bg: theme.background_overlay,
            });
            cmds.push(RenderCmd::Border {
                x: w / 2 - 22,
                y: h / 2 - 2,
                w: 44,
                h: 5,
                fg: theme.error,
                borders: BORDER_ALL,
                bg: Some(theme.background_panel),
                title: Some(" Confirm ".into()),
                title_fg: Some(theme.text),
                title_dash_fg: Some(theme.error),
                border_type: None,
            });
            push_text(&mut cmds, w / 2 - 20, h / 2, msg, theme.text, true);
        }
    }
    cmds
}

fn render_list(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let header = format!(
        "View: {} · Search: {} · Tag: {} · {} task(s)",
        app.view.label(),
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
        .filter_map(|id| app.tasks.iter().find(|task| task.id == *id))
        .map(task_row)
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
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    if let Some(task) = app.selected_task() {
        let detail = format!(
            "{}\nPriority: {}\nDue: {}\nTags: {}\n\n{}",
            task.title,
            task.priority.label(),
            task.due_date.as_deref().unwrap_or("-"),
            task.tags.join(", "),
            task.notes
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

fn render_edit(
    app: &App,
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    w: u16,
    h: u16,
    field: EditField,
) {
    push_text(
        cmds,
        2,
        2,
        "Task editor: Tab field · Enter save · Esc cancel",
        theme.text,
        true,
    );
    let lines = [
        (EditField::Title, "Title", app.edit_title.as_str()),
        (EditField::Notes, "Notes", app.edit_notes.as_str()),
        (EditField::Due, "Due YYYY-MM-DD", app.edit_due.as_str()),
        (
            EditField::Tags,
            "Tags comma-separated",
            app.edit_tags.as_str(),
        ),
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
        y += if line_field == EditField::Notes { 3 } else { 1 };
    }
    push_text(
        cmds,
        2,
        h.saturating_sub(1),
        &app.status,
        theme.text_muted,
        false,
    );
    let _ = w;
}

fn task_row(task: &Task) -> String {
    let done = if task.status == Status::Done {
        "✓"
    } else {
        " "
    };
    let priority = match task.priority {
        Priority::High => "H",
        Priority::Normal => "N",
        Priority::Low => "L",
    };
    format!(
        "[{done}] {priority} {:<10} {}",
        task.due_date.as_deref().unwrap_or("-"),
        task.title
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
fn hints() -> Vec<(String, String)> {
    vec![
        ("a".into(), "active".into()),
        ("y".into(), "today".into()),
        ("o".into(), "overdue".into()),
        ("x".into(), "completed".into()),
        ("n".into(), "new".into()),
        ("e".into(), "edit".into()),
        ("space".into(), "toggle".into()),
        ("d".into(), "delete".into()),
        ("esc".into(), "back".into()),
    ]
}
fn palette_commands() -> Vec<(String, String)> {
    vec![("Productivity".into(), "Open todo task manager".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let request = app.pending_request.take();
    let json = serde_json::json!({ "commands": commands_val, "hints": hints(), "palette_commands": palette_commands(), "request": request, "plugin_message": null, "consumed": consumed });
    if let Ok(json_str) = serde_json::to_string(&json) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{json_str}");
        let _ = out.flush();
    }
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
                log::error!("[todo-task-manager] parse error: {e}: {trimmed}");
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

    fn sample_task(id: &str, title: &str) -> Task {
        Task {
            id: id.into(),
            title: title.into(),
            notes: "notes".into(),
            status: Status::Todo,
            priority: Priority::Normal,
            due_date: None,
            tags: vec!["work".into()],
            created_at: now_string(),
            updated_at: now_string(),
            completed_at: None,
        }
    }

    #[test]
    fn parses_tags_deduped() {
        assert_eq!(parse_tags("Work, home, work"), vec!["home", "work"]);
    }

    #[test]
    fn validates_due_date() {
        assert_eq!(
            parse_optional_date("2026-07-04").unwrap(),
            Some("2026-07-04".into())
        );
        assert!(parse_optional_date("tomorrow").is_err());
    }

    #[test]
    fn toggles_done_and_reopen() {
        let mut app = App::default();
        app.tasks.push(sample_task("a", "Ship"));
        app.apply_filter();
        app.toggle_done("a");
        assert_eq!(app.tasks[0].status, Status::Done);
        app.toggle_done("a");
        assert_eq!(app.tasks[0].status, Status::Todo);
    }

    #[test]
    fn filters_by_search_and_tag() {
        let mut app = App::default();
        app.tasks.push(sample_task("a", "Write docs"));
        app.search = "docs".into();
        app.apply_filter();
        assert_eq!(app.filtered, vec!["a"]);
        app.search.clear();
        app.view = View::Tagged;
        app.tag_filter = "wor".into();
        app.apply_filter();
        assert_eq!(app.filtered, vec!["a"]);
    }

    #[test]
    fn serializes_roundtrip() {
        let mut app = App::default();
        app.tasks.push(sample_task("task-9", "Persist"));
        let json = app.serialize();
        let mut loaded = App::default();
        loaded.load(&json);
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.next_id, 10);
    }
}
