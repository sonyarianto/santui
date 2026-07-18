use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    endpoint: String,
    query: String,
    cursor_pos: usize,
    query_cursor: usize,
    focus: Focus,
    response: String,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Endpoint,
    Query,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            endpoint: String::new(),
            query: String::new(),
            cursor_pos: 0,
            query_cursor: 0,
            focus: Focus::Endpoint,
            response: String::new(),
            status: "Enter GraphQL endpoint and query, press Enter to send".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Char('\n') | IpcKey::Char('\r') => {
                self.send_query();
                true
            }
            IpcKey::Char('\t') => {
                self.focus = match self.focus {
                    Focus::Endpoint => Focus::Query,
                    Focus::Query => Focus::Endpoint,
                };
                true
            }
            IpcKey::Char(c) if !modifiers.ctrl => {
                let buf = match self.focus {
                    Focus::Endpoint => &mut self.endpoint,
                    Focus::Query => &mut self.query,
                };
                let pos = match self.focus {
                    Focus::Endpoint => &mut self.cursor_pos,
                    Focus::Query => &mut self.query_cursor,
                };
                if c == '\u{7f}' || c == '\x08' {
                    if *pos > 0 {
                        buf.remove(*pos - 1);
                        *pos -= 1;
                    }
                } else {
                    buf.insert(*pos, c);
                    *pos += 1;
                }
                true
            }
            IpcKey::Left => {
                match self.focus {
                    Focus::Endpoint => self.cursor_pos = self.cursor_pos.saturating_sub(1),
                    Focus::Query => self.query_cursor = self.query_cursor.saturating_sub(1),
                }
                true
            }
            IpcKey::Right => {
                match self.focus {
                    Focus::Endpoint => {
                        if self.cursor_pos < self.endpoint.len() {
                            self.cursor_pos += 1;
                        }
                    }
                    Focus::Query => {
                        if self.query_cursor < self.query.len() {
                            self.query_cursor += 1;
                        }
                    }
                }
                true
            }
            IpcKey::Home => {
                self.cursor_pos = 0;
                self.query_cursor = 0;
                true
            }
            IpcKey::End => {
                self.cursor_pos = self.endpoint.len();
                self.query_cursor = self.query.len();
                true
            }
            IpcKey::Backspace => {
                match self.focus {
                    Focus::Endpoint => {
                        if self.cursor_pos > 0 {
                            self.endpoint.remove(self.cursor_pos - 1);
                            self.cursor_pos -= 1;
                        }
                    }
                    Focus::Query => {
                        if self.query_cursor > 0 {
                            self.query.remove(self.query_cursor - 1);
                            self.query_cursor -= 1;
                        }
                    }
                }
                true
            }
            IpcKey::Delete => {
                match self.focus {
                    Focus::Endpoint => {
                        if self.cursor_pos < self.endpoint.len() {
                            self.endpoint.remove(self.cursor_pos);
                        }
                    }
                    Focus::Query => {
                        if self.query_cursor < self.query.len() {
                            self.query.remove(self.query_cursor);
                        }
                    }
                }
                true
            }
            _ => true,
        }
    }

    fn send_query(&mut self) {
        let url = self.endpoint.trim();
        let q = self.query.trim();
        if url.is_empty() || q.is_empty() {
            return;
        }
        self.status = format!("Sending query to {url}...");
        let body = json!({"query": q}).to_string();
        match ureq::post(url)
            .header("Content-Type", "application/json")
            .send(body.as_bytes())
        {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.into_body().read_to_string().unwrap_or_default();
                let lines: Vec<&str> = text.lines().collect();
                let preview: String = lines
                    .iter()
                    .take(50)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                self.response = preview;
                self.status = format!("Status: {status}  {} chars", text.len());
            }
            Err(e) => {
                self.response = format!("Request failed: {e}");
                self.status = "Request error".into();
            }
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(56);
        let h = self.area.h.max(14);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": " GraphQL Playground ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let ep_label = if self.focus == Focus::Endpoint {
            "> Endpoint:"
        } else {
            "  Endpoint:"
        };
        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": ep_label, "fg": if self.focus == Focus::Endpoint { t.accent } else { t.text_muted },
            "bg": null, "bold": true, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 2,
            "text": if self.endpoint.is_empty() { String::from("https://api.example.com/graphql") } else { self.endpoint.clone() },
            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
        }}));

        let q_label = if self.focus == Focus::Query {
            "> Query:"
        } else {
            "  Query:"
        };
        cmds.push(json!({"Text": {
            "x": 2, "y": 3, "text": q_label, "fg": if self.focus == Focus::Query { t.accent } else { t.text_muted },
            "bg": null, "bold": true, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 4,
            "text": if self.query.is_empty() { String::from("{ users { id name } }") } else { self.query.clone() },
            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
        }}));

        let box_y = 6u16;
        let box_w = w.saturating_sub(4);
        let box_h = h.saturating_sub(8).max(4);

        cmds.push(json!({"Border": {
            "x": 2, "y": box_y, "w": box_w, "h": box_h, "fg": t.accent,
            "borders": BORDER_ALL, "bg": t.background,
            "title": " Response ", "title_fg": t.accent,
            "title_dash_fg": t.border, "border_type": null,
        }}));

        if self.response.is_empty() {
            cmds.push(json!({"Text": {
                "x": 4, "y": box_y + 1,
                "text": String::from("Press Enter to send query"),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
            }}));
        } else {
            for (i, line) in self
                .response
                .lines()
                .enumerate()
                .take(box_h.saturating_sub(2) as usize)
            {
                cmds.push(json!({"Text": {
                    "x": 4, "y": box_y + 1 + i as u16,
                    "text": line.to_string(),
                    "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
                }}));
            }
        }

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
    }
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

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("esc".into(), "close".into()),
        ("enter".into(), "send query".into()),
        ("tab".into(), "switch field".into()),
    ]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app
            .render()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect(),
        hints: key_hints(),
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
    let mut app = App::default();
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
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
            Ok(HostMsg::Key { key, modifiers }) => app.handle_key(key, modifiers),
            Ok(HostMsg::PaletteCommand { .. }) => {
                app.dirty = true;
                true
            }
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Tick
                | HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[graphql-playground] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
