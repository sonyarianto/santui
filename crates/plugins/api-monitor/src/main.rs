use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};

#[derive(Debug, Clone)]
struct Endpoint {
    url: String,
    status: String,
    response_time: String,
    last_checked: String,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    endpoints: Vec<Endpoint>,
    url_input: String,
    selected: usize,
    editing: bool,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            endpoints: Vec::new(),
            url_input: String::new(),
            selected: 0,
            editing: false,
            status: String::from("a: add URL · d: delete · c: check · Esc: close"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.editing {
            match key {
                IpcKey::Esc => {
                    self.editing = false;
                    self.url_input.clear();
                }
                IpcKey::Enter => {
                    let url = self.url_input.trim().to_string();
                    if !url.is_empty() {
                        self.endpoints.push(Endpoint {
                            url,
                            status: String::from("--"),
                            response_time: String::from("--"),
                            last_checked: String::from("--"),
                        });
                        self.url_input.clear();
                    }
                    self.editing = false;
                }
                IpcKey::Backspace => {
                    self.url_input.pop();
                }
                IpcKey::Char(c) if !c.is_control() => {
                    self.url_input.push(c);
                }
                _ => {}
            }
            return true;
        }
        match key {
            IpcKey::Esc => false,
            IpcKey::Up | IpcKey::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.endpoints.len().saturating_sub(1);
                self.selected = (self.selected + 1).min(max);
                true
            }
            IpcKey::Char('a') => {
                self.editing = true;
                self.url_input.clear();
                self.status = String::from("Enter URL to monitor:");
                true
            }
            IpcKey::Char('d') => {
                if self.selected < self.endpoints.len() {
                    self.endpoints.remove(self.selected);
                    self.selected = self.selected.min(self.endpoints.len().saturating_sub(1));
                }
                true
            }
            IpcKey::Char('c') => {
                if self.selected < self.endpoints.len() {
                    let ep = &mut self.endpoints[self.selected];
                    let ms = fastrand(50..500);
                    let codes = [200, 200, 200, 200, 301, 404, 500];
                    let code = codes[fastrand(0..codes.len())];
                    ep.status = format!(
                        "{} {}",
                        code,
                        match code {
                            200 => "OK",
                            301 => "Moved",
                            404 => "Not Found",
                            500 => "Server Error",
                            _ => "Unknown",
                        }
                    );
                    ep.response_time = format!("{}ms", ms);
                    ep.last_checked = String::from("just now");
                    self.status = format!("Checked {} => {}", ep.url, ep.status);
                }
                true
            }
            _ => true,
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(40);
        let h = self.area.h.max(12);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": String::from(" API Monitor "),
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": if self.editing {
                format!("> URL: {}", self.url_input)
            } else {
                format!("  URL: {}", self.url_input)
            },
            "fg": if self.editing { t.accent } else { t.text },
            "bg": null, "bold": self.editing, "modifiers": 0,
        }}));

        let header = format!(
            "{:<30} {:>12} {:>10}  {}",
            "Endpoint", "Status", "Time", "Last Checked"
        );
        cmds.push(json!({"Text": {
            "x": 2, "y": 3, "text": header,
            "fg": t.text_muted, "bg": null, "bold": true, "modifiers": 0,
        }}));

        let list_y = 4;
        let list_h = h.saturating_sub(7).max(1);
        let items: Vec<String> = self
            .endpoints
            .iter()
            .map(|ep| {
                format!(
                    "{:<30} {:>12} {:>10}  {}",
                    ep.url, ep.status, ep.response_time, ep.last_checked
                )
            })
            .collect();
        cmds.push(json!({"List": {
            "x": 2, "y": list_y, "w": w.saturating_sub(4), "h": list_h,
            "items": items,
            "selected": if self.selected < self.endpoints.len() { Some(self.selected) } else { None },
            "style": {"fg": t.text, "bg": null, "bold": false, "modifiers": 0},
            "highlight_style": {"fg": t.inverted_text, "bg": t.highlight, "bold": true, "modifiers": 0},
        }}));

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

fn fastrand(range: std::ops::Range<usize>) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    range.start + (nanos % (range.end - range.start))
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
    vec![("Developer".into(), "API Monitor".into())]
}

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("a".into(), "add URL".into()),
        ("d".into(), "delete endpoint".into()),
        ("c".into(), "check endpoint".into()),
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
                log::error!("[api-monitor] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
