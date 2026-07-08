use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

enum FetchMsg {
    Response(String),
    Error(String),
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    messages: Vec<ChatMessage>,
    input_buffer: String,
    scroll: usize,
    status: String,
    api_url: String,
    api_key: String,
    fetching: bool,
    rx: Option<mpsc::Receiver<FetchMsg>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            messages: vec![ChatMessage {
                role: String::from("system"),
                content: String::from("You are a helpful assistant."),
            }],
            input_buffer: String::new(),
            scroll: 0,
            status: String::from("Type message, Enter to send"),
            api_url: String::from("https://api.openai.com/v1/chat/completions"),
            api_key: String::new(),
            fetching: false,
            rx: None,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Enter => {
                if self.fetching {
                    return true;
                }
                let input = self.input_buffer.trim().to_string();
                if input.is_empty() {
                    return true;
                }
                self.messages.push(ChatMessage {
                    role: String::from("user"),
                    content: input,
                });
                self.input_buffer.clear();
                self.status = String::from("Thinking...");
                self.fetching = true;
                self.trigger_request();
                true
            }
            IpcKey::Backspace => {
                self.input_buffer.pop();
                true
            }
            IpcKey::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                true
            }
            IpcKey::Down => {
                let max_lines: usize = self
                    .messages
                    .iter()
                    .map(|m| m.content.lines().count())
                    .sum();
                let visible = self.area.h.saturating_sub(6) as usize;
                let max_scroll = max_lines.saturating_sub(visible);
                self.scroll = self.scroll.saturating_add(1).min(max_scroll);
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.input_buffer.push(c);
                true
            }
            _ => true,
        }
    }

    fn trigger_request(&mut self) {
        let api_url = self.api_url.clone();
        let api_key = self.api_key.clone();
        let msgs: Vec<Value> = self
            .messages
            .iter()
            .map(|m| {
                json!({
                    String::from("role"): m.role.clone(),
                    String::from("content"): m.content.clone(),
                })
            })
            .collect();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let body = json!({
                String::from("model"): String::from("gpt-4o-mini"),
                String::from("messages"): msgs,
                String::from("max_tokens"): 1024,
            });
            let result = if api_key.is_empty() {
                Err(String::from("API key not set"))
            } else {
                match ureq::post(&api_url)
                    .header(String::from("Authorization"), format!("Bearer {api_key}"))
                    .header(
                        String::from("Content-Type"),
                        String::from("application/json"),
                    )
                    .send_json(&body)
                {
                    Ok(mut resp) => {
                        let body_str = resp.body_mut().read_to_string().unwrap_or_default();
                        match serde_json::from_str::<Value>(&body_str) {
                            Ok(v) => {
                                let content = v[String::from("choices")][0]
                                    [String::from("message")][String::from("content")]
                                .as_str()
                                .unwrap_or("No response")
                                .to_string();
                                Ok(content)
                            }
                            Err(e) => Err(format!("Parse error: {e}")),
                        }
                    }
                    Err(e) => Err(format!("API error: {e}")),
                }
            };
            match result {
                Ok(content) => {
                    let _ = tx.send(FetchMsg::Response(content));
                }
                Err(e) => {
                    let _ = tx.send(FetchMsg::Error(e));
                }
            }
        });
    }

    fn handle_tick(&mut self) {
        if let Some(ref rx) = self.rx {
            if let Ok(msg) = rx.try_recv() {
                self.fetching = false;
                match msg {
                    FetchMsg::Response(content) => {
                        self.messages.push(ChatMessage {
                            role: String::from("assistant"),
                            content,
                        });
                        self.status = String::from("Response received");
                    }
                    FetchMsg::Error(e) => {
                        self.status = format!("Error: {e}");
                    }
                }
                self.dirty = true;
            }
        }
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
        String::from("title"): String::from(" AI Chat "),
        String::from("title_fg"): t.text,
        String::from("title_dash_fg"): t.border,
    }));

    let content_y = 2u16;
    let content_h = h.saturating_sub(6);
    let msg_start = app.messages.len().saturating_sub(content_h as usize);
    for (line_y, msg) in (content_y..)
        .zip(app.messages.iter().skip(msg_start.max(1)))
        .take(content_h as usize)
    {
        let prefix = match msg.role.as_str() {
            "user" => "You: ",
            "assistant" => "AI:  ",
            _ => "",
        };
        let fg = if msg.role == "user" {
            t.highlight
        } else {
            t.accent
        };
        cmds.push(json!({
            String::from("type"): String::from("Text"),
            String::from("x"): 2, String::from("y"): line_y,
            String::from("text"): format!("{prefix}{}", msg.content),
            String::from("fg"): fg,
            String::from("bold"): false,
            String::from("modifiers"): 0,
        }));
    }

    let input_y = h.saturating_sub(3);
    cmds.push(json!({
        String::from("type"): String::from("Border"),
        String::from("x"): 1, String::from("y"): input_y,
        String::from("w"): w.saturating_sub(2), String::from("h"): 3,
        String::from("fg"): t.border,
        String::from("borders"): BORDER_ALL,
        String::from("bg"): t.background,
    }));
    cmds.push(json!({
        String::from("type"): String::from("Text"),
        String::from("x"): 3, String::from("y"): input_y + 1,
        String::from("text"): app.input_buffer.clone(),
        String::from("fg"): t.text,
        String::from("bold"): false,
        String::from("modifiers"): 0,
    }));

    let status_y = h.saturating_sub(1);
    cmds.push(json!({
        String::from("type"): String::from("Text"),
        String::from("x"): 2, String::from("y"): status_y,
        String::from("text"): app.status.clone(),
        String::from("fg"): t.text_muted,
        String::from("bold"): false,
        String::from("modifiers"): 0,
    }));

    cmds.push(json!({
        String::from("type"): String::from("Text"),
        String::from("x"): 2, String::from("y"): status_y - 1,
        String::from("text"): String::from("Enter send  ·  ↑↓ scroll  ·  esc back"),
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

fn palette_commands() -> Value {
    json!([[String::from("AI & Data"), String::from("Open AI Chat")]])
}

fn respond(app: &mut App, consumed: bool) {
    let commands_val = serde_json::to_value(app.render()).unwrap_or(Value::Null);
    let resp = json!({
        String::from("commands"): commands_val,
        String::from("hints"): [],
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
                        app.handle_tick();
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
                        log::error!("[ai-chat] parse error: {e}: {line}");
                        respond(&mut app, false);
                    }
                }
            }
        }
    }
}
