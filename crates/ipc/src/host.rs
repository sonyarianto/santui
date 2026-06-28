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
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

enum Priority {
    High,
    Low,
}

pub struct IpcPluginHost {
    id: String,
    name: String,
    binary_name: String,
    data_dir: PathBuf,
    process: Option<Child>,
    /// Channel receiver for reading parsed responses from a background thread.
    response_rx: Option<Receiver<PluginMsg>>,
    cached_commands: Vec<RenderCmd>,
    cached_hints: Vec<(String, String)>,
    cached_palette_commands: Vec<(String, String)>,
    area: Area,
    /// Cached terminal area, updated from `render()` which takes `&self`.
    /// `Cell` is safe here because `IpcPluginHost` is !Sync — it's owned by a
    /// single-threaded `Vec<Box<dyn Plugin>>` in `PluginManager`. Never shared
    /// across threads.
    current_area: Cell<Area>,
    theme_data: ThemeData,
    pending_request: Option<PluginRequest>,
    /// High-priority channel sender (key, resize, focus, blur, palette).
    writer_high_tx: Option<SyncSender<String>>,
    /// Low-priority channel sender (tick, theme, update, etc.).
    writer_low_tx: Option<SyncSender<String>>,
    /// Join handle for the background writer thread.
    writer_thread: Option<thread::JoinHandle<()>>,
    /// Join handle for the background reader thread, joined on drop.
    reader_thread: Option<thread::JoinHandle<()>>,
    /// Set to `true` when the plugin child process has exited unexpectedly.
    crashed: bool,
    /// Whether the last key event was consumed (handled) by the plugin.
    consumed: bool,
}

impl IpcPluginHost {
    pub fn new(id: &str, name: &str, binary_base: &str) -> Self {
        IpcPluginHost {
            id: id.to_string(),
            name: name.to_string(),
            binary_name: binary_base.to_string(),
            data_dir: PathBuf::new(),
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
            writer_high_tx: None,
            writer_low_tx: None,
            writer_thread: None,
            reader_thread: None,
            crashed: false,
            consumed: false,
        }
    }

    /// Convenience: create a boxed Plugin via the factory.
    pub fn new_boxed(id: &str, name: &str, path: &std::path::Path) -> Box<dyn Plugin + Send> {
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

/// Background writer loop: drains high-priority channel first, then
/// low-priority with a short timeout, writing each message to the plugin's
/// stdin pipe.
fn writer_loop(mut stdin: impl Write, high_rx: Receiver<String>, low_rx: Receiver<String>) {
    loop {
        // Drain all high-priority messages first.
        loop {
            match high_rx.try_recv() {
                Ok(msg) => {
                    if writeln!(stdin, "{msg}").is_err() || stdin.flush().is_err() {
                        return; // pipe broken
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
        // Then check low-priority with a short timeout so high-priority
        // messages are never stuck behind a long block.
        match low_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(msg) => {
                if writeln!(stdin, "{msg}").is_err() || stdin.flush().is_err() {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

impl IpcPluginHost {
    fn send(&mut self, msg: &HostMsg, priority: Priority) {
        let json = match serde_json::to_string(msg) {
            Ok(j) => j,
            Err(e) => {
                log::error!(
                    "[santui] failed to serialize HostMsg for plugin `{}`: {e}",
                    self.id
                );
                return;
            }
        };
        let tx = match priority {
            Priority::High => &self.writer_high_tx,
            Priority::Low => &self.writer_low_tx,
        };
        let crashed = if let Some(ref tx) = tx {
            match tx.try_send(json) {
                Ok(_) => false,
                Err(mpsc::TrySendError::Disconnected(_)) => true,
                Err(mpsc::TrySendError::Full(_)) => {
                    log::warn!(
                        "[santui] plugin `{}` channel full, dropping message",
                        self.id
                    );
                    false
                }
            }
        } else {
            false
        };
        if crashed {
            self.crashed = true;
            log::warn!(
                "[santui] plugin `{}` crashed, channel disconnected",
                self.id
            );
        }
    }

    /// Non-blocking: consume all pending responses from the background reader
    /// thread and keep only the latest cached state.
    fn drain_responses(&mut self) {
        if let Some(ref rx) = self.response_rx {
            loop {
                match rx.try_recv() {
                    Ok(msg) => {
                        self.cached_commands = msg.commands;
                        self.cached_hints = msg.hints;
                        self.cached_palette_commands = msg.palette_commands;
                        self.pending_request = msg.request;
                        self.consumed = msg.consumed;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // Reader thread exited — plugin process likely crashed.
                        self.crashed = true;
                        break;
                    }
                }
            }
        }
    }

    /// Block briefly for one response (used during shutdown only).
    fn recv_shutdown(&mut self) {
        if let Some(ref rx) = self.response_rx {
            let _ = rx.recv_timeout(Duration::from_secs(3));
        }
    }

    /// Kill the plugin process, stop the writer/reader threads, and join them
    /// so no thread leaks on hot-reload.
    fn kill(&mut self) {
        // Drop senders first so the writer loop exits on Disconnected.
        self.writer_high_tx = None;
        self.writer_low_tx = None;

        if let Some(mut child) = self.process.take() {
            // Try to reap first — if already exited, skip the kill.
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // Already exited, nothing to kill.
                }
                Ok(None) => {
                    // Still running — kill it.
                    if let Err(e) = child.kill() {
                        log::warn!("[santui] kill() failed for plugin `{}`: {e}", self.id);
                        // On Unix, ESRCH means already gone; on Windows,
                        // TerminateProcess may fail on a zombie. Try wait
                        // anyway in case it reaps cleanly.
                    }
                    if let Err(e) = child.wait() {
                        log::error!("[santui] wait() failed to reap plugin `{}`: {e}", self.id);
                    }
                }
                Err(e) => {
                    log::warn!("[santui] try_wait() failed for plugin `{}`: {e}", self.id);
                    // Fallback: force kill + wait.
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }

        if let Some(h) = self.writer_thread.take() {
            if let Err(e) = h.join() {
                log::warn!(
                    "[santui] writer thread join failed for plugin `{}`: {e:?}",
                    self.id
                );
            }
        }

        self.response_rx = None;
        if let Some(h) = self.reader_thread.take() {
            if let Err(e) = h.join() {
                log::warn!(
                    "[santui] reader thread join failed for plugin `{}`: {e:?}",
                    self.id
                );
            }
        }
    }

    fn send_recv(&mut self, msg: &HostMsg) {
        self.send(msg, Priority::High);
        self.drain_responses();
    }

    /// Send a message and block briefly for one response.
    /// Used during `init()` so the first PluginMsg (with palette_commands)
    /// is guaranteed to be cached before the host calls refresh_commands().
    fn send_recv_blocking(&mut self, msg: &HostMsg) {
        self.send_recv_blocking_timeout(msg, Duration::from_millis(500));
    }

    fn send_recv_blocking_timeout(&mut self, msg: &HostMsg, timeout: Duration) {
        self.send(msg, Priority::High);
        // Drain any already-available responses first (e.g., Tick responses)
        // so the blocking recv below picks up the response to *this* message.
        self.drain_responses();
        if let Some(ref rx) = self.response_rx {
            if let Ok(resp) = rx.recv_timeout(timeout) {
                self.cached_commands = resp.commands;
                self.cached_hints = resp.hints;
                self.cached_palette_commands = resp.palette_commands;
                self.pending_request = resp.request;
                self.consumed = resp.consumed;
            }
        }
        // Drain any additional responses that piled up, but preserve
        // consumed from the blocking recv so a stale Tick response
        // cannot overwrite it.
        let consumed = self.consumed;
        self.drain_responses();
        self.consumed = consumed;
    }
}

impl Drop for IpcPluginHost {
    fn drop(&mut self) {
        self.kill();
    }
}

impl IpcPluginHost {
    pub fn reset_before_spawn(&mut self) {
        self.crashed = false;
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
    fn spawn(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reset_before_spawn();
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));

        let binary_name = spawn_binary_name(&self.binary_name);
        let binary_path = exe_dir
            .as_ref()
            .map(|d| d.join(&binary_name))
            .unwrap_or_else(|| std::path::PathBuf::from(&binary_name));

        let mut child = Command::new(&binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                format!(
                    "Failed to spawn plugin `{}`: {e}\n  → Run `cargo build --workspace` to build all plugins",
                    binary_name
                )
            })?;

        let reader = child.stdout.take().map(BufReader::new);

        // Writer thread: reads from priority channels and writes to the
        // plugin's stdin pipe.  This is what makes send() non-blocking —
        // the main thread never blocks on a pipe write.
        if let Some(stdin) = child.stdin.take() {
            let (high_tx, high_rx) = mpsc::sync_channel::<String>(8);
            let (low_tx, low_rx) = mpsc::sync_channel::<String>(32);
            let handle = thread::Builder::new()
                .name(format!("ipc-writer-{}", self.id))
                .spawn(move || writer_loop(stdin, high_rx, low_rx))?;
            self.writer_thread = Some(handle);
            self.writer_high_tx = Some(high_tx);
            self.writer_low_tx = Some(low_tx);
        }

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
        Ok(())
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
                        log::error!("[santui] Sign-in failed: {e}");
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
            PluginRequest::PluginsChanged => {
                // The host picks up changes by polling registry.toml.
                // Nothing else to do here — the flag is for the host loop.
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
        self.data_dir = ctx.data_dir.clone();
        if let Ok((w, h)) = terminal::size() {
            self.area = Area {
                w,
                h: h.saturating_sub(1),
            };
            self.current_area.set(self.area);
        }
        self.spawn()?;

        let msg = HostMsg::Init {
            theme: self.theme_data.clone(),
            area: self.area,
            data_dir: self.data_dir.to_string_lossy().to_string(),
        };
        self.send_recv_blocking(&msg);

        if let Some(ref auth) = ctx.auth {
            if let Some(user) = auth.current_user() {
                self.send_recv_blocking(&HostMsg::UserUpdate {
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
            KeyCode::Tab => IpcKey::Tab,
            KeyCode::Char(c) => IpcKey::Char(c),
            _ => return false,
        };
        // Block briefly for the response so the consumed flag reflects this
        // specific key event, not a stale response (e.g., from an earlier Tick).
        self.send_recv_blocking_timeout(&HostMsg::Key { key: ipc_key }, Duration::from_millis(50));
        self.consumed
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
    fn is_alive(&self) -> bool {
        !self.crashed
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
        // Non-blocking: just send Tick and consume any ready response.
        self.send(&HostMsg::Tick, Priority::Low);
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
        self.send(&HostMsg::Shutdown, Priority::High);
        self.recv_shutdown();
    }

    fn can_background(&self) -> bool {
        self.id == "santui-radio-streaming-player"
    }

    fn binary_path(&self) -> Option<&Path> {
        Some(Path::new(&self.binary_name))
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        self.cached_hints.clone()
    }
}
