mod app_state;
mod handle_key;
mod palette;
mod palette_controller;
mod plugin_manager;
mod registry;
mod screens;
mod starfield;
mod status_bar;
mod theme_manager;

use crate::auth::AuthHandle;
use crate::config::ConfigManager;
use crate::db_access::DbAccess;
use crate::logger::LoggerBuffer;
use crate::plugin::{Plugin, PluginContext};
use crate::sync::SyncClient;
use crate::widgets::DimOverlay;
use crossterm::event::{
    DisableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEvent,
};
use crossterm::execute;
use crossterm::terminal::enable_raw_mode;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;
use ratatui::Frame;
use ratatui::Terminal;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Set by the `ctrlc` signal handler when SIGINT/SIGTERM/SIGHUP is received.
static SIGINT: AtomicBool = AtomicBool::new(false);

/// Identifier for a built-in palette command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BuiltinId {
    SignInGoogle,
    SignInGitHub,
    SignOut,
    PluginRegistry,
    SwitchTheme,
    About,
    Exit,
}

/// Return the canonical list of built-in command definitions.
/// Each entry is `(id, category, label)`.
pub(super) fn all_builtins() -> Vec<(BuiltinId, &'static str, &'static str)> {
    vec![
        (BuiltinId::SignInGoogle, "Auth", "Sign in with Google"),
        (BuiltinId::SignInGitHub, "Auth", "Sign in with GitHub"),
        (BuiltinId::SignOut, "Auth", "Sign out"),
        (BuiltinId::PluginRegistry, "System", "Plugin registry"),
        (BuiltinId::SwitchTheme, "System", "Switch theme"),
        (BuiltinId::About, "System", "About"),
        (BuiltinId::Exit, "System", "Exit"),
    ]
}

/// Index into either built-in, dynamic (registry), or plugin-registered items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemIndex {
    Builtin(usize),
    Dynamic(usize),
    PluginCmd(usize),
}

/// Parse a hex colour string like `"#ff8800"` or `"ff8800"` into a `Color::Rgb`.
pub(super) fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let val = u32::from_str_radix(s, 16).ok()?;
    Some(Color::Rgb(
        ((val >> 16) & 0xFF) as u8,
        ((val >> 8) & 0xFF) as u8,
        (val & 0xFF) as u8,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_hex tests ----

    #[test]
    fn parse_hex_valid_with_hash() {
        assert_eq!(parse_hex("#ff8800"), Some(Color::Rgb(255, 136, 0)));
    }

    #[test]
    fn parse_hex_valid_without_hash() {
        assert_eq!(parse_hex("ff8800"), Some(Color::Rgb(255, 136, 0)));
    }

    #[test]
    fn parse_hex_all_zeros() {
        assert_eq!(parse_hex("#000000"), Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn parse_hex_all_fs() {
        assert_eq!(parse_hex("#ffffff"), Some(Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn parse_hex_mixed_case() {
        assert_eq!(parse_hex("#Ff8800"), Some(Color::Rgb(255, 136, 0)));
    }

    #[test]
    fn parse_hex_uppercase() {
        assert_eq!(parse_hex("#FF8800"), Some(Color::Rgb(255, 136, 0)));
    }

    #[test]
    fn parse_hex_invalid_chars_returns_none() {
        assert_eq!(parse_hex("#gggggg"), None);
    }

    #[test]
    fn parse_hex_too_short_returns_none() {
        assert_eq!(parse_hex("#fff"), None);
    }

    #[test]
    fn parse_hex_too_long_returns_none() {
        assert_eq!(parse_hex("#ff8800ff"), None);
    }

    #[test]
    fn parse_hex_empty_string_returns_none() {
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn parse_hex_just_hash_returns_none() {
        assert_eq!(parse_hex("#"), None);
    }

    #[test]
    fn parse_hex_double_hash_returns_some() {
        // trim_start_matches('#') strips ALL leading #s, so "##ff8800" → "ff8800" (len 6) → valid!
        assert_eq!(parse_hex("##ff8800"), Some(Color::Rgb(255, 136, 0)));
    }

    #[test]
    fn parse_hex_hash_only_returns_none() {
        // "##" → strips both #s → "" → len 0 ≠ 6
        assert_eq!(parse_hex("##"), None);
    }

    #[test]
    fn parse_hex_hash_in_middle_returns_none() {
        // "ff88#00" — no leading # to strip, len 7 ≠ 6
        assert_eq!(parse_hex("ff88#00"), None);
    }
}

pub struct Santui {
    /// All plugin lifecycle management.
    pub(super) plugin_manager: plugin_manager::PluginManager,
    /// In-app event bus for decoupled communication.
    pub(super) event_bus: crate::event::EventBus,
    /// Authentication handle (set by main.rs before run()).
    pub(super) auth: Option<Arc<dyn AuthHandle>>,
    /// Sync client for pushing data to a remote santui-server.
    pub(super) sync: Option<Arc<SyncClient>>,
    /// Central database access for plugin user data.
    pub(super) db: Option<Box<dyn DbAccess>>,
    /// Centralized application state.
    pub(super) app_state: app_state::AppState,
    /// Manages theme selection, preview, and theme-picker UI state.
    pub(super) theme_manager: theme_manager::ThemeManager,
    /// Command palette overlay state and key handling.
    palette_controller: palette_controller::PaletteController,
    /// Hot-reloadable configuration manager.
    pub(super) config_manager: crate::config::ConfigManager,
    /// Resolved key bindings (parsed from config at startup and on hot-reload).
    pub(super) bindings: ResolvedBindings,
    /// Starfield background animation.
    pub(super) starfield: starfield::Starfield,
    /// Cached terminal height, updated on resize.
    term_h: u16,
    /// Runtime log buffer installed as the global `log::Log`. Set by main.rs
    /// before `run()`. Forwarded to plugins that declare `log-consumer` capability.
    pub(super) log_buffer: Option<Arc<LoggerBuffer>>,
}

/// Parsed key bindings used at runtime, resolved once from [`Config`](crate::config::Config).
#[derive(Debug, Clone, Copy)]
pub(super) struct ResolvedBindings {
    pub open_palette: (KeyCode, KeyModifiers),
    pub quit: (KeyCode, KeyModifiers),
    pub about: (KeyCode, KeyModifiers),
}

impl ResolvedBindings {
    fn from_config(kb: &crate::config::KeyBindings) -> Self {
        Self {
            open_palette: parse_binding(
                &kb.open_palette,
                KeyCode::Char('p'),
                KeyModifiers::CONTROL,
            ),
            quit: parse_binding(&kb.quit, KeyCode::Char('q'), KeyModifiers::NONE),
            about: parse_binding(&kb.about, KeyCode::Char('?'), KeyModifiers::NONE),
        }
    }
}

fn parse_binding(
    s: &str,
    default_code: KeyCode,
    default_mods: KeyModifiers,
) -> (KeyCode, KeyModifiers) {
    crate::config::KeyBindings::parse_key(s).unwrap_or((default_code, default_mods))
}

impl Default for Santui {
    fn default() -> Self {
        Self::new()
    }
}

impl Santui {
    pub fn new() -> Self {
        let theme_manager = theme_manager::ThemeManager::new();
        let theme = theme_manager.current().clone();
        Santui {
            plugin_manager: plugin_manager::PluginManager::new(),
            event_bus: crate::event::EventBus::new(),
            auth: None,
            sync: None,
            db: None,
            app_state: app_state::AppState::new(theme, true),
            theme_manager,
            palette_controller: palette_controller::PaletteController::new(),
            config_manager: ConfigManager::new(std::path::PathBuf::new()),
            bindings: ResolvedBindings::from_config(&crate::config::KeyBindings::default()),
            starfield: starfield::Starfield::new(),
            term_h: 24,
            log_buffer: None,
        }
    }

    /// Set the main loop tick rate (default 100ms).
    /// Lower values = smoother animation but more CPU.
    pub fn set_tick_rate(&mut self, duration: Duration) {
        self.config_manager.set_tick_rate(duration);
    }

    pub fn set_auth(&mut self, auth: Arc<dyn AuthHandle>) {
        self.auth = Some(auth);
    }

    /// Set the central database access for plugins.
    /// Call before `run()`.
    pub fn set_db(&mut self, db: Box<dyn DbAccess>) {
        self.db = Some(db);
    }

    /// Set the sync client for pushing data to a remote santui-server.
    /// Call before `run()`.  Reads `server.url` from config to determine
    /// whether sync is enabled.
    pub fn set_sync(&mut self, sync: Option<Arc<SyncClient>>) {
        self.sync = sync;
    }

    /// Set the runtime log buffer. Call before `run()`. The buffer is
    /// forwarded to plugins that declare the `log-consumer` capability.
    pub fn set_log_buffer(&mut self, buf: Arc<LoggerBuffer>) {
        self.log_buffer = Some(buf);
    }

    /// Toggle mouse capture on/off at runtime.
    /// When disabled, terminal text selection works without Shift.
    /// Call after entering alternate screen / raw mode.
    pub fn set_mouse_capture(&mut self, enabled: bool) {
        use crossterm::QueueableCommand;
        self.app_state.mouse_capture = enabled;
        let mut stdout = std::io::stdout();
        if enabled {
            let _ = stdout.queue(crossterm::event::EnableMouseCapture);
        } else {
            let _ = stdout.queue(crossterm::event::DisableMouseCapture);
        }
        let _ = stdout.flush();
    }

    /// Set mouse capture flag without applying it (used before run()).
    /// The actual crossterm call happens at startup inside run().
    pub fn set_mouse_capture_startup(&mut self, enabled: bool) {
        self.app_state.mouse_capture = enabled;
    }

    /// Set the config directory and load (or create) `config.toml`.
    /// Call before `run()`.
    pub fn set_config_dir(&mut self, dir: std::path::PathBuf) {
        self.config_manager = ConfigManager::new(dir.clone());
        self.theme_manager.load_user_themes(&dir);
        self.apply_config();
    }

    /// Access the currently loaded configuration.
    pub fn config(&self) -> &crate::config::Config {
        self.config_manager.config()
    }

    /// Apply the loaded config (theme, custom colors) to the current app state.
    pub(super) fn apply_config(&mut self) {
        // Apply default theme if specified (borrow config_manager, then drop before mutate).
        let theme_idx = self
            .config_manager
            .config()
            .theme
            .as_ref()
            .and_then(|theme_name| {
                let lower = theme_name.to_lowercase();
                self.theme_manager
                    .themes
                    .iter()
                    .position(|(n, _)| n.to_lowercase() == lower)
            });
        if let Some(idx) = theme_idx {
            self.select_theme(idx);
        } else if let Some(theme_name) = &self.config_manager.config().theme {
            log::warn!("[config] unknown theme '{theme_name}'");
        }

        // Apply custom color overrides.
        if let Some(custom) = &self.config_manager.config().custom_theme {
            let mut t = self.app_state.theme.clone();
            macro_rules! apply_color {
                ($field:ident, $v:expr, $name:literal) => {
                    if let Some(c) = parse_hex($v) {
                        t.$field = c;
                    } else {
                        log::warn!(
                            "[config] invalid hex colour '{}' for custom_theme.{}",
                            $v,
                            $name
                        );
                    }
                };
            }
            if let Some(ref v) = custom.accent {
                apply_color!(accent, v, "accent");
            }
            if let Some(ref v) = custom.highlight {
                apply_color!(highlight, v, "highlight");
            }
            if let Some(ref v) = custom.logo {
                apply_color!(logo, v, "logo");
            }
            if let Some(ref v) = custom.text {
                apply_color!(text, v, "text");
            }
            if let Some(ref v) = custom.text_muted {
                apply_color!(text_muted, v, "text_muted");
            }
            if let Some(ref v) = custom.background {
                apply_color!(background, v, "background");
            }
            if let Some(ref v) = custom.background_panel {
                apply_color!(background_panel, v, "background_panel");
            }
            if let Some(ref v) = custom.background_overlay {
                apply_color!(background_overlay, v, "background_overlay");
            }
            if let Some(ref v) = custom.border {
                apply_color!(border, v, "border");
            }
            if let Some(ref v) = custom.success {
                apply_color!(success, v, "success");
            }
            if let Some(ref v) = custom.error {
                apply_color!(error, v, "error");
            }
            if let Some(ref v) = custom.inverted_text {
                apply_color!(inverted_text, v, "inverted_text");
            }
            self.event_bus.emit(crate::event::Event::ThemeChanged(t));
        }

        self.app_state.mouse_capture = self.config_manager.config().mouse_capture;
        self.bindings = ResolvedBindings::from_config(&self.config_manager.config().keybindings);
        self.config_manager.ack();
    }

    /// Get the currently selected theme name.
    pub fn current_theme_name(&self) -> &str {
        &self.theme_manager.themes[self.theme_manager.current_idx].0
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin + Send>) {
        self.plugin_manager.register(plugin);
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset and install OS-level signal handler (SIGINT/SIGTERM/SIGHUP on Unix,
        // CTRL_C_EVENT/CTRL_BREAK_EVENT on Windows). In raw mode, keyboard Ctrl+C
        // passes through as a key event (handled in handle_key), so this catches
        // external signals like `kill` or system shutdown.
        SIGINT.store(false, Ordering::SeqCst);
        ctrlc::set_handler(|| SIGINT.store(true, Ordering::SeqCst))?;

        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        self.set_mouse_capture(self.app_state.mouse_capture);
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        // Restore terminal on panic so user doesn't get stuck in raw mode.
        struct TerminalGuard;
        impl Drop for TerminalGuard {
            fn drop(&mut self) {
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    crossterm::terminal::LeaveAlternateScreen,
                    DisableMouseCapture,
                    crossterm::cursor::Show,
                );
            }
        }
        let _guard = TerminalGuard;

        // Resize starfield to match actual terminal dimensions.
        let (term_w, term_h) = crossterm::terminal::size()?;
        self.term_h = term_h;
        self.starfield.resize(term_w, term_h);

        let mut ctx = PluginContext {
            theme: self.app_state.theme.clone(),
            auth: self.auth.clone(),
            data_dir: self.plugin_manager.data_dir().to_path_buf(),
            log_buffer: self.log_buffer.clone(),
        };
        self.plugin_manager.init_all(&mut ctx)?;

        // Process any pending requests from init responses (e.g. DbGet).
        if let Some(ref mut db) = self.db {
            self.plugin_manager
                .process_all_requests(db.as_mut(), &self.auth);
        }

        // Populate palette "Plugins" category from registry.toml.
        self.plugin_manager.read_registry_installed();

        while self.app_state.running {
            // Check for external signals (SIGINT/SIGTERM via ctrlc handler).
            if SIGINT.load(Ordering::SeqCst) {
                self.app_state.running = false;
                break;
            }
            self.plugin_manager.tick_all();

            // Process pending plugin requests (DB read/write, auth, etc.).
            if let Some(ref mut db) = self.db {
                self.plugin_manager
                    .process_all_requests(db.as_mut(), &self.auth);
            }

            // Push queued writes to the remote sync server (best-effort).
            if let Some(ref sync) = self.sync {
                sync.try_sync(&self.auth);
            }

            // Event-driven Esc: if the active plugin had a pending Esc response
            // resolved on this tick, close the plugin if it was not consumed.
            if let Some(idx) = self.plugin_manager.active() {
                if let Some(consumed) = self.plugin_manager.drain_esc_result(idx) {
                    if !consumed {
                        self.plugin_manager.shutdown_and_remove(idx);
                        self.app_state.home_selected = None;
                    }
                }
            }

            // Poll for config changes (hot-reload).
            self.config_manager.poll();
            if self.config_manager.dirty {
                self.apply_config();
                ctx.theme = self.app_state.theme.clone();
            }

            // Check for plugin binary updates (hot-reload).
            self.plugin_manager.check_reloads(&mut ctx);

            // Poll registry.toml for changes (registry plugin writes it).
            if self.plugin_manager.poll_registry_installed() {
                self.palette_controller.invalidate();
            }

            // Drain the event bus and forward events to subsystems.
            let events = self.event_bus.drain();
            self.app_state.process_events(&events);
            self.plugin_manager.process_events(&events);

            // Check for pending non-blocking sign-in results.
            if let Some(ref auth) = self.auth {
                if let Some(result) = auth.drain_pending_sign_in() {
                    match result {
                        Ok(user) => {
                            self.plugin_manager.on_user_update_all(Some(&user));
                        }
                        Err(e) => {
                            log::error!("[auth] background sign-in error: {e}");
                        }
                    }
                }
            }

            terminal.draw(|f| self.render(f))?;

            if crossterm::event::poll(self.config_manager.tick_rate())? {
                match crossterm::event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        self.handle_key(key);

                        // Process pending plugin requests from key responses
                        // (e.g. DbSet on Space press).
                        if let Some(ref mut db) = self.db {
                            self.plugin_manager
                                .process_all_requests(db.as_mut(), &self.auth);
                        }
                    }
                    Event::Mouse(mouse) if self.app_state.mouse_capture => {
                        self.handle_mouse(mouse);
                    }
                    Event::Mouse(_) => {
                        // Mouse capture disabled — events are swallowed
                        // so they don't interfere with text selection.
                    }
                    _ => {}
                }
            }

            // Activate any plugins requested to launch (e.g. from registry plugin).
            for (id, name) in self.plugin_manager.drain_pending_launches() {
                self.activate_plugin_by_id(&id, &name);
            }
        }

        Ok(())
    }

    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent) {
        if let Some(idx) = self.plugin_manager.active() {
            if self.plugin_manager.is_ready(idx) {
                self.plugin_manager.handle_mouse(idx, mouse);
            }
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let area = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        match self.plugin_manager.active() {
            None => {
                if self.app_state.show_about {
                    self.render_about(f, chunks[0]);
                } else {
                    self.render_splash(f, chunks[0]);
                }
            }
            Some(idx) => {
                if self.plugin_manager.is_ready(idx) {
                    self.plugin_manager.render(idx, f, chunks[0]);
                } else {
                    let name = self
                        .plugin_manager
                        .plugin_name(idx)
                        .unwrap_or("plugin")
                        .to_owned();
                    self.render_loading(f, chunks[0], &name);
                }
            }
        }

        let hints = self
            .plugin_manager
            .active()
            .filter(|&idx| self.plugin_manager.is_ready(idx))
            .map(|idx| self.plugin_manager.status_hints(idx))
            .unwrap_or_default();
        let current_user = self.auth.as_ref().and_then(|a| a.current_user());
        let auth_message = self.auth.as_ref().and_then(|a| a.auth_message());
        status_bar::StatusBar {
            theme: &self.app_state.theme,
            about_open: self.app_state.show_about,
            plugin_active: self.plugin_manager.active().is_some(),
            active_plugin_hints: &hints,
            user: current_user.as_ref(),
            config_error: self.config_manager.error(),
            auth_message: auth_message.as_deref(),
            plugin_errors: self.plugin_manager.crashed_plugins(),
            mouse_capture: self.app_state.mouse_capture,
        }
        .render(f, chunks[1]);

        if self.palette_controller.is_open() || self.app_state.theme_picker_open {
            DimOverlay {
                style: Style::default().bg(self.app_state.theme.background_overlay),
            }
            .render(area, f.buffer_mut());
        } else if self
            .plugin_manager
            .active()
            .is_some_and(|idx| self.plugin_manager.has_dim_overlay(idx))
        {
            // Plugin already dims its own content area; dim the status bar too
            DimOverlay {
                style: Style::default().bg(self.app_state.theme.background_overlay),
            }
            .render(chunks[1], f.buffer_mut());
        }

        if self.palette_controller.is_open() {
            let cmds = self.plugin_manager.commands();
            self.palette_controller.render(
                f,
                chunks[0],
                &self.app_state.theme,
                self.starfield.tick(),
                &self.app_state.builtin_items,
                self.plugin_manager.dynamic_items(),
                cmds,
            );
        }

        if self.app_state.theme_picker_open {
            self.theme_manager.render_picker(
                f,
                chunks[0],
                &self.app_state.theme,
                self.starfield.tick(),
            );
        }
    }
}
