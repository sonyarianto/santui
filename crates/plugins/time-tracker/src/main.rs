use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct TimeEntry {
    name: String,
    running: bool,
    start_time: u64,
    total_secs: u64,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    entries: Vec<TimeEntry>,
    selected: usize,
    scroll: u16,
    input_mode: bool,
    input_buffer: String,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            entries: vec![
                TimeEntry {
                    name: String::from("Work on project"),
                    running: false,
                    start_time: 0,
                    total_secs: 0,
                },
                TimeEntry {
                    name: String::from("Review PRs"),
                    running: false,
                    start_time: 0,
                    total_secs: 0,
                },
            ],
            selected: 0,
            scroll: 0,
            input_mode: false,
            input_buffer: String::new(),
            status: String::from("Ready"),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.input_mode {
            match key {
                IpcKey::Esc => {
                    self.input_mode = false;
                    self.input_buffer.clear();
                    self.status = String::from("Cancelled");
                    true
                }
                IpcKey::Enter => {
                    let name = self.input_buffer.trim().to_string();
                    if !name.is_empty() {
                        self.entries.push(TimeEntry {
                            name,
                            running: false,
                            start_time: 0,
                            total_secs: 0,
                        });
                        self.selected = self.entries.len().saturating_sub(1);
                        self.status = String::from("Task added");
                    }
                    self.input_mode = false;
                    self.input_buffer.clear();
                    true
                }
                IpcKey::Backspace => {
                    self.input_buffer.pop();
                    true
                }
                IpcKey::Char(c) => {
                    self.input_buffer.push(c);
                    true
                }
                _ => true,
            }
        } else {
            match key {
                IpcKey::Char('a') => {
                    self.input_mode = true;
                    self.input_buffer.clear();
                    self.status = String::from("Enter task name:");
                    true
                }
                IpcKey::Char(' ') => {
                    if let Some(entry) = self.entries.get_mut(self.selected) {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        if entry.running {
                            let elapsed = now.saturating_sub(entry.start_time);
                            entry.total_secs = entry.total_secs.saturating_add(elapsed);
                            entry.running = false;
                            entry.start_time = 0;
                            self.status = String::from("Timer stopped");
                        } else {
                            entry.running = true;
                            entry.start_time = now;
                            self.status = format!("Running: {}", entry.name);
                        }
                    }
                    true
                }
                IpcKey::Char('r') => {
                    if let Some(entry) = self.entries.get_mut(self.selected) {
                        entry.total_secs = 0;
                        entry.running = false;
                        entry.start_time = 0;
                        self.status = format!("Reset: {}", entry.name);
                    }
                    true
                }
                IpcKey::Char('D') | IpcKey::Char('d') => {
                    if self.selected < self.entries.len() {
                        self.entries.remove(self.selected);
                        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
                        self.status = String::from("Task deleted");
                    }
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    if self.selected > 0 {
                        self.selected -= 1;
                        self.update_scroll();
                    }
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    if self.selected + 1 < self.entries.len() {
                        self.selected += 1;
                        self.update_scroll();
                    }
                    true
                }
                IpcKey::Esc => false,
                _ => true,
            }
        }
    }

    fn update_scroll(&mut self) {
        let list_h = self.area.h.saturating_sub(5) as usize;
        if self.selected < self.scroll as usize {
            self.scroll = self.selected as u16;
        }
        if self.selected >= self.scroll as usize + list_h {
            self.scroll = (self.selected.saturating_sub(list_h).saturating_add(1)) as u16;
        }
    }

    fn render(&mut self) -> Vec<RenderCmd> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let mut cmds = Vec::new();
        let t = &self.theme;
        let w = self.area.w.max(40);
        let h = self.area.h.max(10);

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
            title: Some(String::from(" Time Tracker ")),
            title_fg: Some(t.text),
            title_dash_fg: Some(t.border),
            border_type: None,
        });

        if self.input_mode {
            let prompt = "Name: ";
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 2,
                text: String::from(prompt),
                fg: Some(t.text),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            let cursor_text = format!("{}_", self.input_buffer);
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 3,
                text: cursor_text,
                fg: Some(t.accent),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        } else {
            let mut total_secs_all: u64 = 0;
            let list_y = 2u16;
            let list_h = h.saturating_sub(4) as usize;
            let mut now_secs = 0u64;
            for entry in &self.entries {
                if entry.running {
                    now_secs = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    break;
                }
            }
            for (i, entry) in self.entries.iter().enumerate() {
                if i < self.scroll as usize || i >= self.scroll as usize + list_h {
                    continue;
                }
                let y = list_y + (i as u16).saturating_sub(self.scroll);
                let is_selected = i == self.selected;
                let display_secs = if entry.running {
                    let elapsed = now_secs.saturating_sub(entry.start_time);
                    entry.total_secs.saturating_add(elapsed)
                } else {
                    entry.total_secs
                };
                total_secs_all = total_secs_all.saturating_add(entry.total_secs);
                if entry.running {
                    total_secs_all =
                        total_secs_all.saturating_add(now_secs.saturating_sub(entry.start_time));
                }
                let time_str = format_secs(display_secs);
                let marker = if entry.running { ">" } else { " " };
                let prefix = if is_selected { ">" } else { " " };
                let line = format!("{}{} {} [{}]", prefix, marker, entry.name, time_str);
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y,
                    text: line,
                    fg: if is_selected {
                        Some(t.highlight)
                    } else {
                        Some(t.text)
                    },
                    bg: if is_selected {
                        Some(t.background_overlay)
                    } else {
                        None
                    },
                    bold: is_selected,
                    modifiers: 0,
                });
            }

            cmds.push(RenderCmd::Text {
                x: 2,
                y: h.saturating_sub(2),
                text: format!("Total: {}", format_secs(total_secs_all)),
                fg: Some(t.accent),
                bg: None,
                bold: true,
                modifiers: 0,
            });
        }

        cmds.push(RenderCmd::Text {
            x: 2,
            y: h.saturating_sub(1),
            text: self.status.clone(),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });

        if !self.input_mode {
            cmds.push(RenderCmd::Text {
                x: 2,
                y: h,
                text: String::from("a add  \u{b7} space toggle  \u{b7} r reset  \u{b7} d delete  \u{b7} \u{2191}\u{2193} nav  \u{b7} esc"),
                fg: Some(t.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
    }
}

fn format_secs(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
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
    vec![(
        "Plugins".to_string(),
        "Track time spent on tasks".to_string(),
    )]
}

fn hints() -> Vec<(String, String)> {
    vec![]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
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
            Ok(HostMsg::Tick) => false,
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[time-tracker] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
