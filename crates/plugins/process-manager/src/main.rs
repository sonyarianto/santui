use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct Process {
    pid: u32,
    name: String,
    status: String,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    processes: Vec<Process>,
    selected: usize,
    scroll: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            processes: Vec::new(),
            selected: 0,
            scroll: 0,
            status: String::from("Reading /proc/ ..."),
        };
        app.read_proc();
        app
    }
}

impl App {
    fn read_proc(&mut self) {
        self.processes.clear();
        match std::fs::read_dir("/proc/") {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy().to_string();
                    let pid: u32 = match name_str.parse() {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    let proc_name = Self::read_proc_name(pid);
                    let status = Self::read_proc_status(pid);
                    self.processes.push(Process {
                        pid,
                        name: proc_name,
                        status,
                    });
                }
                self.processes.sort_by_key(|p| p.pid);
                self.status = format!("{} processes loaded", self.processes.len());
            }
            Err(e) => {
                self.status = format!("Error reading /proc/: {e}");
            }
        }
    }

    fn read_proc_name(pid: u32) -> String {
        let stat_path = format!("/proc/{pid}/stat");
        if let Ok(content) = std::fs::read_to_string(&stat_path) {
            if let Some(open_paren) = content.find('(') {
                if let Some(close_paren) = content.rfind(')') {
                    return content[open_paren + 1..close_paren].to_string();
                }
            }
        }
        String::from("?")
    }

    fn read_proc_status(pid: u32) -> String {
        let status_path = format!("/proc/{pid}/status");
        if let Ok(content) = std::fs::read_to_string(&status_path) {
            for line in content.lines() {
                if line.starts_with("State:") {
                    let state = line.trim_start_matches("State:").trim();
                    return state
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                }
            }
        }
        String::from("?")
    }

    fn kill_selected(&mut self) {
        if let Some(proc) = self.processes.get(self.selected) {
            let pid = proc.pid;
            self.status = format!("Mock kill: PID {pid} ({}) would be terminated", proc.name);
        }
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                true
            }
            IpcKey::Down => {
                if self.selected + 1 < self.processes.len() {
                    self.selected += 1;
                }
                true
            }
            IpcKey::Char('k') => {
                self.kill_selected();
                true
            }
            IpcKey::Char('r') => {
                self.read_proc();
                true
            }
            IpcKey::Esc => false,
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
            "bg": t.background_panel, "title": " Process Manager ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        let list_y = 1u16;
        let list_h = h.saturating_sub(4) as usize;
        let list_w = w.saturating_sub(2);

        let items: Vec<String> = self
            .processes
            .iter()
            .skip(self.scroll)
            .take(list_h)
            .map(|p| format!("{:>7}  {:<4}  {}", p.pid, p.status, p.name))
            .collect();

        let vis_sel = if self.selected >= self.scroll && self.selected - self.scroll < list_h {
            Some(self.selected - self.scroll)
        } else {
            None
        };

        cmds.push(json!({"List": {
            "x": 1, "y": list_y, "w": list_w, "h": list_h as u16,
            "items": items,
            "selected": vis_sel,
            "style": {"fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0},
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
    json!([["Plugins", "Process Manager"]])
}

fn key_hints() -> Value {
    json!([
        ["esc", "close"],
        ["\u{2191}\u{2193}", "navigate"],
        ["k", "kill (mock)"],
        ["r", "refresh"],
    ])
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
                log::error!("[process-manager] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
    }
}
