use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginRequest, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use serde::{Deserialize, Serialize};

const DB_KEY: &str = "calendar-agenda";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CalendarSource {
    name: String,
    path: String,
}

#[derive(Debug, Clone)]
struct Event {
    id: String,
    calendar: String,
    title: String,
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
    all_day: bool,
    location: String,
    description: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    sources: Vec<CalendarSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    Agenda,
    AddSource { field: SourceField },
    ConfirmRemove(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceField {
    Name,
    Path,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    sources: Vec<CalendarSource>,
    events: Vec<Event>,
    filtered: Vec<usize>,
    cursor: usize,
    search: String,
    screen: Screen,
    source_name: String,
    source_path: String,
    status: String,
    warnings: Vec<String>,
    last_refreshed: String,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 100, h: 28 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet { key: DB_KEY.into() }),
            sources: Vec::new(),
            events: Vec::new(),
            filtered: Vec::new(),
            cursor: 0,
            search: String::new(),
            screen: Screen::Agenda,
            source_name: String::new(),
            source_path: String::new(),
            status: "a add source · r refresh · / search · t today · d remove source".into(),
            warnings: Vec::new(),
            last_refreshed: "never".into(),
        };
        app.apply_filter();
        app
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.dirty = true;
        match self.screen.clone() {
            Screen::Agenda => self.handle_agenda_key(key),
            Screen::AddSource { field } => self.handle_add_key(key, field),
            Screen::ConfirmRemove(idx) => self.handle_confirm_key(key, idx),
        }
    }

    fn handle_agenda_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char('a') => {
                self.source_name.clear();
                self.source_path.clear();
                self.screen = Screen::AddSource {
                    field: SourceField::Name,
                };
                true
            }
            IpcKey::Char('r') => {
                self.refresh();
                true
            }
            IpcKey::Char('d') => {
                if !self.sources.is_empty() {
                    self.screen = Screen::ConfirmRemove(0);
                }
                true
            }
            IpcKey::Char('/') => {
                self.search.clear();
                self.status = "Search: type query, Backspace edit, Esc clear".into();
                true
            }
            IpcKey::Char('t') => {
                self.jump_today();
                true
            }
            IpcKey::Char('n') => {
                self.jump_group(1);
                true
            }
            IpcKey::Char('p') => {
                self.jump_group(-1);
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
                if !self.search.is_empty() {
                    self.search.pop();
                    self.apply_filter();
                    true
                } else {
                    false
                }
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.search.push(ch);
                self.apply_filter();
                true
            }
            IpcKey::Esc if !self.search.is_empty() => {
                self.search.clear();
                self.apply_filter();
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_add_key(&mut self, key: IpcKey, field: SourceField) -> bool {
        match key {
            IpcKey::Tab | IpcKey::BackTab => {
                self.screen = Screen::AddSource {
                    field: if field == SourceField::Name {
                        SourceField::Path
                    } else {
                        SourceField::Name
                    },
                };
                true
            }
            IpcKey::Enter => {
                if let Err(e) = self.save_source() {
                    self.status = e;
                }
                true
            }
            IpcKey::Backspace => {
                self.source_buf_mut(field).pop();
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::Agenda;
                true
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.source_buf_mut(field).push(ch);
                true
            }
            _ => false,
        }
    }

    fn handle_confirm_key(&mut self, key: IpcKey, idx: usize) -> bool {
        match key {
            IpcKey::Char('y') | IpcKey::Char('Y') => {
                if idx < self.sources.len() {
                    self.sources.remove(idx);
                }
                self.schedule_save();
                self.refresh();
                self.screen = Screen::Agenda;
                true
            }
            IpcKey::Char('n') | IpcKey::Esc => {
                self.screen = Screen::Agenda;
                true
            }
            _ => true,
        }
    }

    fn source_buf_mut(&mut self, field: SourceField) -> &mut String {
        match field {
            SourceField::Name => &mut self.source_name,
            SourceField::Path => &mut self.source_path,
        }
    }

    fn save_source(&mut self) -> Result<(), String> {
        let name = self.source_name.trim().to_string();
        let path = self.source_path.trim().to_string();
        if name.is_empty() {
            return Err("Calendar name is required".into());
        }
        if path.is_empty() {
            return Err("Calendar path is required".into());
        }
        if !Path::new(&path).exists() {
            return Err("Path does not exist".into());
        }
        self.sources.push(CalendarSource { name, path });
        self.screen = Screen::Agenda;
        self.schedule_save();
        self.refresh();
        Ok(())
    }

    fn refresh(&mut self) {
        self.events.clear();
        self.warnings.clear();
        for source in &self.sources {
            match load_source(source) {
                Ok(mut events) => self.events.append(&mut events),
                Err(e) => self.warnings.push(format!("{}: {e}", source.name)),
            }
        }
        self.events.sort_by_key(event_sort_key);
        self.last_refreshed = Local::now().format("%Y-%m-%d %H:%M").to_string();
        self.status = format!(
            "Refreshed {} source(s), {} event(s)",
            self.sources.len(),
            self.events.len()
        );
        self.apply_filter();
    }

    fn apply_filter(&mut self) {
        let query = self.search.trim().to_lowercase();
        self.filtered = self
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                query.is_empty()
                    || event.title.to_lowercase().contains(&query)
                    || event.location.to_lowercase().contains(&query)
                    || event.description.to_lowercase().contains(&query)
                    || event.calendar.to_lowercase().contains(&query)
            })
            .map(|(idx, _)| idx)
            .collect();
        self.cursor = self.cursor.min(self.filtered.len().saturating_sub(1));
    }

    fn jump_today(&mut self) {
        let today = Local::now().date_naive();
        if let Some(pos) = self
            .filtered
            .iter()
            .position(|idx| event_date(&self.events[*idx]) >= Some(today))
        {
            self.cursor = pos;
        }
    }

    fn jump_group(&mut self, direction: i32) {
        if self.filtered.is_empty() {
            return;
        }
        let current_date = event_date(&self.events[self.filtered[self.cursor]]);
        if direction > 0 {
            if let Some(pos) = self
                .filtered
                .iter()
                .enumerate()
                .skip(self.cursor + 1)
                .find(|(_, idx)| event_date(&self.events[**idx]) != current_date)
                .map(|(pos, _)| pos)
            {
                self.cursor = pos;
            }
        } else if let Some(pos) = self
            .filtered
            .iter()
            .enumerate()
            .take(self.cursor)
            .rev()
            .find(|(_, idx)| event_date(&self.events[**idx]) != current_date)
            .map(|(pos, _)| pos)
        {
            self.cursor = pos;
        }
    }

    fn selected_event(&self) -> Option<&Event> {
        self.filtered.get(self.cursor).map(|idx| &self.events[*idx])
    }
    fn serialize(&self) -> String {
        serde_json::to_string(&PersistedState {
            sources: self.sources.clone(),
        })
        .unwrap_or_default()
    }
    fn load(&mut self, json: &str) {
        if let Ok(state) = serde_json::from_str::<PersistedState>(json) {
            self.sources = state.sources;
            self.refresh();
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

fn load_source(source: &CalendarSource) -> Result<Vec<Event>, String> {
    let path = PathBuf::from(&source.path);
    let mut events = Vec::new();
    if path.is_dir() {
        for entry in fs::read_dir(&path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("ics"))
            {
                let text = fs::read_to_string(&p).map_err(|e| e.to_string())?;
                events.extend(parse_ics(&text, &source.name));
            }
        }
    } else {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        events.extend(parse_ics(&text, &source.name));
    }
    Ok(events)
}

fn parse_ics(text: &str, calendar: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut in_event = false;
    let mut current: Vec<String> = Vec::new();
    for line in unfold_ics(text) {
        match line.as_str() {
            "BEGIN:VEVENT" => {
                in_event = true;
                current.clear();
            }
            "END:VEVENT" => {
                if in_event {
                    if let Some(event) = parse_event(&current, calendar, events.len()) {
                        events.push(event);
                    }
                }
                in_event = false;
            }
            _ if in_event => current.push(line),
            _ => {}
        }
    }
    events
}

fn unfold_ics(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = out.last_mut() {
                last.push_str(line.trim_start());
            }
        } else {
            out.push(line.to_string());
        }
    }
    out
}

fn parse_event(lines: &[String], calendar: &str, idx: usize) -> Option<Event> {
    let mut title = String::new();
    let mut start = None;
    let mut end = None;
    let mut all_day = false;
    let mut location = String::new();
    let mut description = String::new();
    let mut uid = format!("{calendar}-{idx}");
    for line in lines {
        let (key, value) = line.split_once(':')?;
        let prop = key.split(';').next().unwrap_or(key);
        match prop {
            "UID" => uid = unescape(value),
            "SUMMARY" => title = unescape(value),
            "LOCATION" => location = unescape(value),
            "DESCRIPTION" => description = unescape(value),
            "DTSTART" => {
                let parsed = parse_ics_datetime(value);
                all_day |= parsed.1;
                start = parsed.0;
            }
            "DTEND" => {
                let parsed = parse_ics_datetime(value);
                all_day |= parsed.1;
                end = parsed.0;
            }
            _ => {}
        }
    }
    if title.is_empty() {
        title = "(untitled)".into();
    }
    Some(Event {
        id: uid,
        calendar: calendar.into(),
        title,
        start,
        end,
        all_day,
        location,
        description,
    })
}

fn parse_ics_datetime(value: &str) -> (Option<NaiveDateTime>, bool) {
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y%m%d") {
        return (Some(date.and_time(NaiveTime::MIN)), true);
    }
    let trimmed = value.trim_end_matches('Z');
    if let Ok(dt) = NaiveDateTime::parse_from_str(trimmed, "%Y%m%dT%H%M%S") {
        return (Some(dt), false);
    }
    (None, false)
}
fn unescape(value: &str) -> String {
    value
        .replace("\\n", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}
fn event_date(event: &Event) -> Option<NaiveDate> {
    event.start.map(|dt| dt.date())
}
fn event_sort_key(event: &Event) -> (NaiveDate, NaiveTime, String) {
    let dt = event.start.unwrap_or_else(|| {
        NaiveDate::from_ymd_opt(9999, 12, 31)
            .unwrap()
            .and_time(NaiveTime::MIN)
    });
    (dt.date(), dt.time(), event.title.to_lowercase())
}
fn event_time(event: &Event) -> String {
    if event.all_day {
        return "all day".into();
    }
    match (event.start, event.end) {
        (Some(s), Some(e)) => format!("{}-{}", s.format("%H:%M"), e.format("%H:%M")),
        (Some(s), None) => s.format("%H:%M").to_string(),
        _ => "time unknown".into(),
    }
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
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
        title: Some(" Calendar Agenda ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });
    match &app.screen {
        Screen::Agenda => render_agenda(app, &mut cmds, &theme, w, h),
        Screen::AddSource { field } => render_add(app, &mut cmds, &theme, h, *field),
        Screen::ConfirmRemove(_) => {
            render_agenda(app, &mut cmds, &theme, w, h);
            cmds.push(RenderCmd::Dim {
                x: 0,
                y: 0,
                w,
                h,
                bg: theme.background_overlay,
            });
            push_text(
                &mut cmds,
                w / 2 - 20,
                h / 2,
                "Remove first source? y confirm · n/Esc cancel",
                theme.error,
                true,
            );
        }
    }
    cmds
}
fn render_agenda(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let header = format!(
        "Today: {} · Sources: {} · Last refresh: {} · Search: {}",
        Local::now().date_naive(),
        app.sources.len(),
        app.last_refreshed,
        app.search
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
    let detail_x = (w * 58 / 100).max(42);
    let items: Vec<String> = app
        .filtered
        .iter()
        .map(|idx| event_row(&app.events[*idx]))
        .collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 4,
        w: detail_x.saturating_sub(4),
        h: list_h,
        items,
        selected: Some(app.cursor.min(app.filtered.len().saturating_sub(1))),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
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
    });
    if let Some(event) = app.selected_event() {
        let detail = format!(
            "{}\n{} · {}\nLocation: {}\nCalendar: {}\nID: {}\n\n{}",
            event.title,
            event_date(event)
                .map(|d| d.to_string())
                .unwrap_or_else(|| "date unknown".into()),
            event_time(event),
            event.location,
            event.calendar,
            event.id,
            event.description
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
            },
            wrap: true,
        });
    }
    if let Some(warn) = app.warnings.first() {
        push_text(
            cmds,
            2,
            h.saturating_sub(3),
            truncate(warn, w as usize - 4),
            theme.error,
            false,
        );
    }
    push_text(
        cmds,
        2,
        h.saturating_sub(2),
        &app.status,
        theme.text_muted,
        false,
    );
    push_text(
        cmds,
        2,
        h.saturating_sub(1),
        "a add · r refresh · / search · t today · n/p day group · d remove first source · Esc",
        theme.text_muted,
        false,
    );
}
fn render_add(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, h: u16, field: SourceField) {
    push_text(
        cmds,
        2,
        2,
        "Add local .ics file or directory source · Tab field · Enter save · Esc cancel",
        theme.text,
        true,
    );
    push_text(
        cmds,
        2,
        4,
        format!(
            "{} Name: {}",
            if field == SourceField::Name { ">" } else { " " },
            app.source_name
        ),
        theme.text,
        field == SourceField::Name,
    );
    push_text(
        cmds,
        2,
        5,
        format!(
            "{} Path: {}",
            if field == SourceField::Path { ">" } else { " " },
            app.source_path
        ),
        theme.text,
        field == SourceField::Path,
    );
    push_text(
        cmds,
        2,
        h.saturating_sub(1),
        &app.status,
        theme.text_muted,
        false,
    );
}
fn event_row(event: &Event) -> String {
    format!(
        "{:<10} {:<11} {:<24} {}",
        event_date(event)
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "-".into()),
        event_time(event),
        event.title,
        event.calendar
    )
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
    vec![("Productivity".into(), "Open calendar agenda".into())]
}
fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let request = app.pending_request.take();
    let json = serde_json::json!({ "commands": commands_val, "hints": [], "palette_commands": palette_commands(), "request": request, "plugin_message": null, "consumed": consumed });
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
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[calendar-agenda] parse error: {e}: {trimmed}");
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
    const ICS: &str = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nUID:1\nDTSTART:20260704T090000\nDTEND:20260704T100000\nSUMMARY:Standup\nLOCATION:Room A\nDESCRIPTION:Daily sync\nEND:VEVENT\nBEGIN:VEVENT\nUID:2\nDTSTART;VALUE=DATE:20260705\nSUMMARY:Holiday\nEND:VEVENT\nEND:VCALENDAR";
    #[test]
    fn parses_ics_events() {
        let events = parse_ics(ICS, "Work");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].title, "Standup");
        assert!(events[1].all_day);
    }
    #[test]
    fn parses_datetime_and_date() {
        assert!(!parse_ics_datetime("20260704T090000").1);
        assert!(parse_ics_datetime("20260704").1);
    }
    #[test]
    fn unfolds_lines() {
        let lines = unfold_ics("SUMMARY:hello\n world");
        assert_eq!(lines[0], "SUMMARY:helloworld");
    }
    #[test]
    fn filters_events() {
        let mut app = App::default();
        app.events = parse_ics(ICS, "Work");
        app.search = "room".into();
        app.apply_filter();
        assert_eq!(app.filtered.len(), 1);
    }
    #[test]
    fn persists_sources() {
        let mut app = App::default();
        app.sources.push(CalendarSource {
            name: "Work".into(),
            path: "calendar.ics".into(),
        });
        let json = app.serialize();
        let state: PersistedState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.sources.len(), 1);
    }
}
