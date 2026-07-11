use std::io::{BufRead, BufReader, Write};
use std::process::Command;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Editing,
    Running,
    Output,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    code: Vec<String>,
    cursor: usize,
    mode: Mode,
    output: String,
    status: String,
    language: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            code: vec![
                String::from("#!/bin/bash"),
                String::new(),
                String::from("echo \"Hello, World!\""),
            ],
            cursor: 0,
            mode: Mode::Editing,
            output: String::new(),
            status: String::from("Ctrl+R run  ·  tab lang  ·  type to edit"),
            language: String::from("bash"),
        }
    }
}

impl App {
    fn detect_language(&self) -> String {
        if let Some(first) = self.code.first() {
            if first.starts_with("#!/") {
                let parts: Vec<&str> = first.splitn(3, '/').collect();
                if parts.len() >= 3 {
                    let lang = parts[2].trim();
                    match lang {
                        "sh" | "bash" | "zsh" | "dash" => return String::from("bash"),
                        "python" | "python3" => return String::from("python3"),
                        "node" | "nodejs" => return String::from("node"),
                        "ruby" => return String::from("ruby"),
                        "perl" => return String::from("perl"),
                        _ => return lang.to_string(),
                    }
                }
            }
        }
        String::from("bash")
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match self.mode {
            Mode::Editing => match key {
                IpcKey::Esc => false,
                IpcKey::Up | IpcKey::Char('k') => {
                    self.cursor = self.cursor.saturating_sub(1);
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    let max = self.code.len().saturating_sub(1);
                    self.cursor = self.cursor.saturating_add(1).min(max);
                    true
                }
                IpcKey::Enter => {
                    let rest = if self.cursor < self.code.len() {
                        self.code[self.cursor].split_off(0)
                    } else {
                        String::new()
                    };
                    if self.cursor < self.code.len() {
                        self.code.insert(self.cursor + 1, rest);
                    } else {
                        self.code.push(rest);
                    }
                    self.cursor += 1;
                    true
                }
                IpcKey::Backspace => {
                    if self.cursor < self.code.len() && !self.code[self.cursor].is_empty() {
                        self.code[self.cursor].pop();
                    } else if self.cursor > 0 && self.cursor <= self.code.len() {
                        self.code.remove(self.cursor);
                        self.cursor -= 1;
                    }
                    true
                }
                IpcKey::Tab => {
                    self.language = match self.language.as_str() {
                        "bash" => String::from("python3"),
                        "python3" => String::from("node"),
                        "node" => String::from("ruby"),
                        "ruby" => String::from("bash"),
                        _ => String::from("bash"),
                    };
                    if !self.code.is_empty() {
                        self.code[0] = format!("#!/usr/bin/env {}", self.language);
                    }
                    self.status = format!("Language: {}", self.language);
                    true
                }
                IpcKey::Char('r') if modifiers.ctrl => {
                    self.run_code();
                    true
                }
                IpcKey::Char(c) if !c.is_control() => {
                    if self.cursor < self.code.len() {
                        self.code[self.cursor].push(c);
                    } else {
                        self.code.push(c.to_string());
                    }
                    true
                }
                _ => true,
            },
            Mode::Running | Mode::Output => match key {
                IpcKey::Esc => {
                    self.mode = Mode::Editing;
                    self.dirty = true;
                    true
                }
                _ => true,
            },
        }
    }

    fn run_code(&mut self) {
        let source = self.code.join("\n");
        let lang = self.detect_language();
        self.language = lang.clone();
        self.mode = Mode::Running;
        self.status = String::from("Running...");

        let tmpfile = format!("/tmp/santui-code-runner-{}.{}", std::process::id(), lang);
        if let Err(e) = std::fs::write(&tmpfile, &source) {
            self.output = format!("Error writing temp file: {e}");
            self.mode = Mode::Output;
            self.status = String::from("Run failed");
            return;
        }

        let cmd_str = if cfg!(unix) {
            std::fs::set_permissions(
                &tmpfile,
                std::os::unix::fs::PermissionsExt::from_mode(0o755),
            )
            .ok();
            format!("{} {}", lang, tmpfile)
        } else {
            format!("{} {}", lang, tmpfile)
        };

        let shell = if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "sh"
        };
        let flag = if cfg!(target_os = "windows") {
            "/C"
        } else {
            "-c"
        };

        let result = Command::new(shell).arg(flag).arg(&cmd_str).output();

        match result {
            Ok(output) => {
                let mut out = String::new();
                if !output.stdout.is_empty() {
                    out.push_str(&String::from_utf8_lossy(&output.stdout));
                }
                if !output.stderr.is_empty() {
                    if !out.is_empty() {
                        out.push_str("\n--- stderr ---\n");
                    }
                    out.push_str(&String::from_utf8_lossy(&output.stderr));
                }
                if out.is_empty() {
                    out = String::from("(no output)");
                }
                self.output = out;
                self.status = format!("Exit: {:?}", output.status.code());
            }
            Err(e) => {
                self.output = format!("Execution error: {e}");
                self.status = String::from("Run failed");
            }
        }

        let _ = std::fs::remove_file(&tmpfile);
        self.mode = Mode::Output;
    }

    fn render(&mut self) -> Vec<Value> {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        self.cached_commands.clone()
    }
}

fn render_ui(app: &App) -> Vec<Value> {
    let t = &app.theme;
    let w = app.area.w.max(48);
    let h = app.area.h.max(14);
    let mut cmds: Vec<Value> = Vec::new();

    cmds.push(json!({
        String::from("type"): String::from("Rect"),
        String::from("x"): 0, String::from("y"): 0,
        String::from("w"): w, String::from("h"): h,
        String::from("bg"): t.background,
    }));
    cmds.push(json!({
        String::from("type"): String::from("Border"),
        String::from("x"): 0, String::from("y"): 0,
        String::from("w"): w, String::from("h"): h,
        String::from("fg"): t.border,
        String::from("borders"): BORDER_ALL,
        String::from("bg"): t.background_panel,
        String::from("title"): format!(" Code Runner ({}) ", app.language),
        String::from("title_fg"): t.text,
        String::from("title_dash_fg"): t.border,
    }));

    let label = match app.mode {
        Mode::Editing => " Editor ",
        Mode::Running => " Running… ",
        Mode::Output => " Output ",
    };

    let editor_h = h / 2;
    cmds.push(json!({
        String::from("type"): String::from("Border"),
        String::from("x"): 1, String::from("y"): 1,
        String::from("w"): w.saturating_sub(2), String::from("h"): editor_h,
        String::from("fg"): t.accent,
        String::from("borders"): BORDER_ALL,
        String::from("bg"): t.background,
        String::from("title"): String::from(label),
        String::from("title_fg"): t.text,
        String::from("title_dash_fg"): t.border,
    }));

    for (i, line) in app.code.iter().enumerate() {
        let y = 2 + i as u16;
        if y >= editor_h.saturating_sub(1) {
            break;
        }
        let is_cursor = i == app.cursor && app.mode == Mode::Editing;
        let prefix = if is_cursor { "▸ " } else { "  " };
        cmds.push(json!({
            String::from("type"): String::from("Text"),
            String::from("x"): 3, String::from("y"): y,
            String::from("text"): format!("{prefix}{line}"),
            String::from("fg"): if is_cursor { t.highlight } else { t.text },
            String::from("bold"): is_cursor,
            String::from("modifiers"): 0,
        }));
    }

    let output_y = editor_h + 1;
    let output_h = h.saturating_sub(output_y + 4);

    if app.mode == Mode::Output || !app.output.is_empty() {
        cmds.push(json!({
            String::from("type"): String::from("Border"),
            String::from("x"): 1, String::from("y"): output_y,
            String::from("w"): w.saturating_sub(2), String::from("h"): output_h + 2,
            String::from("fg"): t.border,
            String::from("borders"): BORDER_ALL,
            String::from("bg"): t.background,
            String::from("title"): String::from(" Output "),
            String::from("title_fg"): t.text,
            String::from("title_dash_fg"): t.border,
        }));

        for (i, line) in app.output.lines().enumerate() {
            let y = output_y + 1 + i as u16;
            if y >= output_y + output_h {
                break;
            }
            cmds.push(json!({
                String::from("type"): String::from("Text"),
                String::from("x"): 3, String::from("y"): y,
                String::from("text"): line.to_string(),
                String::from("fg"): t.text,
                String::from("bold"): false,
                String::from("modifiers"): 0,
            }));
        }
    }

    let status_y = h.saturating_sub(2);
    cmds.push(json!({
        String::from("type"): String::from("Text"),
        String::from("x"): 2, String::from("y"): status_y,
        String::from("text"): app.status.clone(),
        String::from("fg"): t.text_muted,
        String::from("bold"): false,
        String::from("modifiers"): 0,
    }));

    cmds
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
        ("tab".into(), "lang".into()),
        ("^R".into(), "run".into()),
        ("esc".into(), "back".into()),
    ]
}

fn palette_commands() -> Value {
    json!([[
        String::from("Development"),
        String::from("Open Code Runner")
    ]])
}

fn respond(app: &mut App, consumed: bool) {
    let commands_val = serde_json::to_value(app.render()).unwrap_or(Value::Null);
    let resp = json!({
        String::from("commands"): commands_val,
        String::from("hints"): hints(),
        String::from("palette_commands"): palette_commands(),
        String::from("request"): null,
        String::from("plugin_message"): null,
        String::from("consumed"): consumed,
    });
    if let Ok(json_str) = serde_json::to_string(&resp) {
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
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: Result<HostMsg, _> = serde_json::from_str(&line);
                match msg {
                    Ok(HostMsg::Init { theme, area, .. }) => {
                        app.theme = theme;
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::Resize { area }) => {
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::ThemeChange { theme }) => {
                        app.theme = theme;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::Key { key, modifiers }) => {
                        let consumed = app.handle_key(key, modifiers);
                        respond(&mut app, consumed);
                    }
                    Ok(HostMsg::Tick) => {
                        if app.mode == Mode::Running {
                            app.mode = Mode::Output;
                            app.dirty = true;
                        }
                        respond(&mut app, false);
                    }
                    Ok(HostMsg::PaletteCommand { .. }) => {
                        app.dirty = true;
                        respond(&mut app, true);
                    }
                    Ok(HostMsg::Shutdown) => break,
                    Ok(_) => {
                        respond(&mut app, false);
                    }
                    Err(e) => {
                        log::error!("[code-runner] parse error: {e}: {line}");
                        respond(&mut app, false);
                    }
                }
            }
        }
    }
}
