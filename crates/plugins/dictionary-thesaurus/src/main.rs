use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginRequest, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use serde::{Deserialize, Serialize};

const DB_KEY: &str = "dictionary-thesaurus";
const HISTORY_MAX: usize = 100;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LookupResult {
    word: String,
    phonetic: String,
    source_url: String,
    meanings: Vec<Meaning>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Meaning {
    part_of_speech: String,
    definitions: Vec<Definition>,
    synonyms: Vec<String>,
    antonyms: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Definition {
    text: String,
    example: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    recent: Vec<String>,
}

enum FetchMsg {
    Done(LookupResult),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Search,
    Results,
    Related,
    Recent,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    query: String,
    result: Option<LookupResult>,
    error: String,
    focus: Focus,
    selected_meaning: usize,
    selected_related: usize,
    recent: Vec<String>,
    recent_cursor: usize,
    rx_fetch: Option<mpsc::Receiver<FetchMsg>>,
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
            query: String::new(),
            result: None,
            error: String::new(),
            focus: Focus::Search,
            selected_meaning: 0,
            selected_related: 0,
            recent: Vec::new(),
            recent_cursor: 0,
            rx_fetch: None,
            status: "Type word · Enter search · Tab focus · c copy · h recent".into(),
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
                self.focus = Focus::Search;
                true
            }
            IpcKey::Char('h') => {
                self.focus = Focus::Recent;
                true
            }
            IpcKey::Enter => {
                self.enter_action();
                true
            }
            IpcKey::Char('c') => {
                self.copy_selected();
                true
            }
            IpcKey::Backspace if self.focus == Focus::Search => {
                self.query.pop();
                true
            }
            IpcKey::Backspace if self.focus != Focus::Search => {
                self.focus = Focus::Search;
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.move_selection(-1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.move_selection(1);
                true
            }
            IpcKey::Char(ch) if self.focus == Focus::Search && !ch.is_control() => {
                self.query.push(ch);
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn enter_action(&mut self) {
        match self.focus {
            Focus::Search => self.start_lookup(self.query.trim().to_string()),
            Focus::Related => {
                if let Some(word) = self.related_words().get(self.selected_related).cloned() {
                    self.query = word.clone();
                    self.start_lookup(word);
                }
            }
            Focus::Recent => {
                if let Some(word) = self.recent.get(self.recent_cursor).cloned() {
                    self.query = word.clone();
                    self.start_lookup(word);
                }
            }
            Focus::Results => self.focus = Focus::Related,
        }
    }

    fn start_lookup(&mut self, word: String) {
        let word = word.trim().to_string();
        if word.is_empty() {
            self.status = "Enter a word to search".into();
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.status = format!("Looking up `{word}` via dictionaryapi.dev");
        self.error.clear();
        std::thread::spawn(move || match lookup_word(&word) {
            Ok(result) => {
                let _ = tx.send(FetchMsg::Done(result));
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::Error(e));
            }
        });
    }

    fn handle_tick(&mut self) {
        if let Some(rx) = self.rx_fetch.take() {
            match rx.try_recv() {
                Ok(FetchMsg::Done(result)) => {
                    self.query = result.word.clone();
                    self.add_recent(result.word.clone());
                    self.result = Some(result);
                    self.error.clear();
                    self.selected_meaning = 0;
                    self.selected_related = 0;
                    self.status = "Lookup complete".into();
                    self.dirty = true;
                }
                Ok(FetchMsg::Error(e)) => {
                    self.error = e;
                    self.status = "Lookup failed".into();
                    self.dirty = true;
                }
                Err(mpsc::TryRecvError::Empty) => self.rx_fetch = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.error = "Lookup worker stopped".into();
                    self.dirty = true;
                }
            }
        }
    }

    fn add_recent(&mut self, word: String) {
        self.recent.retain(|item| item != &word);
        self.recent.insert(0, word);
        self.recent.truncate(HISTORY_MAX);
        self.recent_cursor = 0;
        self.pending_request = Some(PluginRequest::DbSet {
            key: DB_KEY.into(),
            value: self.serialize(),
        });
    }

    fn move_selection(&mut self, delta: i32) {
        let adjust = |value: &mut usize, max: usize| {
            if delta < 0 {
                *value = value.saturating_sub(1);
            } else {
                *value = (*value).min(max).saturating_add(1).min(max);
            }
        };
        match self.focus {
            Focus::Results => adjust(
                &mut self.selected_meaning,
                self.result
                    .as_ref()
                    .map(|r| r.meanings.len().saturating_sub(1))
                    .unwrap_or(0),
            ),
            Focus::Related => {
                let max = self.related_words().len().saturating_sub(1);
                adjust(&mut self.selected_related, max);
            }
            Focus::Recent => adjust(&mut self.recent_cursor, self.recent.len().saturating_sub(1)),
            Focus::Search => {}
        }
    }

    fn related_words(&self) -> Vec<String> {
        let mut words = Vec::new();
        if let Some(result) = &self.result {
            for meaning in &result.meanings {
                words.extend(meaning.synonyms.iter().cloned());
                words.extend(meaning.antonyms.iter().cloned());
            }
        }
        words.sort();
        words.dedup();
        words
    }

    fn copy_selected(&mut self) {
        let text = match self.focus {
            Focus::Search => self.query.clone(),
            Focus::Results => self
                .result
                .as_ref()
                .and_then(|r| r.meanings.get(self.selected_meaning))
                .and_then(|m| m.definitions.first())
                .map(|d| d.text.clone())
                .unwrap_or_default(),
            Focus::Related => self
                .related_words()
                .get(self.selected_related)
                .cloned()
                .unwrap_or_default(),
            Focus::Recent => self
                .recent
                .get(self.recent_cursor)
                .cloned()
                .unwrap_or_default(),
        };
        if text.is_empty() {
            self.status = "Nothing to copy".into();
            return;
        }
        match copy_to_clipboard(&text) {
            Ok(()) => self.status = "Copied".into(),
            Err(e) => self.status = format!("Clipboard error: {e}"),
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
            self.recent = state.recent.into_iter().take(HISTORY_MAX).collect();
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
        Focus::Search => Focus::Results,
        Focus::Results => Focus::Related,
        Focus::Related => Focus::Recent,
        Focus::Recent => Focus::Search,
    }
}
fn prev_focus(focus: Focus) -> Focus {
    match focus {
        Focus::Search => Focus::Recent,
        Focus::Results => Focus::Search,
        Focus::Related => Focus::Results,
        Focus::Recent => Focus::Related,
    }
}

fn lookup_word(word: &str) -> Result<LookupResult, String> {
    let url = format!(
        "https://api.dictionaryapi.dev/api/v2/entries/en/{}",
        urlencoding::encode(word)
    );
    let mut resp = ureq::get(&url)
        .call()
        .map_err(|e| classify_lookup_error(&e.to_string()))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    parse_lookup_response(&body)
}
fn classify_lookup_error(err: &str) -> String {
    if err.contains("404") {
        "Word not found".into()
    } else if err.contains("429") {
        "Rate limited by provider".into()
    } else {
        err.into()
    }
}
fn parse_lookup_response(body: &str) -> Result<LookupResult, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let entry = value
        .as_array()
        .and_then(|arr| arr.first())
        .ok_or_else(|| "Provider returned no entries".to_string())?;
    let word = entry["word"].as_str().unwrap_or_default().to_string();
    let phonetic = entry["phonetic"].as_str().unwrap_or_default().to_string();
    let source_url = entry["sourceUrls"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .unwrap_or("https://dictionaryapi.dev")
        .to_string();
    let mut meanings = Vec::new();
    if let Some(arr) = entry["meanings"].as_array() {
        for meaning in arr {
            let mut defs = Vec::new();
            if let Some(def_arr) = meaning["definitions"].as_array() {
                for def in def_arr {
                    defs.push(Definition {
                        text: def["definition"].as_str().unwrap_or_default().to_string(),
                        example: def["example"].as_str().unwrap_or_default().to_string(),
                    });
                }
            }
            meanings.push(Meaning {
                part_of_speech: meaning["partOfSpeech"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
                definitions: defs,
                synonyms: json_string_array(&meaning["synonyms"]),
                antonyms: json_string_array(&meaning["antonyms"]),
            });
        }
    }
    Ok(LookupResult {
        word,
        phonetic,
        source_url,
        meanings,
    })
}
fn json_string_array(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
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
        title: Some(" Dictionary / Thesaurus ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });
    push_text(
        &mut cmds,
        2,
        2,
        focus_line(app.focus, Focus::Search, "Search", &app.query),
        theme.text,
        app.focus == Focus::Search,
    );
    if !app.error.is_empty() {
        push_text(
            &mut cmds,
            2,
            4,
            format!("Error: {}", app.error),
            theme.error,
            true,
        );
    }
    render_results(app, &mut cmds, &theme, w, h);
    push_text(
        &mut cmds,
        2,
        h.saturating_sub(2),
        &app.status,
        theme.text_muted,
        false,
    );
    push_text(
        &mut cmds,
        2,
        h.saturating_sub(1),
        "/ search · Enter lookup/drill · Tab focus · c copy · h recent · Backspace return",
        theme.text_muted,
        false,
    );
    cmds
}
fn render_results(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let pane_y = 4;
    let pane_h = h.saturating_sub(8).max(4);
    let left_w = (w * 62 / 100).max(44);
    let right_x = left_w + 3;
    let right_w = w.saturating_sub(right_x + 2);
    let items = app.result.as_ref().map(meaning_items).unwrap_or_default();
    cmds.push(RenderCmd::List {
        x: 2,
        y: pane_y,
        w: left_w,
        h: pane_h,
        items,
        selected: if app.focus == Focus::Results {
            Some(
                app.selected_meaning.min(
                    app.result
                        .as_ref()
                        .map(|r| r.meanings.len().saturating_sub(1))
                        .unwrap_or(0),
                ),
            )
        } else {
            None
        },
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
    let related = app.related_words();
    let recent_items: Vec<String> = app.recent.iter().take(8).cloned().collect();
    let related_h = pane_h / 2;
    cmds.push(RenderCmd::Border {
        x: right_x,
        y: pane_y,
        w: right_w,
        h: related_h,
        fg: if app.focus == Focus::Related {
            theme.highlight
        } else {
            theme.border
        },
        borders: BORDER_ALL,
        bg: None,
        title: Some(" Related ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });
    cmds.push(RenderCmd::List {
        x: right_x + 1,
        y: pane_y + 1,
        w: right_w.saturating_sub(2),
        h: related_h.saturating_sub(2),
        items: related,
        selected: if app.focus == Focus::Related {
            Some(app.selected_related)
        } else {
            None
        },
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
        x: right_x,
        y: pane_y + related_h,
        w: right_w,
        h: pane_h.saturating_sub(related_h),
        fg: if app.focus == Focus::Recent {
            theme.highlight
        } else {
            theme.border
        },
        borders: BORDER_ALL,
        bg: None,
        title: Some(" Recent ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });
    cmds.push(RenderCmd::List {
        x: right_x + 1,
        y: pane_y + related_h + 1,
        w: right_w.saturating_sub(2),
        h: pane_h.saturating_sub(related_h + 2),
        items: recent_items,
        selected: if app.focus == Focus::Recent {
            Some(app.recent_cursor)
        } else {
            None
        },
        style: TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
        },
    });
    if let Some(result) = &app.result {
        push_text(
            cmds,
            2,
            h.saturating_sub(3),
            truncate(
                &format!(
                    "{} {} · source: {}",
                    result.word, result.phonetic, result.source_url
                ),
                w as usize - 4,
            ),
            theme.accent,
            false,
        );
    }
}
fn meaning_items(result: &LookupResult) -> Vec<String> {
    result
        .meanings
        .iter()
        .flat_map(|m| {
            let mut rows = vec![format!("{}", m.part_of_speech)];
            rows.extend(m.definitions.iter().take(3).map(|d| {
                format!(
                    "  - {}{}",
                    d.text,
                    if d.example.is_empty() {
                        String::new()
                    } else {
                        format!(" · e.g. {}", d.example)
                    }
                )
            }));
            rows
        })
        .collect()
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
    vec![("Reference".into(), "Open dictionary / thesaurus".into())]
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
            Ok(HostMsg::Tick) => {
                app.handle_tick();
                false
            }
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
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::PaletteCommand { .. }
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[dictionary-thesaurus] parse error: {e}: {trimmed}");
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
    const BODY: &str = r#"[{"word":"test","phonetic":"/tɛst/","sourceUrls":["https://example.com"],"meanings":[{"partOfSpeech":"noun","definitions":[{"definition":"A procedure.","example":"Run a test."}],"synonyms":["trial"],"antonyms":["certainty"]}]}]"#;
    #[test]
    fn parses_lookup_response() {
        let result = parse_lookup_response(BODY).unwrap();
        assert_eq!(result.word, "test");
        assert_eq!(result.meanings[0].definitions[0].text, "A procedure.");
        assert_eq!(result.meanings[0].synonyms, vec!["trial"]);
    }
    #[test]
    fn classifies_not_found() {
        assert_eq!(classify_lookup_error("404"), "Word not found");
    }
    #[test]
    fn related_words_are_deduped() {
        let mut app = App::default();
        app.result = Some(parse_lookup_response(BODY).unwrap());
        assert_eq!(app.related_words(), vec!["certainty", "trial"]);
    }
    #[test]
    fn recent_is_limited_and_deduped() {
        let mut app = App::default();
        app.add_recent("test".into());
        app.add_recent("test".into());
        assert_eq!(app.recent.len(), 1);
    }
    #[test]
    fn persists_recent() {
        let mut app = App::default();
        app.recent.push("word".into());
        let json = app.serialize();
        let mut loaded = App::default();
        loaded.load(&json);
        assert_eq!(loaded.recent, vec!["word"]);
    }
}
