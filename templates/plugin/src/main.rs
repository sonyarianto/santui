use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, PluginMsg, RenderCmd, ThemeData,
};
use std::io::{self, BufRead};

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

    /// Serialise a `PluginMsg` to stdout as a binary bincode frame.
    fn respond(&self, consumed: bool) {
        let msg = PluginMsg {
            commands: self.render(),
            hints: vec![("Ctrl+P".into(), "commands".into())],
            palette_commands: vec![],
            request: None,
            consumed,
        };
        let mut out = io::stdout().lock();
        let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &msg);
    }
}

fn main() {
    let mut state: Option<PluginState> = None;
    let mut stdin = io::stdin().lock();

    loop {
        let host_msg: HostMsg = match santui_ipc::protocol::read_host_msg_json(&mut stdin) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => {
                eprintln!("[{{project-name}}] Failed to parse HostMsg: {e}");
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
            HostMsg::DbValue { key, value } => {
                // Restore persisted state from `value` (e.g. JSON),
                // or handle `value == None` as "no data yet".
                // If your plugin uses DbGet/DbSet requests, use this
                // arm to update your state from the database response.
                let _ = (key, value);
            }
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
            let mut out = io::stdout().lock();
            let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &empty);
        }
    }
}
