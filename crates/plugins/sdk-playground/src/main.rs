use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    endpoint: String,
    body: String,
    response: String,
    cursor: u16,
    editing_endpoint: bool,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            endpoint: String::new(),
            body: String::new(),
            response: String::new(),
            cursor: 0,
            editing_endpoint: true,
            status: String::from("Tab to switch fields, Enter to send"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.editing_endpoint = !self.editing_endpoint;
                self.cursor = 0;
                true
            }
            IpcKey::Enter => {
                if self.endpoint.is_empty() {
                    self.status = String::from("Enter an endpoint URL first");
                    true
                } else {
                    self.send_mock();
                    true
                }
            }
            IpcKey::Char(c) if !c.is_control() => {
                let field = if self.editing_endpoint {
                    &mut self.endpoint
                } else {
                    &mut self.body
                };
                let pos = self.cursor as usize;
                if pos <= field.len() {
                    field.insert(pos, c);
                    self.cursor += 1;
                }
                true
            }
            IpcKey::Backspace => {
                let field = if self.editing_endpoint {
                    &mut self.endpoint
                } else {
                    &mut self.body
                };
                if self.cursor > 0 {
                    let pos = self.cursor as usize - 1;
                    if pos < field.len() {
                        field.remove(pos);
                    }
                    self.cursor -= 1;
                }
                true
            }
            IpcKey::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Right => {
                let max = if self.editing_endpoint {
                    self.endpoint.len()
                } else {
                    self.body.len()
                } as u16;
                if self.cursor < max {
                    self.cursor += 1;
                }
                true
            }
            IpcKey::Home => {
                self.cursor = 0;
                true
            }
            IpcKey::End => {
                self.cursor = if self.editing_endpoint {
                    self.endpoint.len()
                } else {
                    self.body.len()
                } as u16;
                true
            }
            IpcKey::Esc => false,
            _ => true,
        }
    }

    fn send_mock(&mut self) {
        let ep = self.endpoint.clone();
        let b = self.body.clone();
        let body_preview = if b.len() > 60 {
            format!("{}...", &b[..60])
        } else {
            b.clone()
        };
        self.response = format!(
            "Mock Response for: {}\n\nStatus: 200 OK\n\nBody sent: {}\n\nResponse:\n{{\n  \"success\": true,\n  \"data\": {{\n    \"id\": 42,\n    \"message\": \"Hello from the API playground\"\n  }}\n}}",
            ep, body_preview
        );
        self.status = String::from("Response received (mock)");
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(42);
    let h = app.area.h.max(14);

    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: t.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: t.border,
        borders: BORDER_ALL,
        bg: Some(t.background_panel),
        title: Some(String::from(" SDK Playground ")),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let input_fg = if app.editing_endpoint {
        t.highlight
    } else {
        t.text
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!("Endpoint: {}", app.endpoint),
        fg: Some(input_fg),
        bg: None,
        bold: app.editing_endpoint,
        modifiers: 0,
    });

    let body_fg = if app.editing_endpoint {
        t.text
    } else {
        t.highlight
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: format!("Body:     {}", app.body),
        fg: Some(body_fg),
        bg: None,
        bold: !app.editing_endpoint,
        modifiers: 0,
    });

    if !app.response.is_empty() {
        let resp_h = (h.saturating_sub(7)).max(3);
        let resp_lines: Vec<&str> = app.response.lines().collect();
        let max_lines = resp_h as usize;
        for (i, line) in resp_lines.iter().enumerate().take(max_lines) {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 5 + i as u16,
                text: line.to_string(),
                fg: Some(t.text),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
    }

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: app.status.clone(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text: String::from("\u{2190}\u{2192} move \u{b7} tab field \u{b7} enter send \u{b7} esc"),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

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

fn palette_commands() -> serde_json::Value {
    serde_json::json!([("SDK".to_string(), "Open SDK Playground".to_string())])
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
                log::error!("[sdk-playground] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
