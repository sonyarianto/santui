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
use santui_core::{AuthHandle, Plugin, PluginCmdItem, PluginContext};
use std::cell::Cell;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// How long to wait for a plugin response before considering it dead.
const IPC_TIMEOUT: Duration = Duration::from_secs(5);

pub struct IpcPluginHost {
    id: String,
    name: String,
    binary_name: String,
    process: Option<Child>,
    /// Channel receiver for reading parsed responses from a background thread.
    response_rx: Option<Receiver<PluginMsg>>,
    cached_commands: Vec<RenderCmd>,
    cached_hints: Vec<(String, String)>,
    cached_palette_commands: Vec<(String, String)>,
    area: Area,
    current_area: Cell<Area>,
    theme_data: ThemeData,
    pending_request: Option<PluginRequest>,
    /// Join handle for the background reader thread, joined on drop.
    reader_thread: Option<thread::JoinHandle<()>>,
}

impl IpcPluginHost {
    pub fn new(id: &str, name: &str, binary_base: &str) -> Self {
        IpcPluginHost {
            id: id.to_string(),
            name: name.to_string(),
            binary_name: binary_base.to_string(),
            process: None,
            response_rx: None,
            cached_commands: Vec::new(),
            cached_hints: Vec::new(),
            cached_palette_commands: Vec::new(),
            area: Area { w: 80, h: 24 },
            current_area: Cell::new(Area { w: 80, h: 24 }),
            theme_data: ThemeData {
                text: [220, 220, 220],
                text_muted: [140, 140, 140],
                accent: [157, 124, 216],
                highlight: [250, 178, 131],
                logo: [255, 185, 0],
                background: [20, 20, 20],
                background_panel: [20, 20, 20],
                background_overlay: [10, 10, 10],
                border: [250, 178, 131],
                success: [127, 216, 143],
                error: [224, 108, 117],
                inverted_text: [20, 20, 20],
            },
            pending_request: None,
            reader_thread: None,
        }
    }

    /// Convenience: create a boxed Plugin via the factory.
    pub fn new_boxed(id: &str, name: &str, path: &std::path::Path) -> Box<dyn Plugin> {
        let binary = path.to_string_lossy().to_string();
        Box::new(IpcPluginHost::new(id, name, &binary))
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
        logo: color_to_rgb(&theme.logo),
        background: color_to_rgb(&theme.background),
        background_panel: color_to_rgb(&theme.background_panel),
        background_overlay: color_to_rgb(&theme.background_overlay),
        border: color_to_rgb(&theme.border),
        success: color_to_rgb(&theme.success),
        error: color_to_rgb(&theme.error),
        inverted_text: color_to_rgb(&theme.inverted_text),
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

    /// Block up to `IPC_TIMEOUT` waiting for a plugin response.
    /// If the plugin doesn't respond in time we kill it so the main
    /// thread can never hang forever on a crashed child process.
    fn recv(&mut self) {
        if let Some(ref rx) = self.response_rx {
            match rx.recv_timeout(IPC_TIMEOUT) {
                Ok(msg) => {
                    self.cached_commands = msg.commands;
                    self.cached_hints = msg.hints;
                    self.cached_palette_commands = msg.palette_commands;
                    self.pending_request = msg.request;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    eprintln!(
                        "[santui] Plugin `{}` didn\'t respond within {}.{}s — killing",
                        self.name,
                        IPC_TIMEOUT.as_secs(),
                        IPC_TIMEOUT.subsec_millis(),
                    );
                    self.kill();
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.process = None;
                }
            }
        }
    }

    /// Non-blocking: consume all pending responses from the background reader
    /// thread and keep only the latest cached state.
    fn drain_responses(&mut self) {
        if let Some(ref rx) = self.response_rx {
            while let Ok(msg) = rx.try_recv() {
                self.cached_commands = msg.commands;
                self.cached_hints = msg.hints;
                self.cached_palette_commands = msg.palette_commands;
                self.pending_request = msg.request;
            }
        }
    }

    /// Kill the plugin process, drop the response channel, and join the
    /// background reader thread so no thread leaks on hot-reload.
    fn kill(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.response_rx = None;
        if let Some(h) = self.reader_thread.take() {
            let _ = h.join();
        }
    }

    fn send_recv(&mut self, msg: &HostMsg) {
        self.send(msg);
        self.recv();
    }
}

impl Drop for IpcPluginHost {
    fn drop(&mut self) {
        self.kill();
    }
}

fn spawn_binary_name(binary_base: &str) -> String {
    if cfg!(windows) && !binary_base.ends_with(".exe") {
        format!("{}.exe", binary_base)
    } else {
        binary_base.to_string()
    }
}

impl IpcPluginHost {
    fn spawn(&mut self) {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));

        let binary_name = spawn_binary_name(&self.binary_name);
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
                let reader = child.stdout.take().map(BufReader::new);
                self.process = Some(child);

                // Background thread: continuously read stdout, send parsed
                // responses back via the channel.  This is what makes tick()
                // non-blocking — the main thread never blocks on a read.
                if let Some(reader) = reader {
                    let (tx, rx) = mpsc::channel::<PluginMsg>();
                    let handle = thread::Builder::new()
                        .name(format!("ipc-reader-{}", self.id))
                        .spawn(move || {
                            let mut reader = reader;
                            let mut line = String::new();
                            loop {
                                line.clear();
                                match reader.read_line(&mut line) {
                                    Ok(0) | Err(_) => break,
                                    Ok(_) => {
                                        if let Ok(msg) = serde_json::from_str::<PluginMsg>(&line) {
                                            let _ = tx.send(msg);
                                        }
                                    }
                                }
                            }
                        });
                    if let Ok(h) = handle {
                        self.reader_thread = Some(h);
                    }
                    self.response_rx = Some(rx);
                }
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
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
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

    /// Tick is non-blocking: send the message, then drain any pending
    /// responses without waiting.  This keeps the UI responsive even when
    /// a plugin is slow to process a tick.
    fn tick(&mut self) {
        if let Ok((w, h)) = terminal::size() {
            let usable = Area {
                w,
                h: h.saturating_sub(1),
            };
            if usable != self.current_area.get() {
                self.current_area.set(usable);
                self.area = usable;
                // Resize needs an immediate response so we know the new layout.
                self.send_recv(&HostMsg::Resize { area: usable });
                return;
            }
        }
        // Non-blocking: just send Tick and consume any ready response.
        self.send(&HostMsg::Tick);
        self.drain_responses();
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

    fn commands(&self) -> Vec<PluginCmdItem> {
        self.cached_palette_commands
            .iter()
            .map(|(cat, label)| PluginCmdItem {
                category: cat.clone(),
                label: label.clone(),
            })
            .collect()
    }

    fn handle_palette_command(&mut self, index: usize) {
        self.send_recv(&HostMsg::PaletteCommand {
            index: index as u32,
        });
    }

    fn on_plugin_message(&mut self, from: &str, action: &str, data: &str) {
        self.send_recv(&HostMsg::PluginMessage {
            from: from.to_string(),
            action: action.to_string(),
            data: data.to_string(),
        });
    }

    fn shutdown(&mut self) {
        self.send(&HostMsg::Shutdown);
        if let Some(ref rx) = self.response_rx {
            let _ = rx.recv_timeout(Duration::from_secs(1));
        }
    }

    fn binary_path(&self) -> Option<&Path> {
        Some(Path::new(&self.binary_name))
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        self.cached_hints.clone()
    }
}
