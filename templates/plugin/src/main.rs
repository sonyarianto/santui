use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, PluginMsg, RenderCmd, ThemeData,
};
use std::io::{self, BufRead, Write};

/// Per-plugin state.  Add your own fields below.
struct PluginState {
    theme: ThemeData,
    area: Area,
    counter: u64,
}

impl PluginState {
    fn new(theme: ThemeData, area: Area) -> Self {
        PluginState {
            theme,
            area,
            counter: 0,
        }
    }

    /// Called every frame — update animations, timers, etc.
    fn handle_tick(&mut self) {
        self.counter = self.counter.wrapping_add(1);
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) {
        match key {
            IpcKey::Up => {}
            IpcKey::Down => {}
            IpcKey::Left => {}
            IpcKey::Right => {}
            IpcKey::Enter => {}
            IpcKey::Esc => {}
            IpcKey::Backspace => {}
            IpcKey::Tab => {}
            IpcKey::Char(_c) => {}
            _ => {}
        }
    }

    /// Build the set of drawing commands for the current frame.
    fn render(&self) -> Vec<RenderCmd> {
        vec![RenderCmd::Text {
            x: 1,
            y: 1,
            text: format!("Hello from {{project-name}}! (tick {})", self.counter),
            fg: Some(self.theme.accent),
            bg: None,
            bold: true,
        }]
    }

    /// Serialise a `PluginMsg` to stdout so the host can consume it.
    fn respond(&self, consumed: bool) {
        let msg = PluginMsg {
            commands: self.render(),
            hints: vec![("Ctrl+P".into(), "commands".into())],
            palette_commands: vec![],
            request: None,
            consumed,
        };
        let json = serde_json::to_string(&msg).expect("serialise PluginMsg");
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "{json}");
        let _ = out.flush();
    }
}

fn main() {
    let mut state: Option<PluginState> = None;
    let stdin = io::stdin().lock();
    let mut raw = String::new();

    for line in stdin.lines() {
        raw.clear();
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        raw = line;

        let host_msg: HostMsg = match serde_json::from_str(&raw) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[{{project-name}}] Failed to parse HostMsg: {e}");
                eprintln!("[{{project-name}}] Raw input: {raw}");
                continue;
            }
        };

        match host_msg {
            HostMsg::Init { theme, area, .. } => {
                state = Some(PluginState::new(theme, area));
            }
            HostMsg::Key { key, modifiers } => {
                if let Some(ref mut s) = state {
                    s.handle_key(key, modifiers);
                }
            }
            HostMsg::Tick => {
                if let Some(ref mut s) = state {
                    s.handle_tick();
                }
            }
            HostMsg::Focus => {}
            HostMsg::Blur => {}
            HostMsg::ThemeChange { theme } => {
                if let Some(ref mut s) = state {
                    s.theme = theme;
                }
            }
            HostMsg::Resize { area } => {
                if let Some(ref mut s) = state {
                    s.area = area;
                }
            }
            HostMsg::Shutdown => break,
            HostMsg::UserUpdate { .. } => {}
            HostMsg::PaletteCommand { .. } => {}
            HostMsg::PluginMessage { .. } => {}
            HostMsg::Mouse { .. } => {}
        }

        // Every host message expects at least one response.
        if let Some(ref s) = state {
            // Track whether the last key was handled internally.
            // Set `true` for keys your plugin consumes (e.g. Esc on a
            // sub-dialog) so the host won't fall back to default handling.
            s.respond(false);
        } else {
            // Not yet initialised — send an empty response so the host
            // doesn't hang waiting for us.
            let empty = PluginMsg {
                commands: vec![],
                hints: vec![],
                palette_commands: vec![],
                request: None,
                consumed: false,
            };
            let json = serde_json::to_string(&empty).expect("serialise empty PluginMsg");
            let mut out = io::stdout().lock();
            let _ = writeln!(out, "{json}");
            let _ = out.flush();
        }
    }
}
