use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use ureq::config::Config;

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
    model: String,
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
            model: String::from("gpt-4o-mini"),
            fetching: false,
            rx: None,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => {
                if self.fetching {
                    self.fetching = false;
                    self.rx = None;
                    self.status = String::from("Cancelled");
                    return true;
                }
                if !self.input_buffer.is_empty() {
                    self.input_buffer.clear();
                    return true;
                }
                false
            }
            IpcKey::Enter => {
                if self.fetching {
                    return true;
                }
                let input = self.input_buffer.trim().to_string();
                if input.is_empty() {
                    return true;
                }
                if self.api_key.is_empty() {
                    self.status = String::from("Set OPENAI_API_KEY env var and restart the plugin");
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
        let model = self.model.clone();
        let msgs: Vec<Value> = self
            .messages
            .iter()
            .map(|m| {
                json!({
                    "role": m.role.clone(),
                    "content": m.content.clone(),
                })
            })
            .collect();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let agent = ureq::Agent::new_with_config(
                Config::builder()
                    .timeout_global(Some(Duration::from_secs(120)))
                    .build(),
            );
            let body = json!({
                "model": model,
                "messages": msgs,
                "max_tokens": 4096,
            });
            let result = match agent
                .post(&api_url)
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Content-Type", "application/json")
                .send_json(&body)
            {
                Ok(mut resp) => {
                    let body_str = resp.body_mut().read_to_string().unwrap_or_default();
                    match serde_json::from_str::<Value>(&body_str) {
                        Ok(v) => {
                            let content = v["choices"][0]["message"]["content"]
                                .as_str()
                                .unwrap_or("No response")
                                .to_string();
                            Ok(content)
                        }
                        Err(e) => Err(format!("Parse error: {e}")),
                    }
                }
                Err(e) => Err(format!("API error: {e}")),
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
            match rx.try_recv() {
                Ok(msg) => {
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
                Err(TryRecvError::Disconnected) => {
                    self.fetching = false;
                    self.status = String::from("Request failed unexpectedly");
                    self.dirty = true;
                }
                Err(TryRecvError::Empty) => {}
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

    cmds.push(json!({"Rect": {
        "x": 0, "y": 0, "w": w, "h": h, "bg": t.background,
    }}));
    cmds.push(json!({"Border": {
        "x": 0, "y": 0, "w": w, "h": h,
        "fg": t.border, "borders": BORDER_ALL,
        "bg": t.background_panel,
        "title": " AI Chat ", "title_fg": t.text, "title_dash_fg": t.border,
        "border_type": null,
    }}));

    if app.api_key.is_empty() {
        cmds.push(json!({"Text": {
            "x": 2, "y": 4,
            "text": "OPENAI_API_KEY not set",
            "fg": t.error, "bg": null, "bold": true, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 5,
            "text": "Set the environment variable and restart santui",
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        return cmds;
    }

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
        cmds.push(json!({"Text": {
            "x": 2, "y": line_y,
            "text": format!("{prefix}{}", msg.content),
            "fg": fg, "bg": null, "bold": false, "modifiers": 0,
        }}));
    }

    let input_y = h.saturating_sub(3);
    cmds.push(json!({"Border": {
        "x": 1, "y": input_y,
        "w": w.saturating_sub(2), "h": 3,
        "fg": t.border, "borders": BORDER_ALL,
        "bg": t.background,
        "title": null, "title_fg": null, "title_dash_fg": null,
        "border_type": null,
    }}));
    cmds.push(json!({"Text": {
        "x": 3, "y": input_y + 1,
        "text": app.input_buffer.clone(),
        "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
    }}));

    let status_y = h.saturating_sub(1);
    cmds.push(json!({"Text": {
        "x": 2, "y": status_y,
        "text": app.status.clone(),
        "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
    }}));

    cmds.push(json!({"Text": {
        "x": 2, "y": status_y - 1,
        "text": "Enter send  ·  esc cancel  ·  ↑↓ scroll",
        "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
    }}));

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

fn palette_commands() -> Vec<(String, String)> {
    vec![]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app
            .render()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect(),
        hints: vec![],
        palette_commands: palette_commands(),
        request: None,
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

    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
    let api_url = std::env::var("OPENAI_BASE_URL")
        .map(|u| format!("{}/chat/completions", u.trim_end_matches('/')))
        .unwrap_or_else(|_| String::from("https://api.openai.com/v1/chat/completions"));
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| String::from("gpt-4o-mini"));

    let status = if api_key.is_empty() {
        String::from("OPENAI_API_KEY not set — see status bar")
    } else {
        String::from("Type message, Enter to send")
    };
    let mut app = App {
        api_key,
        api_url,
        model,
        status,
        ..Default::default()
    };

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
