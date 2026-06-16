use crate::protocol::{
    Area, HostMsg, IpcKey, PluginMsg, PluginRequest, RenderCmd, ThemeData, UserData,
};
use crate::render::render_commands;
use crossterm::event::{KeyCode, KeyEvent};
use crossterm::terminal;
use ratatui::layout::Rect;
use ratatui::Frame;
use santui_core::auth::User;
use santui_core::theme::Theme;
use santui_core::{AuthHandle, Plugin, PluginContext};
use std::cell::Cell;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::Arc;

pub struct IpcPluginHost {
    id: &'static str,
    name: &'static str,
    binary_name: &'static str,
    process: Option<Child>,
    reader: Option<BufReader<ChildStdout>>,
    cached_commands: Vec<RenderCmd>,
    cached_hints: Vec<(String, String)>,
    area: Area,
    current_area: Cell<Area>,
    theme_data: ThemeData,
    pending_request: Option<PluginRequest>,
}

impl IpcPluginHost {
    pub fn new(id: &'static str, name: &'static str, binary_base: &'static str) -> Self {
        IpcPluginHost {
            id,
            name,
            binary_name: binary_base,
            process: None,
            reader: None,
            cached_commands: Vec::new(),
            cached_hints: Vec::new(),
            area: Area { w: 80, h: 24 },
            current_area: Cell::new(Area { w: 80, h: 24 }),
            theme_data: ThemeData {
                text: [220, 220, 220],
                text_muted: [140, 140, 140],
                accent: [157, 124, 216],
                highlight: [250, 178, 131],
                border: [250, 178, 131],
                success: [127, 216, 143],
                error: [224, 108, 117],
                background_panel: [20, 20, 20],
            },
            pending_request: None,
        }
    }
}

fn color_to_rgb(c: &ratatui::style::Color) -> [u8; 3] {
    match c {
        ratatui::style::Color::Rgb(r, g, b) => [*r, *g, *b],
        _ => [220, 220, 220],
    }
}

fn theme_to_data(theme: &Theme) -> ThemeData {
    ThemeData {
        text: color_to_rgb(&theme.text),
        text_muted: color_to_rgb(&theme.text_muted),
        accent: color_to_rgb(&theme.accent),
        highlight: color_to_rgb(&theme.highlight),
        border: color_to_rgb(&theme.border),
        success: color_to_rgb(&theme.success),
        error: color_to_rgb(&theme.error),
        background_panel: color_to_rgb(&theme.background_panel),
    }
}

fn user_to_data(user: &User) -> UserData {
    UserData {
        id: user.id.clone(),
        email: user.email.clone(),
        name: user.name.clone(),
        avatar_url: user.avatar_url.clone(),
        provider: user.provider.clone(),
    }
}

impl IpcPluginHost {
    fn send(&mut self, msg: &HostMsg) {
        let child = match self.process.as_mut() {
            Some(c) => c,
            None => return,
        };
        let stdin = match child.stdin.as_mut() {
            Some(s) => s,
            None => return,
        };
        let json = serde_json::to_string(msg).expect("HostMsg serialization");
        let _ = writeln!(stdin, "{json}");
        let _ = stdin.flush();
    }

    fn recv(&mut self) {
        if self.process.is_none() {
            return;
        }
        let reader = match self.reader.as_mut() {
            Some(r) => r,
            None => {
                self.process = None;
                return;
            }
        };
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => {
                self.process = None;
                self.reader = None;
            }
            Ok(_) => {
                if let Ok(msg) = serde_json::from_str::<PluginMsg>(&line) {
                    self.cached_commands = msg.commands;
                    self.cached_hints = msg.hints;
                    self.pending_request = msg.request;
                }
            }
        }
    }

    fn send_recv(&mut self, msg: &HostMsg) {
        self.send(msg);
        self.recv();
    }

    fn spawn_binary_name(&self) -> String {
        let base = self.binary_name;
        if cfg!(windows) && !base.ends_with(".exe") {
            format!("{}.exe", base)
        } else {
            base.to_string()
        }
    }

    fn spawn(&mut self) {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));

        let binary_name = self.spawn_binary_name();
        let binary_path = exe_dir
            .as_ref()
            .map(|d| d.join(&binary_name))
            .unwrap_or_else(|| std::path::PathBuf::from(&binary_name));

        match Command::new(&binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(mut child) => {
                self.reader = child.stdout.take().map(BufReader::new);
                self.process = Some(child);
            }
            Err(e) => {
                eprintln!("[santui] Failed to spawn plugin `{}`: {e}", binary_name);
                eprintln!("[santui]   → Run `cargo build --workspace` to build all plugins");
            }
        }
    }

    /// Process any pending request from the plugin.
    /// Returns true if the plugin's render cache is now stale and should be re-queried.
    pub fn process_request(&mut self, auth: &Arc<dyn AuthHandle>) -> bool {
        let req = match self.pending_request.take() {
            Some(r) => r,
            None => return false,
        };
        match req {
            PluginRequest::SignIn { provider } => {
                match auth.sign_in(&provider) {
                    Ok(user) => {
                        self.send_recv(&HostMsg::UserUpdate {
                            user: Some(user_to_data(&user)),
                        });
                    }
                    Err(e) => {
                        eprintln!("[santui] Sign-in failed: {e}");
                        self.send_recv(&HostMsg::UserUpdate { user: None });
                    }
                }
                true
            }
            PluginRequest::SignOut => {
                auth.sign_out();
                self.send_recv(&HostMsg::UserUpdate { user: None });
                true
            }
        }
    }
}

impl Plugin for IpcPluginHost {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &str {
        self.name
    }

    fn init(&mut self, ctx: &mut PluginContext) -> Result<(), Box<dyn std::error::Error>> {
        self.theme_data = theme_to_data(&ctx.theme);
        if let Ok((w, h)) = terminal::size() {
            self.area = Area {
                w,
                h: h.saturating_sub(1),
            };
            self.current_area.set(self.area);
        }
        self.spawn();

        let msg = HostMsg::Init {
            theme: self.theme_data.clone(),
            area: self.area,
        };
        self.send_recv(&msg);

        if let Some(ref auth) = ctx.auth {
            if let Some(user) = auth.current_user() {
                self.send_recv(&HostMsg::UserUpdate {
                    user: Some(user_to_data(&user)),
                });
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let ipc_key = match key.code {
            KeyCode::Up => IpcKey::Up,
            KeyCode::Down => IpcKey::Down,
            KeyCode::PageUp => IpcKey::PageUp,
            KeyCode::PageDown => IpcKey::PageDown,
            KeyCode::Enter => IpcKey::Enter,
            KeyCode::Esc => IpcKey::Esc,
            KeyCode::Backspace => IpcKey::Backspace,
            KeyCode::Char(c) => IpcKey::Char(c),
            _ => return false,
        };
        self.send_recv(&HostMsg::Key { key: ipc_key });
        true
    }

    fn render(&self, f: &mut Frame, area: Rect) {
        self.current_area.set(Area {
            w: area.width,
            h: area.height,
        });
        render_commands(f, area, &self.cached_commands);
    }

    fn tick(&mut self) {
        if let Ok((w, h)) = terminal::size() {
            let usable = Area {
                w,
                h: h.saturating_sub(1),
            };
            if usable != self.current_area.get() {
                self.current_area.set(usable);
                self.area = usable;
                self.send_recv(&HostMsg::Resize { area: usable });
                return;
            }
        }
        self.send_recv(&HostMsg::Tick);
    }

    fn on_focus(&mut self) {
        self.send_recv(&HostMsg::Focus);
    }

    fn on_blur(&mut self) {
        self.send_recv(&HostMsg::Blur);
    }

    fn on_theme_change(&mut self, theme: &Theme) {
        self.theme_data = theme_to_data(theme);
        self.send_recv(&HostMsg::ThemeChange {
            theme: self.theme_data.clone(),
        });
    }

    fn on_user_update(&mut self, user: Option<&User>) {
        self.send_recv(&HostMsg::UserUpdate {
            user: user.map(user_to_data),
        });
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        self.cached_hints.clone()
    }
}
