use crate::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, IpcMouseEvent, PluginMsg, PluginRequest, RenderCmd,
    ThemeData, UserData,
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
    #[allow(dead_code)]
    Low,
}

pub struct IpcPluginHost {
    id: String,
    name: String,
    binary_name: String,
    capabilities: Vec<String>,
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
    /// Whether this plugin stays loaded on Esc (e.g., the registry plugin).
    persistent: bool,
    /// Event-driven Esc: set to true when Esc is sent, cleared when response arrives.
    esc_pending: bool,
    /// Frames waited since esc_pending was set (timeout safeguard).
    esc_pending_frames: u32,
    /// response_count value when esc_pending was set.
    last_resp_count: u64,
    /// Total responses received (incremented in drain_responses).
    response_count: u64,
    /// Resolved consumed value from the pending Esc response.
    esc_consumed: Option<bool>,
}

impl IpcPluginHost {
    pub fn new(id: &str, name: &str, binary_base: &str) -> Self {
        IpcPluginHost {
            id: id.to_string(),
            name: name.to_string(),
            binary_name: binary_base.to_string(),
            capabilities: Vec::new(),
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
            persistent: false,
            esc_pending: false,
            esc_pending_frames: 0,
            last_resp_count: 0,
            response_count: 0,
            esc_consumed: None,
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
        match low_rx.recv_timeout(Duration::from_millis(1)) {
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
                        self.response_count += 1;
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
        // Drain stale responses so the blocking recv picks up the fresh one.
        self.drain_responses();
        if let Some(ref rx) = self.response_rx {
            if let Ok(resp) = rx.recv_timeout(timeout) {
                self.cached_commands = resp.commands;
                self.cached_hints = resp.hints;
                self.cached_palette_commands = resp.palette_commands;
                self.pending_request = resp.request;
                self.consumed = resp.consumed;
            }
            // Drain any additional responses. If a stale Tick response was
            // processed during recv_timeout, it arrived first in the channel
            // and recv_timeout captured it (consumed=false). But this
            // message's response is always processed last (FIFO) and is still
            // in the channel, so drain_responses overwrites consumed with
            // the correct value. If recv_timeout already got this message's
            // response, drain_responses finds nothing and consumed is preserved.
            self.drain_responses();
        }
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

        let mut cmd = Command::new(&binary_path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        // Tell plugins where the host's native/ directory lives so they can
        // find bundled native dependencies (e.g. libmpv for radio-stream-player)
        // even when installed via the plugin registry to a separate path.
        if let Some(ref dir) = exe_dir {
            let native_dir = dir.join("native");
            cmd.env("SANTUI_NATIVE_DIR", native_dir);
        }

        let mut child = cmd.spawn().map_err(|e| {
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
            KeyCode::Left => IpcKey::Left,
            KeyCode::Right => IpcKey::Right,
            KeyCode::PageUp => IpcKey::PageUp,
            KeyCode::PageDown => IpcKey::PageDown,
            KeyCode::Enter => IpcKey::Enter,
            KeyCode::Esc => IpcKey::Esc,
            KeyCode::Backspace => IpcKey::Backspace,
            KeyCode::Tab => IpcKey::Tab,
            KeyCode::BackTab => IpcKey::BackTab,
            KeyCode::Delete => IpcKey::Delete,
            KeyCode::Insert => IpcKey::Insert,
            KeyCode::Home => IpcKey::Home,
            KeyCode::End => IpcKey::End,
            KeyCode::Char(c) => IpcKey::Char(c),
            KeyCode::F(n) => IpcKey::F(n),
            _ => return false,
        };
        let modifiers = IpcKeyModifiers {
            shift: key
                .modifiers
                .contains(crossterm::event::KeyModifiers::SHIFT),
            ctrl: key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL),
            alt: key.modifiers.contains(crossterm::event::KeyModifiers::ALT),
        };

        if matches!(key.code, KeyCode::Esc) {
            // Event-driven Esc: send immediately, drain stale Tick responses,
            // set esc_pending so tick() can resolve consumed on the next frame.
            // Return true optimistically — the host defers the close decision
            // until tick() resolves the pending Esc.
            self.send(
                &HostMsg::Key {
                    key: ipc_key,
                    modifiers,
                },
                Priority::High,
            );
            self.drain_responses();
            self.esc_pending = true;
            self.esc_pending_frames = 0;
            self.last_resp_count = self.response_count;
            return true;
        }

        // Non-Esc keys: host doesn't care about consumed, no blocking needed.
        self.send(
            &HostMsg::Key {
                key: ipc_key,
                modifiers,
            },
            Priority::High,
        );
        self.drain_responses();
        self.consumed
    }

    fn handle_mouse(&mut self, event: &crossterm::event::MouseEvent) -> bool {
        use crate::protocol::{MouseButton, MouseEventKind};
        use crossterm::event::{MouseButton as Mb, MouseEventKind as Mek};
        let button = match event.kind {
            Mek::Down(btn) | Mek::Up(btn) | Mek::Drag(btn) => match btn {
                Mb::Left => MouseButton::Left,
                Mb::Right => MouseButton::Right,
                Mb::Middle => MouseButton::Middle,
            },
            Mek::ScrollUp => MouseButton::Left,
            Mek::ScrollDown => MouseButton::Left,
            _ => return false,
        };
        let kind = match event.kind {
            Mek::Down(_) => MouseEventKind::Down,
            Mek::Up(_) => MouseEventKind::Up,
            Mek::Drag(_) => MouseEventKind::Drag,
            Mek::Moved => MouseEventKind::Moved,
            Mek::ScrollUp => MouseEventKind::ScrollUp,
            Mek::ScrollDown => MouseEventKind::ScrollDown,
            Mek::ScrollLeft | Mek::ScrollRight => return false,
        };
        let ipc = IpcMouseEvent {
            x: event.column,
            y: event.row,
            button,
            kind,
            modifiers: IpcKeyModifiers {
                shift: event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT),
                ctrl: event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL),
                alt: event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::ALT),
            },
        };
        self.send_recv(&HostMsg::Mouse { event: ipc });
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
        if self.esc_pending {
            // Don't send Tick while waiting for the Esc response.
            // No Tick → any response in the channel must be from Esc.
            // Drain once and check if a new response arrived.
            self.esc_pending_frames += 1;
            let before = self.response_count;
            self.drain_responses();
            if self.response_count > before {
                // New response received → it's from Esc (no Ticks were sent).
                self.esc_consumed = Some(self.consumed);
                self.esc_pending = false;
            } else if self.esc_pending_frames >= 10 {
                // Timeout safeguard: ~1s at 100ms tick rate.
                self.esc_consumed = Some(false);
                self.esc_pending = false;
            }
            return;
        }

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
        self.send(&HostMsg::Tick, Priority::High);
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
        self.capabilities.iter().any(|c| c == "background")
    }

    fn set_capabilities(&mut self, caps: Vec<String>) {
        self.capabilities = caps;
    }

    fn persistent(&self) -> bool {
        self.persistent
    }

    fn set_persistent(&mut self, persistent: bool) {
        self.persistent = persistent;
    }

    fn binary_path(&self) -> Option<&Path> {
        Some(Path::new(&self.binary_name))
    }

    fn take_pending_esc_result(&mut self) -> Option<bool> {
        self.esc_consumed.take()
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        self.cached_hints.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use santui_core::auth::User;
    use santui_core::theme::Theme;
    use std::sync::mpsc::sync_channel;

    // ─── Free function tests ───

    #[test]
    fn test_spawn_binary_name_appends_exe_on_windows() {
        let name = spawn_binary_name("my-plugin");
        if cfg!(windows) {
            assert_eq!(name, "my-plugin.exe");
        } else {
            assert_eq!(name, "my-plugin");
        }
    }

    #[test]
    fn test_spawn_binary_name_keeps_existing_exe() {
        assert_eq!(spawn_binary_name("plugin.exe"), "plugin.exe");
    }

    #[test]
    fn test_color_to_rgb_converts_rgb() {
        let c = ratatui::style::Color::Rgb(100, 150, 200);
        assert_eq!(color_to_rgb(&c), [100, 150, 200]);
    }

    #[test]
    fn test_color_to_rgb_fallback_for_non_rgb() {
        let c = ratatui::style::Color::Reset;
        assert_eq!(color_to_rgb(&c), [220, 220, 220]);
    }

    #[test]
    fn test_theme_to_data_converts_all_fields() {
        let theme = Theme {
            accent: ratatui::style::Color::Rgb(1, 2, 3),
            highlight: ratatui::style::Color::Rgb(4, 5, 6),
            logo: ratatui::style::Color::Rgb(7, 8, 9),
            text: ratatui::style::Color::Rgb(10, 11, 12),
            text_muted: ratatui::style::Color::Rgb(13, 14, 15),
            background: ratatui::style::Color::Rgb(16, 17, 18),
            background_panel: ratatui::style::Color::Rgb(19, 20, 21),
            background_overlay: ratatui::style::Color::Rgb(22, 23, 24),
            border: ratatui::style::Color::Rgb(25, 26, 27),
            success: ratatui::style::Color::Rgb(28, 29, 30),
            error: ratatui::style::Color::Rgb(31, 32, 33),
            inverted_text: ratatui::style::Color::Rgb(34, 35, 36),
        };
        let data = theme_to_data(&theme);
        assert_eq!(data.accent, [1, 2, 3]);
        assert_eq!(data.highlight, [4, 5, 6]);
        assert_eq!(data.logo, [7, 8, 9]);
        assert_eq!(data.text, [10, 11, 12]);
        assert_eq!(data.text_muted, [13, 14, 15]);
        assert_eq!(data.background, [16, 17, 18]);
        assert_eq!(data.background_panel, [19, 20, 21]);
        assert_eq!(data.background_overlay, [22, 23, 24]);
        assert_eq!(data.border, [25, 26, 27]);
        assert_eq!(data.success, [28, 29, 30]);
        assert_eq!(data.error, [31, 32, 33]);
        assert_eq!(data.inverted_text, [34, 35, 36]);
    }

    #[test]
    fn test_user_to_data_converts_all_fields() {
        let user = User {
            id: "42".into(),
            email: "test@test.com".into(),
            name: "Test User".into(),
            avatar_url: Some("https://example.com/avatar.png".into()),
            provider: "github".into(),
        };
        let data = user_to_data(&user);
        assert_eq!(data.id, "42");
        assert_eq!(data.email, "test@test.com");
        assert_eq!(data.name, "Test User");
        assert_eq!(
            data.avatar_url,
            Some("https://example.com/avatar.png".into())
        );
        assert_eq!(data.provider, "github");
    }

    #[test]
    fn test_user_to_data_without_avatar() {
        let user = User {
            id: "1".into(),
            email: "a@b.com".into(),
            name: "A".into(),
            avatar_url: None,
            provider: "google".into(),
        };
        let data = user_to_data(&user);
        assert_eq!(data.avatar_url, None);
    }

    #[test]
    fn test_writer_loop_writes_high_priority_messages() {
        let written = Arc::new(std::sync::Mutex::new(Vec::new()));
        let w = Arc::clone(&written);

        struct SharedWriter(Arc<std::sync::Mutex<Vec<u8>>>);
        impl std::io::Write for SharedWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let (high_tx, high_rx) = sync_channel::<String>(8);
        let (_low_tx, low_rx) = sync_channel::<String>(32);

        let handle = thread::spawn(move || {
            writer_loop(SharedWriter(w), high_rx, low_rx);
        });

        high_tx.try_send("ping".into()).unwrap();
        // Drop sender so writer loop exits after draining
        drop(high_tx);
        drop(_low_tx);
        handle.join().unwrap();

        let output = String::from_utf8(written.lock().unwrap().clone()).unwrap();
        assert_eq!(output, "ping\n");
    }

    #[test]
    fn test_writer_loop_writes_low_priority_messages() {
        let written = Arc::new(std::sync::Mutex::new(Vec::new()));
        let w = Arc::clone(&written);

        struct SharedWriter(Arc<std::sync::Mutex<Vec<u8>>>);
        impl std::io::Write for SharedWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let (high_tx, high_rx) = sync_channel::<String>(8);
        let (low_tx, low_rx) = sync_channel::<String>(32);

        let handle = thread::spawn(move || {
            writer_loop(SharedWriter(w), high_rx, low_rx);
        });

        // Give writer time to reach the low-priority check
        thread::sleep(Duration::from_millis(10));
        low_tx.try_send("data".into()).unwrap();
        thread::sleep(Duration::from_millis(10));

        drop(high_tx);
        drop(low_tx);
        handle.join().unwrap();

        let output = String::from_utf8(written.lock().unwrap().clone()).unwrap();
        assert_eq!(output, "data\n");
    }

    // ─── IpcPluginHost unit tests ───

    #[test]
    fn test_new_default_fields() {
        let host = IpcPluginHost::new("test-id", "Test Plugin", "/path/to/bin");
        assert_eq!(host.id, "test-id");
        assert_eq!(host.name, "Test Plugin");
        assert_eq!(host.binary_name, "/path/to/bin");
        assert!(host.capabilities.is_empty());
        assert!(host.process.is_none());
        assert!(host.response_rx.is_none());
        assert!(host.cached_commands.is_empty());
        assert!(host.cached_hints.is_empty());
        assert!(host.cached_palette_commands.is_empty());
        assert_eq!(host.area, Area { w: 80, h: 24 });
        assert!(!host.crashed);
        assert!(!host.consumed);
        assert!(!host.persistent);
        assert!(host.pending_request.is_none());
        assert!(host.writer_high_tx.is_none());
        assert!(host.writer_low_tx.is_none());
    }

    #[test]
    fn test_id_and_name_accessors() {
        let host = IpcPluginHost::new("my-id", "My Name", "bin");
        assert_eq!(host.id(), "my-id");
        assert_eq!(host.name(), "My Name");
    }

    #[test]
    fn test_reset_before_spawn_clears_crashed() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        host.crashed = true;
        host.reset_before_spawn();
        assert!(!host.crashed);
    }

    #[test]
    fn test_is_alive_reflects_crashed() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        assert!(host.is_alive());
        host.crashed = true;
        assert!(!host.is_alive());
    }

    #[test]
    fn test_can_background_without_capabilities() {
        let host = IpcPluginHost::new("id", "name", "bin");
        assert!(!host.can_background());
    }

    #[test]
    fn test_can_background_with_background_cap() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        host.set_capabilities(vec!["background".into()]);
        assert!(host.can_background());
    }

    #[test]
    fn test_can_background_with_other_cap() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        host.set_capabilities(vec!["audio".into()]);
        assert!(!host.can_background());
    }

    #[test]
    fn test_persistent_default_and_set() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        assert!(!host.persistent());
        host.set_persistent(true);
        assert!(host.persistent());
    }

    #[test]
    fn test_binary_path_returns_some() {
        let host = IpcPluginHost::new("id", "name", "/some/path");
        assert_eq!(host.binary_path(), Some(Path::new("/some/path")));
    }

    #[test]
    fn test_new_boxed_creates_plugin_with_correct_path() {
        let path = Path::new("/tmp/my-plugin");
        let plugin = IpcPluginHost::new_boxed("b", "B", path);
        assert_eq!(plugin.id(), "b");
        assert_eq!(plugin.name(), "B");
        assert_eq!(plugin.binary_path(), Some(path));
    }

    #[test]
    fn test_commands_returns_cached_palette_commands() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        host.cached_palette_commands = vec![
            ("Cat".into(), "Action".into()),
            ("Other".into(), "Thing".into()),
        ];
        let cmds = host.commands();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].category, "Cat");
        assert_eq!(cmds[0].label, "Action");
        assert_eq!(cmds[1].category, "Other");
        assert_eq!(cmds[1].label, "Thing");
    }

    #[test]
    fn test_status_hints_returns_cached_hints() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        host.cached_hints = vec![("k".into(), "v".into())];
        let hints = host.status_hints();
        assert_eq!(hints, vec![("k".into(), "v".into())]);
    }

    // ─── Channel-based tests ───

    #[test]
    fn test_send_high_priority_puts_json_on_high_channel() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (tx, rx) = sync_channel::<String>(8);
        host.writer_high_tx = Some(tx);
        host.writer_low_tx = Some(sync_channel::<String>(8).0);

        host.send(&HostMsg::Tick, Priority::High);

        let sent = rx.try_recv().unwrap();
        let decoded: HostMsg = serde_json::from_str(&sent).unwrap();
        assert!(matches!(decoded, HostMsg::Tick));
    }

    #[test]
    fn test_send_low_priority_puts_json_on_low_channel() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        host.writer_high_tx = Some(sync_channel::<String>(8).0);
        let (tx, rx) = sync_channel::<String>(32);
        host.writer_low_tx = Some(tx);

        host.send(&HostMsg::Focus, Priority::Low);

        let sent = rx.try_recv().unwrap();
        let decoded: HostMsg = serde_json::from_str(&sent).unwrap();
        assert!(matches!(decoded, HostMsg::Focus));
    }

    #[test]
    fn test_send_sets_crashed_on_channel_disconnect() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (tx, rx) = sync_channel::<String>(8);
        host.writer_high_tx = Some(tx);
        drop(rx); // Disconnect the receiver before sending

        host.send(&HostMsg::Tick, Priority::High);
        assert!(host.crashed);
    }

    #[test]
    fn test_send_does_not_set_crashed_on_full_channel() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        // Sync channel with capacity 1
        let (tx, rx) = sync_channel::<String>(1);
        host.writer_low_tx = Some(sync_channel::<String>(8).0);

        // Fill the channel before moving tx into host
        tx.try_send("fill".into()).unwrap();
        host.writer_high_tx = Some(tx);

        host.send(&HostMsg::Tick, Priority::High);
        // Channel is full, not disconnected — should NOT set crashed
        assert!(!host.crashed);
        // Tidy up
        drop(rx);
    }

    #[test]
    fn test_drain_responses_updates_all_cached_fields() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (tx, rx) = mpsc::channel::<PluginMsg>();
        host.response_rx = Some(rx);

        tx.send(PluginMsg {
            commands: vec![RenderCmd::Clear {
                x: 1,
                y: 2,
                w: 3,
                h: 4,
            }],
            hints: vec![("k".into(), "v".into())],
            palette_commands: vec![("P".into(), "A".into())],
            request: Some(PluginRequest::SignOut),
            consumed: true,
        })
        .unwrap();

        host.drain_responses();

        assert_eq!(host.cached_commands.len(), 1);
        assert_eq!(host.cached_hints.len(), 1);
        assert_eq!(host.cached_palette_commands.len(), 1);
        assert!(host.pending_request.is_some());
        assert!(host.consumed);
        assert!(!host.crashed);
    }

    #[test]
    fn test_drain_responses_takes_latest_message() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (tx, rx) = mpsc::channel::<PluginMsg>();
        host.response_rx = Some(rx);

        // First message — stale
        tx.send(PluginMsg {
            commands: vec![],
            hints: vec![],
            palette_commands: vec![],
            request: None,
            consumed: false,
        })
        .unwrap();

        // Second message — latest
        tx.send(PluginMsg {
            commands: vec![RenderCmd::Text {
                x: 5,
                y: 5,
                text: "latest".into(),
                fg: None,
                bg: None,
                bold: false,
            }],
            hints: vec![("new".into(), "hint".into())],
            palette_commands: vec![],
            request: None,
            consumed: true,
        })
        .unwrap();

        host.drain_responses();

        match &host.cached_commands[0] {
            RenderCmd::Text { text, .. } => assert_eq!(text, "latest"),
            _ => panic!("expected Text command"),
        }
        assert_eq!(host.cached_hints[0], ("new".into(), "hint".into()));
        assert!(host.consumed);
    }

    #[test]
    fn test_drain_responses_sets_crashed_on_disconnect() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (tx, rx) = mpsc::channel::<PluginMsg>();
        host.response_rx = Some(rx);
        drop(tx);

        host.drain_responses();
        assert!(host.crashed);
    }

    #[test]
    fn test_send_recv_sends_and_drains() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (high_tx, high_rx) = sync_channel::<String>(8);
        let (resp_tx, resp_rx) = mpsc::channel::<PluginMsg>();

        host.writer_high_tx = Some(high_tx);
        host.writer_low_tx = Some(sync_channel::<String>(8).0);
        host.response_rx = Some(resp_rx);

        resp_tx
            .send(PluginMsg {
                commands: vec![],
                hints: vec![],
                palette_commands: vec![],
                request: None,
                consumed: true,
            })
            .unwrap();

        host.send_recv(&HostMsg::Focus);

        // Message was sent to the high channel
        let sent: String = high_rx.try_recv().unwrap();
        assert!(matches!(
            serde_json::from_str::<HostMsg>(&sent).unwrap(),
            HostMsg::Focus
        ));

        // Response was drained
        assert!(host.consumed);
    }

    #[test]
    fn test_send_recv_blocking_timeout_receives_response() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (high_tx, _high_rx) = sync_channel::<String>(8);
        let (resp_tx, resp_rx) = mpsc::channel::<PluginMsg>();

        host.writer_high_tx = Some(high_tx);
        host.writer_low_tx = Some(sync_channel::<String>(8).0);
        host.response_rx = Some(resp_rx);

        // Put a stale response that will be drained first
        resp_tx
            .send(PluginMsg {
                commands: vec![],
                hints: vec![],
                palette_commands: vec![],
                request: None,
                consumed: false,
            })
            .unwrap();

        // Put the response for the blocking recv
        resp_tx
            .send(PluginMsg {
                commands: vec![],
                hints: vec![],
                palette_commands: vec![],
                request: None,
                consumed: true,
            })
            .unwrap();

        host.send_recv_blocking_timeout(
            &HostMsg::Key {
                key: IpcKey::Char('q'),
                modifiers: IpcKeyModifiers::default(),
            },
            Duration::from_millis(50),
        );

        // consumed=true from the blocking recv should be preserved
        assert!(host.consumed);
    }

    #[test]
    fn test_send_recv_blocking_timeout_timeout_does_not_set_consumed() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (high_tx, _high_rx) = sync_channel::<String>(8);
        let (_resp_tx, resp_rx) = mpsc::channel::<PluginMsg>();

        host.writer_high_tx = Some(high_tx);
        host.writer_low_tx = Some(sync_channel::<String>(8).0);
        host.response_rx = Some(resp_rx);

        host.consumed = false;

        host.send_recv_blocking_timeout(
            &HostMsg::Key {
                key: IpcKey::Esc,
                modifiers: IpcKeyModifiers::default(),
            },
            Duration::from_millis(10),
        );

        assert!(!host.consumed);
    }

    struct MockAuth {
        user: std::sync::Mutex<Option<User>>,
    }
    impl MockAuth {
        fn new() -> Self {
            MockAuth {
                user: std::sync::Mutex::new(None),
            }
        }
        fn with_user(user: User) -> Self {
            MockAuth {
                user: std::sync::Mutex::new(Some(user)),
            }
        }
    }
    impl AuthHandle for MockAuth {
        fn current_user(&self) -> Option<User> {
            self.user.lock().unwrap().clone()
        }
        fn bearer_token(&self) -> Option<String> {
            None
        }
        fn sign_out(&self) {
            *self.user.lock().unwrap() = None;
        }
        fn sign_in(&self, _provider: &str) -> Result<User, Box<dyn std::error::Error>> {
            self.user
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| "no user".into())
        }
        fn start_sign_in(&self, _provider: &str) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
        fn drain_pending_sign_in(&self) -> Option<Result<User, Box<dyn std::error::Error>>> {
            None
        }
        fn auth_message(&self) -> Option<String> {
            None
        }
    }

    #[test]
    fn test_process_request_sign_in_triggers_user_update() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (high_tx, _high_rx) = sync_channel::<String>(8);
        let (resp_tx, resp_rx) = mpsc::channel::<PluginMsg>();

        host.writer_high_tx = Some(high_tx);
        host.writer_low_tx = Some(sync_channel::<String>(8).0);
        host.response_rx = Some(resp_rx);
        host.pending_request = Some(PluginRequest::SignIn {
            provider: "github".into(),
        });

        let user = User {
            id: "1".into(),
            email: "u@t.com".into(),
            name: "U".into(),
            avatar_url: None,
            provider: "github".into(),
        };

        let auth: Arc<dyn AuthHandle> = Arc::new(MockAuth::with_user(user.clone()));

        // Plugin will respond to UserUpdate with its new state
        resp_tx
            .send(PluginMsg {
                commands: vec![],
                hints: vec![],
                palette_commands: vec![],
                request: None,
                consumed: false,
            })
            .unwrap();

        let changed = host.process_request(&auth);
        assert!(changed);
        assert!(host.pending_request.is_none());
        assert!(!host.crashed);
    }

    #[test]
    fn test_process_request_sign_out_clears_user() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (high_tx, _high_rx) = sync_channel::<String>(8);
        let (resp_tx, resp_rx) = mpsc::channel::<PluginMsg>();

        host.writer_high_tx = Some(high_tx);
        host.writer_low_tx = Some(sync_channel::<String>(8).0);
        host.response_rx = Some(resp_rx);
        host.pending_request = Some(PluginRequest::SignOut);

        resp_tx
            .send(PluginMsg {
                commands: vec![],
                hints: vec![],
                palette_commands: vec![],
                request: None,
                consumed: false,
            })
            .unwrap();

        let auth: Arc<dyn AuthHandle> = Arc::new(MockAuth::new());
        let changed = host.process_request(&auth);
        assert!(changed);
    }

    #[test]
    fn test_process_request_plugins_changed_returns_true() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        host.pending_request = Some(PluginRequest::PluginsChanged);

        let auth: Arc<dyn AuthHandle> = Arc::new(MockAuth::new());
        let changed = host.process_request(&auth);
        assert!(changed);
        assert!(host.pending_request.is_none());
    }

    #[test]
    fn test_process_request_no_pending_returns_false() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let auth: Arc<dyn AuthHandle> = Arc::new(MockAuth::new());
        assert!(!host.process_request(&auth));
    }

    #[test]
    fn test_handle_key_esc_returns_consumed() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        let (high_tx, _high_rx) = sync_channel::<String>(8);
        let (resp_tx, resp_rx) = mpsc::channel::<PluginMsg>();

        host.writer_high_tx = Some(high_tx);
        host.writer_low_tx = Some(sync_channel::<String>(8).0);
        host.response_rx = Some(resp_rx);

        // Plugin responds that it consumed the key
        resp_tx
            .send(PluginMsg {
                commands: vec![],
                hints: vec![],
                palette_commands: vec![],
                request: None,
                consumed: true,
            })
            .unwrap();

        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        );

        let consumed = host.handle_key(key);
        assert!(consumed);
    }

    #[test]
    fn test_handle_key_unrecognized_returns_false() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        // No channels set up — any key that isn't in the match returns false
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::CapsLock,
            crossterm::event::KeyModifiers::NONE,
        );
        assert!(!host.handle_key(key));
    }

    #[test]
    fn test_render_updates_current_area() {
        let mut host = IpcPluginHost::new("id", "name", "bin");
        host.cached_commands = vec![RenderCmd::Clear {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        }];
        assert_eq!(host.current_area.get(), Area { w: 80, h: 24 });
    }
}
