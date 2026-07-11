use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

const PROVIDERS: &[&str] = &["AWS", "Azure", "GCP"];
const SERVICES: &[&str] = &[
    "Compute (EC2/VM/GCE)",
    "Storage (S3/Blob/GCS)",
    "Database (RDS/Cosmos/CloudSQL)",
    "Serverless (Lambda/Functions/CloudFunctions)",
];
const RATES: &[&[f64]] = &[
    &[0.10, 0.023, 0.15, 0.00002],
    &[0.12, 0.025, 0.18, 0.000025],
    &[0.09, 0.020, 0.13, 0.000018],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Provider,
    Service,
    Hours,
    Rate,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    provider: usize,
    service: usize,
    hours: String,
    rate_input: String,
    focus: Focus,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            provider: 0,
            service: 0,
            hours: String::from("720"),
            rate_input: String::new(),
            focus: Focus::Provider,
            status: String::from("Tab: cycle · \u{2190}\u{2192}: change option · Esc: close"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::Provider => Focus::Service,
                    Focus::Service => Focus::Hours,
                    Focus::Hours => Focus::Rate,
                    Focus::Rate => Focus::Provider,
                };
                true
            }
            IpcKey::Left => {
                match self.focus {
                    Focus::Provider => {
                        self.provider = self.provider.saturating_sub(1);
                    }
                    Focus::Service => {
                        self.service = self.service.saturating_sub(1);
                    }
                    _ => {}
                }
                true
            }
            IpcKey::Right => {
                match self.focus {
                    Focus::Provider => {
                        self.provider = (self.provider + 1).min(PROVIDERS.len() - 1);
                    }
                    Focus::Service => {
                        self.service = (self.service + 1).min(SERVICES.len() - 1);
                    }
                    _ => {}
                }
                true
            }
            IpcKey::Backspace => {
                match self.focus {
                    Focus::Hours => {
                        self.hours.pop();
                    }
                    Focus::Rate => {
                        self.rate_input.pop();
                    }
                    _ => {}
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                match self.focus {
                    Focus::Hours => {
                        if c.is_ascii_digit() || c == '.' {
                            self.hours.push(c);
                        }
                    }
                    Focus::Rate if c.is_ascii_digit() || c == '.' => {
                        self.rate_input.push(c);
                    }
                    _ => {}
                }
                true
            }
            _ => true,
        }
    }

    fn estimated_cost(&self) -> f64 {
        let hours = self.hours.parse::<f64>().unwrap_or(0.0);
        let rate = if self.rate_input.is_empty() {
            RATES[self.provider][self.service]
        } else {
            self.rate_input.parse::<f64>().unwrap_or(0.0)
        };
        hours * rate
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(40);
        let h = self.area.h.max(14);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": String::from(" Cloud Cost Estimator "),
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let provider_str = PROVIDERS
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if i == self.provider {
                    format!("[{}]", p)
                } else {
                    format!(" {} ", p)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        cmds.push(json!({"Text": {
            "x": 2, "y": 1, "text": format!("Provider: {}", provider_str),
            "fg": if self.focus == Focus::Provider { t.accent } else { t.text },
            "bg": null, "bold": self.focus == Focus::Provider, "modifiers": 0,
        }}));

        let service_str = SERVICES
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == self.service {
                    format!("[{}]", s)
                } else {
                    format!(" {} ", s)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        cmds.push(json!({"Text": {
            "x": 2, "y": 3, "text": format!("Service:\n{}", service_str),
            "fg": if self.focus == Focus::Service { t.accent } else { t.text },
            "bg": null, "bold": self.focus == Focus::Service, "modifiers": 0,
        }}));

        let default_rate = RATES[self.provider][self.service];
        let rate_display = if self.rate_input.is_empty() {
            format!("${:.4}/hr (default)", default_rate)
        } else {
            format!("${}/hr", self.rate_input)
        };
        cmds.push(json!({"Text": {
            "x": 2, "y": 8, "text": format!("Hours: {}", self.hours),
            "fg": if self.focus == Focus::Hours { t.accent } else { t.text },
            "bg": null, "bold": self.focus == Focus::Hours, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 9, "text": format!("Rate: {}", rate_display),
            "fg": if self.focus == Focus::Rate { t.accent } else { t.text },
            "bg": null, "bold": self.focus == Focus::Rate, "modifiers": 0,
        }}));

        let cost = self.estimated_cost();
        cmds.push(json!({"Text": {
            "x": 2, "y": 11, "text": format!("Estimated cost: ${:.2}", cost),
            "fg": t.success, "bg": null, "bold": true, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": 12, "text": format!("Monthly (720h): ${:.2}", cost.max(0.0)),
            "fg": t.text, "bg": null, "bold": false, "modifiers": 0,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
            "text": String::from("Tab: cycle focus · \u{2190}\u{2192}: change option · type: edit values · Esc: close"),
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

fn palette_commands() -> Value {
    json!([["Plugins", "Cloud Cost"]])
}

fn key_hints() -> Value {
    json!([["Tab", "cycle focus"],])
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = json!({
        "commands": commands_val, "hints": key_hints(), "palette_commands": palette_commands(),
        "request": null, "plugin_message": null, "consumed": consumed,
    });
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json}");
    let _ = out.flush();
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
                log::error!("[cloud-cost] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
