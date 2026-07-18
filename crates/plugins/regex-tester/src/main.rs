use std::io::{BufRead, BufReader};

use regex::{Regex, RegexBuilder};
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginRequest, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use serde::{Deserialize, Serialize};

const MAX_MATCHES: usize = 200;
const MAX_SAMPLE_BYTES: usize = 32 * 1024;
const RECENT_MAX: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Pattern,
    Sample,
    Replacement,
    Results,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RegexFlags {
    case_insensitive: bool,
    multi_line: bool,
    dot_matches_new_line: bool,
}

impl Default for RegexFlags {
    fn default() -> Self {
        Self {
            case_insensitive: false,
            multi_line: true,
            dot_matches_new_line: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecentPattern {
    pattern: String,
    flags: RegexFlags,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    recent_patterns: Vec<RecentPattern>,
}

#[derive(Debug, Clone)]
struct MatchInfo {
    start: usize,
    end: usize,
    text: String,
    captures: Vec<(usize, Option<String>)>,
}

struct Analysis {
    regex: Option<Regex>,
    matches: Vec<MatchInfo>,
    replacement_preview: Option<String>,
    error: Option<String>,
    truncated: bool,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    pattern: String,
    sample: String,
    replacement: String,
    flags: RegexFlags,
    focus: Focus,
    selected_match: usize,
    scroll: usize,
    status: String,
    recent_patterns: Vec<RecentPattern>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 100, h: 28 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet {
                key: "regex-tester".into(),
            }),
            pattern: r"(\w+)".into(),
            sample: "hello santui\nregex tester".into(),
            replacement: "[$1]".into(),
            flags: RegexFlags::default(),
            focus: Focus::Pattern,
            selected_match: 0,
            scroll: 0,
            status: "Tab focus · i/m/d flags · n/p match · c copy · Enter save recent".into(),
            recent_patterns: Vec::new(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::Pattern => Focus::Sample,
                    Focus::Sample => Focus::Replacement,
                    Focus::Replacement => Focus::Results,
                    Focus::Results => Focus::Pattern,
                };
                true
            }
            IpcKey::BackTab => {
                self.focus = match self.focus {
                    Focus::Pattern => Focus::Results,
                    Focus::Sample => Focus::Pattern,
                    Focus::Replacement => Focus::Sample,
                    Focus::Results => Focus::Replacement,
                };
                true
            }
            IpcKey::Char('i') if self.focus == Focus::Results => {
                self.flags.case_insensitive = !self.flags.case_insensitive;
                self.status = flag_status("case-insensitive", self.flags.case_insensitive);
                true
            }
            IpcKey::Char('m') if self.focus == Focus::Results => {
                self.flags.multi_line = !self.flags.multi_line;
                self.status = flag_status("multi-line", self.flags.multi_line);
                true
            }
            IpcKey::Char('d') if self.focus == Focus::Results => {
                self.flags.dot_matches_new_line = !self.flags.dot_matches_new_line;
                self.status = flag_status("dot matches newline", self.flags.dot_matches_new_line);
                true
            }
            IpcKey::Char('n') if self.focus == Focus::Results => {
                let max = analyze(&self.pattern, &self.sample, &self.replacement, &self.flags)
                    .matches
                    .len()
                    .saturating_sub(1);
                self.selected_match = self.selected_match.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Char('p') if self.focus == Focus::Results => {
                self.selected_match = self.selected_match.saturating_sub(1);
                true
            }
            IpcKey::Char('c') if self.focus == Focus::Results => {
                let analysis = analyze(&self.pattern, &self.sample, &self.replacement, &self.flags);
                let text = analysis
                    .matches
                    .get(self.selected_match)
                    .map(|m| m.text.clone())
                    .or(analysis.replacement_preview)
                    .unwrap_or_default();
                match copy_to_clipboard(&text) {
                    Ok(()) => self.status = "Copied selected match/replacement preview".into(),
                    Err(e) => self.status = format!("Clipboard error: {e}"),
                }
                true
            }
            IpcKey::Enter => {
                match self.focus {
                    Focus::Sample => self.insert_char('\n'),
                    Focus::Replacement => self.insert_char('\n'),
                    Focus::Pattern | Focus::Results => self.save_recent_pattern(),
                }
                true
            }
            IpcKey::Backspace => {
                self.backspace();
                true
            }
            IpcKey::Up | IpcKey::Char('k') if self.focus == Focus::Results => {
                self.scroll = self.scroll.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') if self.focus == Focus::Results => {
                self.scroll = self.scroll.saturating_add(1);
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.insert_char(c);
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn insert_char(&mut self, c: char) {
        match self.focus {
            Focus::Pattern => self.pattern.push(c),
            Focus::Sample => {
                if self.sample.len() < MAX_SAMPLE_BYTES {
                    self.sample.push(c);
                } else {
                    self.status = format!("Sample capped at {MAX_SAMPLE_BYTES} bytes");
                }
            }
            Focus::Replacement => self.replacement.push(c),
            Focus::Results => {}
        }
        self.selected_match = 0;
    }

    fn backspace(&mut self) {
        match self.focus {
            Focus::Pattern => {
                self.pattern.pop();
            }
            Focus::Sample => {
                self.sample.pop();
            }
            Focus::Replacement => {
                self.replacement.pop();
            }
            Focus::Results => {}
        }
        self.selected_match = 0;
    }

    fn save_recent_pattern(&mut self) {
        if self.pattern.trim().is_empty() {
            self.status = "Pattern is empty".into();
            return;
        }
        if compile_regex(&self.pattern, &self.flags).is_err() {
            self.status = "Fix regex before saving recent pattern".into();
            return;
        }
        self.recent_patterns
            .retain(|entry| entry.pattern != self.pattern || entry.flags != self.flags);
        self.recent_patterns.insert(
            0,
            RecentPattern {
                pattern: self.pattern.clone(),
                flags: self.flags.clone(),
            },
        );
        self.recent_patterns.truncate(RECENT_MAX);
        self.pending_request = Some(PluginRequest::DbSet {
            key: "regex-tester".into(),
            value: self.serialize(),
        });
        self.status = "Saved recent pattern".into();
    }

    fn serialize(&self) -> String {
        serde_json::to_string(&PersistedState {
            recent_patterns: self.recent_patterns.clone(),
        })
        .unwrap_or_default()
    }

    fn load(&mut self, json: &str) {
        if let Ok(state) = serde_json::from_str::<PersistedState>(json) {
            self.recent_patterns = state.recent_patterns.into_iter().take(RECENT_MAX).collect();
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

fn flag_status(label: &str, enabled: bool) -> String {
    format!("{label} {}", if enabled { "enabled" } else { "disabled" })
}

fn compile_regex(pattern: &str, flags: &RegexFlags) -> Result<Regex, String> {
    RegexBuilder::new(pattern)
        .case_insensitive(flags.case_insensitive)
        .multi_line(flags.multi_line)
        .dot_matches_new_line(flags.dot_matches_new_line)
        .build()
        .map_err(|e| e.to_string())
}

fn analyze(pattern: &str, sample: &str, replacement: &str, flags: &RegexFlags) -> Analysis {
    let regex = match compile_regex(pattern, flags) {
        Ok(regex) => regex,
        Err(e) => {
            return Analysis {
                regex: None,
                matches: Vec::new(),
                replacement_preview: None,
                error: Some(e),
                truncated: false,
            }
        }
    };
    let sample = if sample.len() > MAX_SAMPLE_BYTES {
        &sample[..safe_boundary(sample, MAX_SAMPLE_BYTES)]
    } else {
        sample
    };
    let mut matches = Vec::new();
    for captures in regex.captures_iter(sample).take(MAX_MATCHES) {
        if let Some(mat) = captures.get(0) {
            let caps = captures
                .iter()
                .enumerate()
                .skip(1)
                .map(|(idx, cap)| (idx, cap.map(|m| m.as_str().to_string())))
                .collect();
            matches.push(MatchInfo {
                start: mat.start(),
                end: mat.end(),
                text: mat.as_str().to_string(),
                captures: caps,
            });
        }
    }
    let truncated = regex.find_iter(sample).nth(MAX_MATCHES).is_some();
    let replacement_preview = if replacement.is_empty() {
        None
    } else {
        Some(regex.replace_all(sample, replacement).to_string())
    };
    Analysis {
        regex: Some(regex),
        matches,
        replacement_preview,
        error: None,
        truncated,
    }
}

fn safe_boundary(text: &str, max: usize) -> usize {
    let mut idx = max.min(text.len());
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
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
    let w = app.area.w.max(60);
    let h = app.area.h.max(16);
    let analysis = analyze(&app.pattern, &app.sample, &app.replacement, &app.flags);

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
        title: Some(" Regex Tester ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });

    push_text(
        &mut cmds,
        2,
        2,
        focus_line(app.focus, Focus::Pattern, "Pattern", &app.pattern),
        theme.text,
        app.focus == Focus::Pattern,
    );
    let flags = format!(
        "Flags: [{}] ignore case  [{}] multi-line  [{}] dot-newline",
        mark(app.flags.case_insensitive),
        mark(app.flags.multi_line),
        mark(app.flags.dot_matches_new_line)
    );
    push_text(&mut cmds, 2, 3, flags, theme.text_muted, false);
    push_text(
        &mut cmds,
        2,
        4,
        focus_line(
            app.focus,
            Focus::Replacement,
            "Replace",
            &visible_one_line(&app.replacement),
        ),
        theme.text,
        app.focus == Focus::Replacement,
    );

    let pane_y = 6;
    let pane_h = h.saturating_sub(10).max(4);
    let left_w = (w / 2).saturating_sub(3).max(20);
    let right_x = left_w + 4;
    let right_w = w.saturating_sub(right_x + 2).max(20);

    cmds.push(RenderCmd::Border {
        x: 2,
        y: pane_y - 1,
        w: left_w,
        h: pane_h + 2,
        fg: if app.focus == Focus::Sample {
            theme.highlight
        } else {
            theme.border
        },
        borders: BORDER_ALL,
        bg: None,
        title: Some(" Sample ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    let sample_lines = app
        .sample
        .lines()
        .skip(app.scroll)
        .take(pane_h as usize)
        .collect::<Vec<_>>()
        .join("\n");
    cmds.push(RenderCmd::Paragraph {
        x: 3,
        y: pane_y,
        w: left_w.saturating_sub(2),
        h: pane_h,
        text: sample_lines,
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

    cmds.push(RenderCmd::Border {
        x: right_x,
        y: pane_y - 1,
        w: right_w,
        h: pane_h + 2,
        fg: if app.focus == Focus::Results {
            theme.highlight
        } else {
            theme.border
        },
        borders: BORDER_ALL,
        bg: None,
        title: Some(" Matches ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    let items = match &analysis.error {
        Some(err) => vec![format!("Error: {err}")],
        None => match_items(&analysis),
    };
    cmds.push(RenderCmd::List {
        x: right_x + 1,
        y: pane_y,
        w: right_w.saturating_sub(2),
        h: pane_h,
        items,
        selected: if analysis.error.is_none() {
            Some(
                app.selected_match
                    .min(analysis.matches.len().saturating_sub(1)),
            )
        } else {
            None
        },
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

    let detail_y = h.saturating_sub(4);
    if let Some(mat) = analysis.matches.get(app.selected_match) {
        let captures = if mat.captures.is_empty() {
            "captures: none".into()
        } else {
            mat.captures
                .iter()
                .map(|(idx, value)| format!("${idx}={}", value.as_deref().unwrap_or("<none>")))
                .collect::<Vec<_>>()
                .join("  ")
        };
        push_text(
            &mut cmds,
            2,
            detail_y,
            truncate(
                &format!("Selected: bytes {}..{} {:?}", mat.start, mat.end, mat.text),
                w as usize - 4,
            ),
            theme.success,
            true,
        );
        push_text(
            &mut cmds,
            2,
            detail_y + 1,
            truncate(&captures, w as usize - 4),
            theme.text_muted,
            false,
        );
    } else if analysis.regex.is_some() {
        push_text(
            &mut cmds,
            2,
            detail_y,
            "No matches",
            theme.text_muted,
            false,
        );
    }
    if let Some(preview) = &analysis.replacement_preview {
        push_text(
            &mut cmds,
            2,
            detail_y + 2,
            truncate(
                &format!("Replacement: {}", visible_one_line(preview)),
                w as usize - 4,
            ),
            theme.accent,
            false,
        );
    }
    let mut status = app.status.clone();
    if analysis.truncated {
        status.push_str(" · match list truncated");
    }
    push_text(
        &mut cmds,
        2,
        h.saturating_sub(1),
        truncate(&status, w as usize - 4),
        theme.text_muted,
        false,
    );
    cmds
}

fn match_items(analysis: &Analysis) -> Vec<String> {
    if analysis.matches.is_empty() {
        return vec!["No matches".into()];
    }
    analysis
        .matches
        .iter()
        .enumerate()
        .map(|(idx, mat)| {
            format!(
                "#{:<3} {:>5}..{:<5} {}",
                idx + 1,
                mat.start,
                mat.end,
                visible_one_line(&mat.text)
            )
        })
        .collect()
}

fn mark(enabled: bool) -> &'static str {
    if enabled {
        "x"
    } else {
        " "
    }
}

fn focus_line(active: Focus, field: Focus, label: &str, value: &str) -> String {
    format!(
        "{} {label}: {value}",
        if active == field { ">" } else { " " }
    )
}

fn visible_one_line(value: &str) -> String {
    value.replace('\n', "⏎")
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
    vec![("Utilities".into(), "Open regex tester".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: vec![],
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
                if key == "regex-tester" {
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
                log::error!("[regex-tester] parse error: {e}: {trimmed}");
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

    #[test]
    fn reports_compile_errors() {
        let analysis = analyze("(", "sample", "", &RegexFlags::default());
        assert!(analysis.error.is_some());
    }

    #[test]
    fn finds_matches_and_captures() {
        let analysis = analyze(r"(foo)(\d)", "foo1 bar foo2", "$1", &RegexFlags::default());
        assert_eq!(analysis.matches.len(), 2);
        assert_eq!(analysis.matches[0].captures[0].1.as_deref(), Some("foo"));
        assert_eq!(analysis.matches[0].captures[1].1.as_deref(), Some("1"));
    }

    #[test]
    fn replacement_preview_uses_same_regex() {
        let analysis = analyze(r"(\w+)", "a b", "[$1]", &RegexFlags::default());
        assert_eq!(analysis.replacement_preview.as_deref(), Some("[a] [b]"));
    }

    #[test]
    fn case_insensitive_flag_changes_results() {
        let flags = RegexFlags {
            case_insensitive: true,
            ..RegexFlags::default()
        };
        let analysis = analyze("abc", "ABC", "", &flags);
        assert_eq!(analysis.matches.len(), 1);
    }

    #[test]
    fn saves_recent_patterns_without_duplicates() {
        let mut app = App::default();
        app.pattern = "abc".into();
        app.save_recent_pattern();
        app.save_recent_pattern();
        assert_eq!(app.recent_patterns.len(), 1);
        assert!(app.pending_request.is_some());
    }
}
