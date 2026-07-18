use std::collections::BTreeSet;
use std::io::{BufRead, BufReader};

use chrono::{DateTime, Datelike, Duration, Local, Timelike};
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginRequest, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use serde::{Deserialize, Serialize};

const DB_KEY: &str = "cron-expression-helper";
const RECENT_MAX: usize = 20;

#[derive(Debug, Clone)]
struct CronField {
    raw: String,
    allowed: BTreeSet<u32>,
    wildcard: bool,
}

#[derive(Debug, Clone)]
struct CronSchedule {
    minute: CronField,
    hour: CronField,
    day_of_month: CronField,
    month: CronField,
    day_of_week: CronField,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RecentExpression {
    expression: String,
    timezone: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    recent: Vec<RecentExpression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Expression,
    Timezone,
    Results,
    Recent,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    expression: String,
    timezone: String,
    focus: Focus,
    next_count: usize,
    recent: Vec<RecentExpression>,
    recent_cursor: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 100, h: 28 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet { key: DB_KEY.into() }),
            expression: "0 9 * * 1-5".into(),
            timezone: "local".into(),
            focus: Focus::Expression,
            next_count: 8,
            recent: Vec::new(),
            recent_cursor: 0,
            status: "Standard 5-field cron · Tab focus · n more runs · c copy · Enter save/load"
                .into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.focus = next_focus(self.focus);
                true
            }
            IpcKey::BackTab => {
                self.focus = prev_focus(self.focus);
                true
            }
            IpcKey::Char('/') => {
                self.focus = Focus::Expression;
                true
            }
            IpcKey::Char('z') => {
                self.focus = Focus::Timezone;
                true
            }
            IpcKey::Char('h') => {
                self.focus = Focus::Recent;
                true
            }
            IpcKey::Char('n') if self.focus == Focus::Results => {
                self.next_count = (self.next_count + 1).min(20);
                true
            }
            IpcKey::Char('N') if self.focus == Focus::Results => {
                self.next_count = self.next_count.saturating_sub(1).max(1);
                true
            }
            IpcKey::Char('c') if self.focus == Focus::Results => {
                self.copy_explanation();
                true
            }
            IpcKey::Enter => {
                if self.focus == Focus::Recent {
                    self.load_recent();
                } else {
                    self.save_recent();
                }
                true
            }
            IpcKey::Backspace => {
                self.backspace();
                true
            }
            IpcKey::Up | IpcKey::Char('k') if self.focus == Focus::Recent => {
                self.recent_cursor = self.recent_cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') if self.focus == Focus::Recent => {
                let max = self.recent.len().saturating_sub(1);
                self.recent_cursor = self.recent_cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.insert_char(ch);
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn insert_char(&mut self, ch: char) {
        match self.focus {
            Focus::Expression => self.expression.push(ch),
            Focus::Timezone => self.timezone.push(ch),
            Focus::Results | Focus::Recent => {}
        }
    }

    fn backspace(&mut self) {
        match self.focus {
            Focus::Expression => {
                self.expression.pop();
            }
            Focus::Timezone => {
                self.timezone.pop();
            }
            Focus::Results | Focus::Recent => {}
        }
    }

    fn copy_explanation(&mut self) {
        match parse_cron(&self.expression) {
            Ok(schedule) => {
                let text = format!("{}\n{}", self.expression, explain_schedule(&schedule));
                match copy_to_clipboard(&text) {
                    Ok(()) => self.status = "Copied expression and explanation".into(),
                    Err(e) => self.status = format!("Clipboard error: {e}"),
                }
            }
            Err(e) => self.status = e,
        }
    }

    fn save_recent(&mut self) {
        if let Err(e) = parse_cron(&self.expression) {
            self.status = e;
            return;
        }
        let entry = RecentExpression {
            expression: self.expression.clone(),
            timezone: self.timezone.clone(),
        };
        self.recent.retain(|item| item != &entry);
        self.recent.insert(0, entry);
        self.recent.truncate(RECENT_MAX);
        self.recent_cursor = 0;
        self.pending_request = Some(PluginRequest::DbSet {
            key: DB_KEY.into(),
            value: self.serialize(),
        });
        self.status = "Saved recent expression".into();
    }

    fn load_recent(&mut self) {
        if let Some(entry) = self.recent.get(self.recent_cursor) {
            self.expression = entry.expression.clone();
            self.timezone = entry.timezone.clone();
            self.focus = Focus::Expression;
            self.status = "Loaded recent expression".into();
        }
    }

    fn serialize(&self) -> String {
        serde_json::to_string(&PersistedState {
            recent: self.recent.clone(),
        })
        .unwrap_or_default()
    }
    fn load(&mut self, json: &str) {
        if let Ok(state) = serde_json::from_str::<PersistedState>(json) {
            self.recent = state.recent.into_iter().take(RECENT_MAX).collect();
        }
    }
    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn next_focus(focus: Focus) -> Focus {
    match focus {
        Focus::Expression => Focus::Timezone,
        Focus::Timezone => Focus::Results,
        Focus::Results => Focus::Recent,
        Focus::Recent => Focus::Expression,
    }
}
fn prev_focus(focus: Focus) -> Focus {
    match focus {
        Focus::Expression => Focus::Recent,
        Focus::Timezone => Focus::Expression,
        Focus::Results => Focus::Timezone,
        Focus::Recent => Focus::Results,
    }
}

fn parse_cron(expression: &str) -> Result<CronSchedule, String> {
    let parts: Vec<&str> = expression.split_whitespace().collect();
    if parts.len() != 5 {
        return Err(format!(
            "Expected 5 fields, got {}. Format: minute hour day-of-month month day-of-week",
            parts.len()
        ));
    }
    Ok(CronSchedule {
        minute: parse_field("minute", parts[0], 0, 59, false)?,
        hour: parse_field("hour", parts[1], 0, 23, false)?,
        day_of_month: parse_field("day-of-month", parts[2], 1, 31, false)?,
        month: parse_field("month", parts[3], 1, 12, false)?,
        day_of_week: parse_field("day-of-week", parts[4], 0, 7, true)?,
    })
}

fn parse_field(
    name: &str,
    raw: &str,
    min: u32,
    max: u32,
    normalize_sunday: bool,
) -> Result<CronField, String> {
    if raw.trim().is_empty() {
        return Err(format!("{name}: field is empty"));
    }
    let mut allowed = BTreeSet::new();
    let wildcard = raw == "*" || raw.starts_with("*/");
    for part in raw.split(',') {
        parse_part(name, part.trim(), min, max, normalize_sunday, &mut allowed)?;
    }
    if allowed.is_empty() {
        return Err(format!("{name}: no values selected"));
    }
    Ok(CronField {
        raw: raw.to_string(),
        allowed,
        wildcard,
    })
}

fn parse_part(
    name: &str,
    part: &str,
    min: u32,
    max: u32,
    normalize_sunday: bool,
    allowed: &mut BTreeSet<u32>,
) -> Result<(), String> {
    if part.is_empty() {
        return Err(format!("{name}: empty list item"));
    }
    let (base, step) = if let Some((base, step)) = part.split_once('/') {
        let step = step
            .parse::<u32>()
            .map_err(|_| format!("{name}: invalid step `{step}`"))?;
        if step == 0 {
            return Err(format!("{name}: step cannot be 0"));
        }
        (base, step)
    } else {
        (part, 1)
    };
    let (start, end) = if base == "*" {
        (min, max)
    } else if let Some((start, end)) = base.split_once('-') {
        (
            parse_num(name, start, min, max)?,
            parse_num(name, end, min, max)?,
        )
    } else {
        let value = parse_num(name, base, min, max)?;
        (value, value)
    };
    if start > end {
        return Err(format!(
            "{name}: range start {start} is greater than end {end}"
        ));
    }
    let mut value = start;
    while value <= end {
        let normalized = if normalize_sunday && value == 7 {
            0
        } else {
            value
        };
        allowed.insert(normalized);
        value = value.saturating_add(step);
        if step == 0 {
            break;
        }
    }
    Ok(())
}

fn parse_num(name: &str, text: &str, min: u32, max: u32) -> Result<u32, String> {
    let value = text
        .parse::<u32>()
        .map_err(|_| format!("{name}: `{text}` is not a number"))?;
    if value < min || value > max {
        return Err(format!("{name}: {value} is outside {min}-{max}"));
    }
    Ok(value)
}

fn matches_schedule(schedule: &CronSchedule, dt: DateTime<Local>) -> bool {
    let minute = dt.minute();
    let hour = dt.hour();
    let dom = dt.day();
    let month = dt.month();
    let dow = dt.weekday().num_days_from_sunday();
    if !schedule.minute.allowed.contains(&minute)
        || !schedule.hour.allowed.contains(&hour)
        || !schedule.month.allowed.contains(&month)
    {
        return false;
    }
    let dom_match = schedule.day_of_month.allowed.contains(&dom);
    let dow_match = schedule.day_of_week.allowed.contains(&dow);
    if !schedule.day_of_month.wildcard && !schedule.day_of_week.wildcard {
        dom_match || dow_match
    } else {
        dom_match && dow_match
    }
}

fn next_runs(
    schedule: &CronSchedule,
    start: DateTime<Local>,
    count: usize,
) -> Vec<DateTime<Local>> {
    let mut out = Vec::new();
    let mut cursor = start + Duration::minutes(1);
    let max_checks = 60 * 24 * 366 * 5;
    for _ in 0..max_checks {
        if matches_schedule(schedule, cursor) {
            out.push(cursor);
            if out.len() >= count {
                break;
            }
        }
        cursor += Duration::minutes(1);
    }
    out
}

fn explain_schedule(schedule: &CronSchedule) -> String {
    let mut parts = Vec::new();
    parts.push(format!("minute {}", explain_field(&schedule.minute, 0, 59)));
    parts.push(format!("hour {}", explain_field(&schedule.hour, 0, 23)));
    parts.push(format!(
        "day-of-month {}",
        explain_field(&schedule.day_of_month, 1, 31)
    ));
    parts.push(format!("month {}", explain_field(&schedule.month, 1, 12)));
    parts.push(format!(
        "day-of-week {}",
        explain_field(&schedule.day_of_week, 0, 6)
    ));
    let dom_dow = if !schedule.day_of_month.wildcard && !schedule.day_of_week.wildcard {
        "DOM/DOW use OR semantics"
    } else {
        "DOM/DOW wildcard fields use AND semantics"
    };
    format!("Runs when {}. {dom_dow}.", parts.join(", "))
}

fn explain_field(field: &CronField, min: u32, max: u32) -> String {
    if field.wildcard && field.allowed.len() as u32 == max - min + 1 {
        return "every value".into();
    }
    if field.raw.contains('/') {
        return format!("matching `{}`", field.raw);
    }
    let values = field
        .allowed
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("in {{{values}}}")
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let theme = app.theme.clone();
    let w = app.area.w.max(76);
    let h = app.area.h.max(20);
    let parsed = parse_cron(&app.expression);
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
        title: Some(" Cron Expression Helper ".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    push_text(
        &mut cmds,
        2,
        2,
        focus_line(app.focus, Focus::Expression, "Expression", &app.expression),
        theme.text,
        app.focus == Focus::Expression,
    );
    push_text(
        &mut cmds,
        2,
        3,
        focus_line(
            app.focus,
            Focus::Timezone,
            "Timezone",
            &format!("{} (local only in MVP)", app.timezone),
        ),
        theme.text_muted,
        app.focus == Focus::Timezone,
    );
    match parsed {
        Ok(schedule) => render_success(app, &schedule, &mut cmds, &theme, w, h),
        Err(err) => push_text(&mut cmds, 2, 5, format!("Error: {err}"), theme.error, true),
    }
    push_text(
        &mut cmds,
        2,
        h.saturating_sub(2),
        &app.status,
        theme.text_muted,
        false,
    );
    cmds
}

fn render_success(
    app: &App,
    schedule: &CronSchedule,
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    w: u16,
    h: u16,
) {
    push_text(
        cmds,
        2,
        5,
        truncate(&explain_schedule(schedule), w as usize - 4),
        theme.success,
        false,
    );
    let rows = vec![
        vec![
            "minute".into(),
            schedule.minute.raw.clone(),
            range_preview(&schedule.minute),
        ],
        vec![
            "hour".into(),
            schedule.hour.raw.clone(),
            range_preview(&schedule.hour),
        ],
        vec![
            "day-of-month".into(),
            schedule.day_of_month.raw.clone(),
            range_preview(&schedule.day_of_month),
        ],
        vec![
            "month".into(),
            schedule.month.raw.clone(),
            range_preview(&schedule.month),
        ],
        vec![
            "day-of-week".into(),
            schedule.day_of_week.raw.clone(),
            range_preview(&schedule.day_of_week),
        ],
    ];
    cmds.push(RenderCmd::Table {
        x: 2,
        y: 7,
        w: w / 2 - 3,
        h: 8,
        header: vec!["Field".into(), "Raw".into(), "Values".into()],
        header_style: TextStyle {
            fg: Some(theme.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![14, 12, (w / 2).saturating_sub(31)],
        selected: None,
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
    let runs = next_runs(schedule, Local::now(), app.next_count);
    let items: Vec<String> = runs
        .iter()
        .map(|dt| dt.format("%Y-%m-%d %H:%M %Z").to_string())
        .collect();
    cmds.push(RenderCmd::List {
        x: w / 2 + 1,
        y: 7,
        w: w / 2 - 3,
        h: 8,
        items,
        selected: None,
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
    let recent_h = h.saturating_sub(18).max(1);
    let recent_items: Vec<String> = app
        .recent
        .iter()
        .map(|entry| format!("{} [{}]", entry.expression, entry.timezone))
        .collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 16,
        w: w.saturating_sub(4),
        h: recent_h,
        items: recent_items,
        selected: if app.focus == Focus::Recent {
            Some(app.recent_cursor.min(app.recent.len().saturating_sub(1)))
        } else {
            None
        },
        style: TextStyle {
            fg: Some(theme.text_muted),
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

fn range_preview(field: &CronField) -> String {
    let values: Vec<String> = field.allowed.iter().take(12).map(u32::to_string).collect();
    let suffix = if field.allowed.len() > 12 { "…" } else { "" };
    format!("{}{suffix}", values.join(","))
}
fn focus_line(active: Focus, field: Focus, label: &str, value: &str) -> String {
    format!(
        "{} {label}: {value}",
        if active == field { ">" } else { " " }
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
        ("z".into(), "timezone".into()),
        ("tab".into(), "focus".into()),
        ("n/N".into(), "run count".into()),
        ("h".into(), "recent".into()),
        ("enter".into(), "save/load".into()),
        ("c".into(), "copy".into()),
        ("esc".into(), "back".into()),
    ]
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("Plugins".into(), "Open cron expression helper".into())]
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
                log::error!("[cron-expression-helper] parse error: {e}: {trimmed}");
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
    use chrono::TimeZone;

    #[test]
    fn parses_standard_expression() {
        let cron = parse_cron("0 9 * * 1-5").unwrap();
        assert!(cron.minute.allowed.contains(&0));
        assert!(cron.hour.allowed.contains(&9));
        assert!(cron.day_of_week.allowed.contains(&1));
        assert!(cron.day_of_week.allowed.contains(&5));
    }

    #[test]
    fn reports_field_specific_errors() {
        let err = parse_cron("70 9 * * *").unwrap_err();
        assert!(err.contains("minute"));
    }

    #[test]
    fn supports_steps_and_lists() {
        let field = parse_field("minute", "*/15,7", 0, 59, false).unwrap();
        assert!(field.allowed.contains(&0));
        assert!(field.allowed.contains(&15));
        assert!(field.allowed.contains(&7));
    }

    #[test]
    fn matches_weekday_schedule() {
        let cron = parse_cron("0 9 * * 1-5").unwrap();
        let dt = Local
            .with_ymd_and_hms(2026, 7, 6, 9, 0, 0)
            .single()
            .unwrap();
        assert!(matches_schedule(&cron, dt));
    }

    #[test]
    fn computes_next_runs() {
        let cron = parse_cron("0 9 * * *").unwrap();
        let start = Local
            .with_ymd_and_hms(2026, 7, 4, 8, 58, 0)
            .single()
            .unwrap();
        let runs = next_runs(&cron, start, 2);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].hour(), 9);
        assert_eq!(runs[0].minute(), 0);
    }

    #[test]
    fn recent_roundtrip() {
        let mut app = App::default();
        app.save_recent();
        let json = app.serialize();
        let mut loaded = App::default();
        loaded.load(&json);
        assert_eq!(loaded.recent.len(), 1);
    }
}
