use rand::Rng;
use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

struct Stock {
    symbol: String,
    name: String,
    price: f64,
    change: f64,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    stocks: Vec<Stock>,
    selected: usize,
    tick_count: u32,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            stocks: vec![
                Stock {
                    symbol: "AAPL".into(),
                    name: "Apple Inc.".into(),
                    price: 198.50,
                    change: 1.20,
                },
                Stock {
                    symbol: "GOOGL".into(),
                    name: "Alphabet Inc.".into(),
                    price: 175.30,
                    change: -0.80,
                },
                Stock {
                    symbol: "MSFT".into(),
                    name: "Microsoft Corp.".into(),
                    price: 425.10,
                    change: 2.50,
                },
                Stock {
                    symbol: "AMZN".into(),
                    name: "Amazon.com Inc.".into(),
                    price: 185.60,
                    change: -1.30,
                },
                Stock {
                    symbol: "TSLA".into(),
                    name: "Tesla Inc.".into(),
                    price: 245.80,
                    change: 5.40,
                },
                Stock {
                    symbol: "NVDA".into(),
                    name: "NVIDIA Corp.".into(),
                    price: 880.20,
                    change: 15.30,
                },
                Stock {
                    symbol: "META".into(),
                    name: "Meta Platforms".into(),
                    price: 505.40,
                    change: -2.10,
                },
                Stock {
                    symbol: "BRK.B".into(),
                    name: "Berkshire Hathaway".into(),
                    price: 415.90,
                    change: 0.60,
                },
            ],
            selected: 0,
            tick_count: 0,
            status: "Simulated prices \u{b7} \u{2191}\u{2193} select \u{b7} r randomize \u{b7} esc"
                .into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            IpcKey::Up => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            IpcKey::Down => {
                self.selected = (self.selected + 1).min(self.stocks.len().saturating_sub(1));
                true
            }
            IpcKey::Char('r') if !modifiers.ctrl => {
                self.randomize();
                true
            }
            _ => true,
        }
    }

    fn randomize(&mut self) {
        let mut rng = rand::thread_rng();
        for stock in &mut self.stocks {
            let delta = rng.gen_range(-5.0..5.0);
            stock.price = (stock.price + delta * 0.1).max(1.0);
            stock.change = delta.round() / 10.0;
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(46);
        let h = self.area.h.max(14);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": " Stock Ticker ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": 1,
            "text": format!("{:<8} {:<20} {:>10} {:>8}", "Symbol", "Name", "Price", "Change"),
            "fg": t.text_muted, "bg": null, "bold": true, "modifiers": 0,
        }}));

        let content_y = 3u16;
        let max_rows = h.saturating_sub(5) as usize;

        for (i, stock) in self.stocks.iter().enumerate().take(max_rows) {
            let is_selected = i == self.selected;
            let bg = if is_selected { Some(t.highlight) } else { None };
            let line = format!(
                "{:<8} {:<20} {:>10.2} {:>+8.2}",
                stock.symbol, stock.name, stock.price, stock.change,
            );
            cmds.push(json!({"Text": {
                "x": 2, "y": content_y + i as u16,
                "text": line,
                "fg": if stock.change >= 0.0 { t.success } else { t.error },
                "bg": bg, "bold": is_selected, "modifiers": 0,
            }}));
        }

        let selected = &self.stocks[self.selected];
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": format!("Selected: {} - {} @ ${:.2}", selected.symbol, selected.name, selected.price),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));
        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(1),
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

fn palette_commands() -> Value {
    json!([
        {"key": "esc", "hint": "close"},
        {"key": "up", "hint": "select up"},
        {"key": "down", "hint": "select down"},
        {"key": "r", "hint": "randomize"},
    ])
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = json!({
        "commands": commands_val, "hints": [], "palette_commands": palette_commands(),
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
            Ok(HostMsg::Tick) => {
                app.tick_count += 1;
                if app.tick_count % 5 == 0 {
                    app.randomize();
                }
                app.dirty = true;
                false
            }
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[stock-ticker] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
