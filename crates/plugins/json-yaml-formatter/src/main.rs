use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, RenderCmd, TextStyle, ThemeData, BORDER_ALL};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    Auto,
    Json,
    Yaml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Json,
    Yaml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Format,
    MinifyJson,
}

struct TransformResult {
    output: String,
    detected: &'static str,
    status: String,
    ok: bool,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    input: String,
    input_format: InputFormat,
    output_format: OutputFormat,
    action: Action,
    focus: Focus,
    input_scroll: usize,
    output_scroll: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 110, h: 30 },
            dirty: true,
            cached_commands: Vec::new(),
            input: "{\"name\":\"Santui\",\"plugins\":[\"json\",\"yaml\"]}".into(),
            input_format: InputFormat::Auto,
            output_format: OutputFormat::Json,
            action: Action::Format,
            focus: Focus::Input,
            input_scroll: 0,
            output_scroll: 0,
            status: "Tab focus · f format · m minify JSON · j/y output · 1/2 input mode · c copy"
                .into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab | IpcKey::BackTab => {
                self.focus = if self.focus == Focus::Input {
                    Focus::Output
                } else {
                    Focus::Input
                };
                true
            }
            IpcKey::Char('f') if self.focus == Focus::Output => {
                self.action = Action::Format;
                self.status = "Formatting output".into();
                true
            }
            IpcKey::Char('m') if self.focus == Focus::Output => {
                self.action = Action::MinifyJson;
                self.output_format = OutputFormat::Json;
                self.status = "Minifying JSON output".into();
                true
            }
            IpcKey::Char('j') if self.focus == Focus::Output => {
                self.output_format = OutputFormat::Json;
                self.action = Action::Format;
                true
            }
            IpcKey::Char('y') if self.focus == Focus::Output => {
                self.output_format = OutputFormat::Yaml;
                self.action = Action::Format;
                true
            }
            IpcKey::Char('1') if self.focus == Focus::Output => {
                self.input_format = InputFormat::Auto;
                true
            }
            IpcKey::Char('2') if self.focus == Focus::Output => {
                self.input_format = match self.input_format {
                    InputFormat::Auto => InputFormat::Json,
                    InputFormat::Json => InputFormat::Yaml,
                    InputFormat::Yaml => InputFormat::Auto,
                };
                true
            }
            IpcKey::Char('r') if self.focus == Focus::Output => {
                self.input.clear();
                self.input_scroll = 0;
                self.output_scroll = 0;
                self.focus = Focus::Input;
                self.status = "Input cleared".into();
                true
            }
            IpcKey::Char('c') if self.focus == Focus::Output => {
                let result = transform(
                    &self.input,
                    self.input_format,
                    self.output_format,
                    self.action,
                );
                if result.ok {
                    match copy_to_clipboard(&result.output) {
                        Ok(()) => self.status = "Copied formatted output".into(),
                        Err(e) => self.status = format!("Clipboard error: {e}"),
                    }
                } else {
                    self.status = "Fix parse error before copying".into();
                }
                true
            }
            IpcKey::Enter if self.focus == Focus::Input => {
                self.input.push('\n');
                true
            }
            IpcKey::Backspace if self.focus == Focus::Input => {
                self.input.pop();
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                match self.focus {
                    Focus::Input => self.input_scroll = self.input_scroll.saturating_sub(1),
                    Focus::Output => self.output_scroll = self.output_scroll.saturating_sub(1),
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') if self.focus == Focus::Input => {
                self.input_scroll = self.input_scroll.saturating_add(1);
                true
            }
            IpcKey::Down if self.focus == Focus::Output => {
                self.output_scroll = self.output_scroll.saturating_add(1);
                true
            }
            IpcKey::Char(c) if self.focus == Focus::Input && !c.is_control() => {
                self.input.push(c);
                true
            }
            IpcKey::Esc => false,
            _ => false,
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

fn transform(
    input: &str,
    input_format: InputFormat,
    output_format: OutputFormat,
    action: Action,
) -> TransformResult {
    if input.trim().is_empty() {
        return TransformResult {
            output: String::new(),
            detected: "empty",
            status: "Input is empty".into(),
            ok: true,
        };
    }

    match parse_value(input, input_format) {
        Ok((value, detected)) => {
            let rendered = match (output_format, action) {
                (OutputFormat::Json, Action::Format) => {
                    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
                }
                (OutputFormat::Json, Action::MinifyJson) => {
                    serde_json::to_string(&value).map_err(|e| e.to_string())
                }
                (OutputFormat::Yaml, Action::Format) | (OutputFormat::Yaml, Action::MinifyJson) => {
                    serde_yaml::to_string(&value).map_err(|e| e.to_string())
                }
            };
            match rendered {
                Ok(output) => TransformResult {
                    output,
                    detected,
                    status: "OK".into(),
                    ok: true,
                },
                Err(e) => TransformResult {
                    output: String::new(),
                    detected,
                    status: format!("Render error: {e}"),
                    ok: false,
                },
            }
        }
        Err(err) => TransformResult {
            output: err.clone(),
            detected: "unknown",
            status: err,
            ok: false,
        },
    }
}

fn parse_value(
    input: &str,
    input_format: InputFormat,
) -> Result<(serde_json::Value, &'static str), String> {
    match input_format {
        InputFormat::Json => parse_json(input).map(|value| (value, "JSON")),
        InputFormat::Yaml => parse_yaml(input).map(|value| (value, "YAML")),
        InputFormat::Auto => parse_json(input)
            .map(|value| (value, "JSON"))
            .or_else(|json_err| {
                parse_yaml(input)
                    .map(|value| (value, "YAML"))
                    .map_err(|yaml_err| format!("JSON error: {json_err}\nYAML error: {yaml_err}"))
            }),
    }
}

fn parse_json(input: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str::<serde_json::Value>(input).map_err(|e| {
        format!(
            "JSON parse error at line {}, column {}: {}",
            e.line(),
            e.column(),
            e
        )
    })
}

fn parse_yaml(input: &str) -> Result<serde_json::Value, String> {
    let yaml =
        serde_yaml::from_str::<serde_yaml::Value>(input).map_err(|e| match e.location() {
            Some(loc) => format!(
                "YAML parse error at line {}, column {}: {}",
                loc.line(),
                loc.column(),
                e
            ),
            None => format!("YAML parse error: {e}"),
        })?;
    serde_json::to_value(yaml).map_err(|e| format!("YAML conversion error: {e}"))
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
    let w = app.area.w.max(64);
    let h = app.area.h.max(18);
    let result = transform(&app.input, app.input_format, app.output_format, app.action);

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
        title: Some(" JSON / YAML Formatter ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });

    let header = format!(
        "Input: {} ({})  Output: {}  Action: {}  Detected: {}",
        input_label(app.input_format),
        if app.focus == Focus::Input {
            "focused"
        } else {
            "1/2 cycle"
        },
        output_label(app.output_format),
        action_label(app.action),
        result.detected
    );
    push_text(
        &mut cmds,
        2,
        2,
        truncate(&header, w as usize - 4),
        theme.text,
        true,
    );

    let pane_y = 4;
    let pane_h = h.saturating_sub(8).max(6);
    let left_w = (w / 2).saturating_sub(3).max(24);
    let right_x = left_w + 4;
    let right_w = w.saturating_sub(right_x + 2).max(24);

    cmds.push(RenderCmd::Border {
        x: 2,
        y: pane_y - 1,
        w: left_w,
        h: pane_h + 2,
        fg: if app.focus == Focus::Input {
            theme.highlight
        } else {
            theme.border
        },
        borders: BORDER_ALL,
        bg: None,
        title: Some(" Input ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });
    cmds.push(RenderCmd::Paragraph {
        x: 3,
        y: pane_y,
        w: left_w.saturating_sub(2),
        h: pane_h,
        text: visible_lines(&app.input, app.input_scroll, pane_h),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
        },
        wrap: false,
    });

    cmds.push(RenderCmd::Border {
        x: right_x,
        y: pane_y - 1,
        w: right_w,
        h: pane_h + 2,
        fg: if app.focus == Focus::Output {
            theme.highlight
        } else {
            theme.border
        },
        borders: BORDER_ALL,
        bg: None,
        title: Some(" Output ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });
    cmds.push(RenderCmd::Paragraph {
        x: right_x + 1,
        y: pane_y,
        w: right_w.saturating_sub(2),
        h: pane_h,
        text: visible_lines(&result.output, app.output_scroll, pane_h),
        style: TextStyle {
            fg: Some(if result.ok { theme.text } else { theme.error }),
            bg: None,
            bold: false,
        },
        wrap: false,
    });

    let counts = format!(
        "Input: {} bytes, {} lines · Output: {} bytes, {} lines",
        app.input.len(),
        line_count(&app.input),
        result.output.len(),
        line_count(&result.output)
    );
    push_text(
        &mut cmds,
        2,
        h.saturating_sub(3),
        counts,
        theme.text_muted,
        false,
    );
    let status = if result.ok {
        app.status.clone()
    } else {
        result.status
    };
    push_text(
        &mut cmds,
        2,
        h.saturating_sub(2),
        truncate(&status, w as usize - 4),
        if result.ok {
            theme.success
        } else {
            theme.error
        },
        false,
    );
    push_text(&mut cmds, 2, h.saturating_sub(1), "Tab focus · f format · m minify · j JSON · y YAML · 2 input JSON/YAML/Auto · c copy · r reset", theme.text_muted, false);
    cmds
}

fn visible_lines(text: &str, scroll: usize, height: u16) -> String {
    text.lines()
        .skip(scroll)
        .take(height as usize)
        .collect::<Vec<_>>()
        .join("\n")
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count().max(1)
    }
}

fn input_label(format: InputFormat) -> &'static str {
    match format {
        InputFormat::Auto => "Auto",
        InputFormat::Json => "JSON",
        InputFormat::Yaml => "YAML",
    }
}

fn output_label(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Json => "JSON",
        OutputFormat::Yaml => "YAML",
    }
}

fn action_label(action: Action) -> &'static str {
    match action {
        Action::Format => "Format",
        Action::MinifyJson => "Minify JSON",
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
    vec![("Utilities".into(), "Open JSON/YAML formatter".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": [],
        "palette_commands": palette_commands(),
        "request": null,
        "plugin_message": null,
        "consumed": consumed,
    });
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
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Tick
                | HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::PaletteCommand { .. }
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[json-yaml-formatter] parse error: {e}: {trimmed}");
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
    fn formats_json_pretty() {
        let result = transform(
            r#"{"a":1,"b":[true]}"#,
            InputFormat::Json,
            OutputFormat::Json,
            Action::Format,
        );
        assert!(result.ok);
        assert!(result.output.contains("\n  \"a\": 1"));
    }

    #[test]
    fn minifies_json() {
        let result = transform(
            "{\n  \"a\": 1\n}",
            InputFormat::Json,
            OutputFormat::Json,
            Action::MinifyJson,
        );
        assert_eq!(result.output, r#"{"a":1}"#);
    }

    #[test]
    fn converts_yaml_to_json() {
        let result = transform(
            "name: Santui\nplugins:\n  - json\n  - yaml\n",
            InputFormat::Yaml,
            OutputFormat::Json,
            Action::Format,
        );
        assert!(result.ok);
        assert!(result.output.contains("\"name\": \"Santui\""));
    }

    #[test]
    fn converts_json_to_yaml() {
        let result = transform(
            r#"{"name":"Santui"}"#,
            InputFormat::Json,
            OutputFormat::Yaml,
            Action::Format,
        );
        assert!(result.ok);
        assert!(result.output.contains("name: Santui"));
    }

    #[test]
    fn reports_line_column_errors() {
        let result = transform(
            "{\n  bad\n}",
            InputFormat::Json,
            OutputFormat::Json,
            Action::Format,
        );
        assert!(!result.ok);
        assert!(result.status.contains("line"));
        assert!(result.status.contains("column"));
    }
}
