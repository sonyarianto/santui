use std::collections::HashMap;
use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone, Default)]
struct IfaceStats {
    rx_bytes: u64,
    tx_bytes: u64,
    rx_speed: f64,
    tx_speed: f64,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    interfaces: Vec<String>,
    prev: HashMap<String, IfaceStats>,
    cur: HashMap<String, IfaceStats>,
    max_speed: f64,
    ticks: u64,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            interfaces: Vec::new(),
            prev: HashMap::new(),
            cur: HashMap::new(),
            max_speed: 1.0,
            ticks: 0,
        };
        app.sample();
        std::mem::swap(&mut app.prev, &mut app.cur);
        app
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        !matches!(key, IpcKey::Esc)
    }

    fn sample(&mut self) {
        let content = match std::fs::read_to_string("/proc/net/dev") {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut new_interfaces: Vec<String> = Vec::new();
        let mut new_cur: HashMap<String, IfaceStats> = HashMap::new();
        let mut max_speed: f64 = 1.0;

        for line in content.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 10 {
                continue;
            }
            let name = parts[0].trim_end_matches(':');
            if name == "lo" {
                continue;
            }

            let rx_bytes: u64 = parts[1].parse().unwrap_or(0);
            let tx_bytes: u64 = parts[9].parse().unwrap_or(0);

            let rx_speed = if let Some(prev) = self.prev.get(name) {
                let dt = 1;
                let drx = rx_bytes.saturating_sub(prev.rx_bytes);
                (drx as f64) / dt as f64
            } else {
                0.0
            };

            let tx_speed = if let Some(prev) = self.prev.get(name) {
                let dt = 1;
                let dtx = tx_bytes.saturating_sub(prev.tx_bytes);
                (dtx as f64) / dt as f64
            } else {
                0.0
            };

            if rx_speed > max_speed {
                max_speed = rx_speed;
            }
            if tx_speed > max_speed {
                max_speed = tx_speed;
            }

            new_cur.insert(
                name.to_string(),
                IfaceStats {
                    rx_bytes,
                    tx_bytes,
                    rx_speed,
                    tx_speed,
                },
            );
            new_interfaces.push(name.to_string());
        }

        if max_speed > 0.0 {
            self.max_speed = max_speed;
        }
        self.interfaces = new_interfaces;
        self.cur = new_cur;
        self.ticks += 1;
    }

    fn handle_tick(&mut self) {
        self.prev = self.cur.clone();
        self.sample();
        self.dirty = true;
    }

    fn format_speed(speed: f64) -> String {
        if speed >= 1_000_000.0 {
            format!("{:.1} MB/s", speed / 1_000_000.0)
        } else if speed >= 1_000.0 {
            format!("{:.1} KB/s", speed / 1_000.0)
        } else {
            format!("{:.0} B/s", speed)
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

fn bar(ratio: f64, width: u16) -> String {
    let full = '█' as u32;
    let parts = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let w = width as usize;
    let mut s = String::with_capacity(w);
    let scaled = ratio * (w * 8) as f64;
    let full_chars = (scaled / 8.0) as usize;
    let remainder = (scaled as usize) % 8;
    for _ in 0..full_chars.min(w) {
        s.push(char::from_u32(full).unwrap());
    }
    if full_chars < w && remainder > 0 {
        s.push(parts[remainder]);
    }
    while s.len() < w {
        s.push(' ');
    }
    s
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
        String::from("title"): String::from(" Network Bandwidth Monitor "),
        String::from("title_fg"): t.text,
        String::from("title_dash_fg"): t.border,
    }));

    let header = format!(
        "{:<12}  {:>12}  {:>12}  {:>12}  {:>12}",
        "Interface", "Down speed", "Up speed", "Down bar", "Up bar"
    );
    cmds.push(json!({
        String::from("type"): String::from("Text"),
        String::from("x"): 2, String::from("y"): 1,
        String::from("text"): header,
        String::from("fg"): t.accent,
        String::from("bold"): true,
        String::from("modifiers"): 0,
    }));

    let bar_w = ((w as usize).saturating_sub(56) / 2).max(5) as u16;
    let max_speed = app.max_speed.max(1.0);
    let mut y = 3u16;

    for iface in &app.interfaces {
        if y >= h.saturating_sub(3) {
            break;
        }
        if let Some(cur) = app.cur.get(iface) {
            let down_str = App::format_speed(cur.rx_speed);
            let up_str = App::format_speed(cur.tx_speed);
            let down_ratio = (cur.rx_speed / max_speed).min(1.0);
            let up_ratio = (cur.tx_speed / max_speed).min(1.0);
            let down_bar = bar(down_ratio, bar_w);
            let up_bar = bar(up_ratio, bar_w);

            let line = format!(
                "{:<12}  {:>12}  {:>12}  {:<bar_w$}  {:<bar_w$}",
                iface,
                down_str,
                up_str,
                down_bar,
                up_bar,
                bar_w = bar_w as usize,
            );

            cmds.push(json!({
                String::from("type"): String::from("Text"),
                String::from("x"): 2, String::from("y"): y,
                String::from("text"): line,
                String::from("fg"): t.text,
                String::from("bold"): false,
                String::from("modifiers"): 0,
            }));
            y += 1;
        }
    }

    if app.interfaces.is_empty() {
        cmds.push(json!({
            String::from("type"): String::from("Text"),
            String::from("x"): 2, String::from("y"): 3,
            String::from("text"): String::from("No non-loopback interfaces found"),
            String::from("fg"): t.text_muted,
            String::from("bold"): false,
            String::from("modifiers"): 0,
        }));
    }

    let status_y = h.saturating_sub(2);
    cmds.push(json!({
        String::from("type"): String::from("Text"),
        String::from("x"): 2, String::from("y"): status_y,
        String::from("text"): format!("Ticks: {}  Max: {}", app.ticks, App::format_speed(app.max_speed)),
        String::from("fg"): t.text_muted,
        String::from("bold"): false,
        String::from("modifiers"): 0,
    }));

    cmds
}

fn hints() -> Vec<(String, String)> {
    vec![("esc".into(), "back".into())]
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
        hints: hints(),
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
                        log::error!("[network-bandwidth-monitor] parse error: {e}: {line}");
                        respond(&mut app, false);
                    }
                }
            }
        }
    }
}
