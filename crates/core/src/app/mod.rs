mod app_state;
mod handle_key;
mod palette;
mod palette_controller;
mod palette_widget;
mod plugin_manager;
mod registry;
mod screens;
mod starfield;
mod status_bar;
mod theme_manager;

use crate::auth::AuthHandle;
use crate::config::ConfigManager;
use crate::plugin::{Plugin, PluginContext};
use crossterm::event::{Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::enable_raw_mode;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Color;
use ratatui::Frame;
use ratatui::Terminal;
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
    SwitchTheme,
    About,
}

/// Return the canonical list of built-in command definitions.
/// Each entry is `(id, category, label)`.
pub(super) fn all_builtins() -> Vec<(BuiltinId, &'static str, &'static str)> {
    vec![
        (BuiltinId::SignInGoogle, "Auth", "Sign in with Google"),
        (BuiltinId::SignInGitHub, "Auth", "Sign in with GitHub"),
        (BuiltinId::SignOut, "Auth", "Sign out"),
        (BuiltinId::SwitchTheme, "System", "Switch theme"),
        (BuiltinId::About, "System", "About"),
    ]
}

/// Index into either built-in, dynamic (registry), or plugin-registered items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemIndex {
    Builtin(usize),
    Dynamic(usize),
    PluginCmd(usize),
}

const PAD_L: u16 = 2;
const PAD_T: u16 = 1;
const PAD_B: u16 = 1;
const HEADER_H: u16 = 4;
const PAL_MIN_W: u16 = 30;
const PAL_IDEAL_W: u16 = 60;

pub(super) fn max_list_h(content_h: u16) -> u16 {
    (content_h / 2).saturating_sub(6).max(3)
}

pub(super) fn pal_w(content_w: u16) -> u16 {
    let max = content_w.saturating_sub(2);
    if max < PAL_MIN_W {
        return max;
    }
    max.clamp(PAL_MIN_W, PAL_IDEAL_W)
}

/// Scale the brightness of an RGB foreground color by `factor`,
/// preserving its hue. Non-RGB colors (Reset, Indexed) pass through.
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

fn dim_color(fg: Color, factor: f64) -> Color {
    match fg {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f64 * factor) as u8,
            (g as f64 * factor) as u8,
            (b as f64 * factor) as u8,
        ),
        _ => fg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pal_w_given_small_width_returns_width() {
        let w = pal_w(10);
        assert_eq!(w, 8);
    }

    #[test]
    fn pal_w_returns_content_minus_two_when_above_min() {
        let w = pal_w(40);
        assert_eq!(w, 38);
    }

    #[test]
    fn pal_w_clamps_to_max() {
        let w = pal_w(200);
        assert_eq!(w, 60);
    }

    #[test]
    fn pal_w_zero_saturates() {
        let w = pal_w(0);
        assert_eq!(w, 0);
    }

    #[test]
    fn pal_w_one_saturates() {
        let w = pal_w(1);
        assert_eq!(w, 0);
    }

    #[test]
    fn max_list_h_small_height() {
        let h = max_list_h(10);
        assert_eq!(h, 3);
    }

    #[test]
    fn max_list_h_normal() {
        let h = max_list_h(48);
        assert_eq!(h, 18);
    }

    #[test]
    fn max_list_h_large() {
        let h = max_list_h(100);
        assert_eq!(h, 44);
    }

    #[test]
    fn max_list_h_minimum() {
        let h = max_list_h(4);
        assert_eq!(h, 3);
    }

    #[test]
    fn filtered_themes_empty_query_returns_all() {
        let app = Santui::new();
        let themes = app.theme_manager.filtered();
        assert_eq!(themes.len(), app.theme_manager.themes.len());
    }

    #[test]
    fn filtered_themes_matches_partial() {
        let mut app = Santui::new();
        app.theme_manager.picker_query = "cat".into();
        let themes = app.theme_manager.filtered();
        assert!(themes.len() >= 3);
        for &i in &themes {
            let name = app.theme_manager.themes[i].0.to_lowercase();
            assert!(
                name.contains("cat"),
                "expected '{}' to contain 'cat'",
                app.theme_manager.themes[i].0
            );
        }
    }

    #[test]
    fn filtered_themes_no_match() {
        let mut app = Santui::new();
        app.theme_manager.picker_query = "xyznonexistent".into();
        let themes = app.theme_manager.filtered();
        assert!(themes.is_empty());
    }

    #[test]
    fn filtered_themes_case_insensitive() {
        let mut app = Santui::new();
        app.theme_manager.picker_query = "NORD".into();
        let themes = app.theme_manager.filtered();
        assert_eq!(themes.len(), 1);
        assert_eq!(app.theme_manager.themes[themes[0]].0, "Nord");
    }

    // ---- dim_color tests ----

    #[test]
    fn dim_color_black_stays_black() {
        assert_eq!(dim_color(Color::Rgb(0, 0, 0), 0.45), Color::Rgb(0, 0, 0));
    }

    #[test]
    fn dim_color_white_becomes_gray() {
        assert_eq!(
            dim_color(Color::Rgb(255, 255, 255), 0.45),
            Color::Rgb(114, 114, 114)
        );
    }

    #[test]
    fn dim_color_gold_becomes_dim_gold() {
        // Gold RGB(255, 185, 0) × 0.45 = (114, 83, 0)
        assert_eq!(
            dim_color(Color::Rgb(255, 185, 0), 0.45),
            Color::Rgb(114, 83, 0)
        );
    }

    #[test]
    fn dim_color_full_factor_returns_original() {
        assert_eq!(
            dim_color(Color::Rgb(123, 200, 50), 1.0),
            Color::Rgb(123, 200, 50)
        );
    }

    #[test]
    fn dim_color_zero_factor_returns_black() {
        assert_eq!(
            dim_color(Color::Rgb(100, 150, 200), 0.0),
            Color::Rgb(0, 0, 0)
        );
    }

    #[test]
    fn dim_color_preserves_hue() {
        // Dimming preserves the ratio between channels
        let dimmed = dim_color(Color::Rgb(200, 100, 50), 0.5);
        assert_eq!(dimmed, Color::Rgb(100, 50, 25));
    }

    #[test]
    fn dim_color_reset_passes_through() {
        assert_eq!(dim_color(Color::Reset, 0.45), Color::Reset);
    }

    #[test]
    fn dim_color_indexed_passes_through() {
        assert_eq!(dim_color(Color::Indexed(7), 0.45), Color::Indexed(7));
    }

    // ---- dim_color edge cases ----

    #[test]
    fn dim_color_factor_above_one_saturates_at_255() {
        // Rust's f64→u8 cast saturates: 400.0 as u8 = 255, not 144.
        let result = dim_color(Color::Rgb(200, 100, 50), 2.0);
        // 200*2=400 → 255, 100*2=200, 50*2=100
        assert_eq!(result, Color::Rgb(255, 200, 100));
    }

    #[test]
    fn dim_color_negative_factor_saturates_to_zero() {
        // Rust defines f64→u8 cast as saturating at 0.
        assert_eq!(
            dim_color(Color::Rgb(100, 150, 200), -0.5),
            Color::Rgb(0, 0, 0)
        );
    }

    #[test]
    fn dim_color_tiny_factor_is_effectively_zero() {
        // 1e-12 * 255 < 1, so all channels truncate to 0.
        assert_eq!(
            dim_color(Color::Rgb(200, 150, 100), 1e-12),
            Color::Rgb(0, 0, 0)
        );
    }

    #[test]
    fn dim_color_nan_factor_does_not_panic() {
        // f64::NAN * integer → NaN, (NaN) as u8 = 0 (saturating cast).
        let result = dim_color(Color::Rgb(100, 150, 200), f64::NAN);
        assert_eq!(result, Color::Rgb(0, 0, 0));
    }

    #[test]
    fn dim_color_inf_factor_does_not_panic() {
        // Rust's f64→u8 cast: ∞ saturates to 255, not 0.
        let result = dim_color(Color::Rgb(100, 150, 200), f64::INFINITY);
        assert_eq!(result, Color::Rgb(255, 255, 255));
    }

    #[test]
    fn dim_color_indexed_196_passes_through() {
        // Indexed 196 = bright red in 256-color ANSI palette
        assert_eq!(dim_color(Color::Indexed(196), 0.45), Color::Indexed(196));
    }

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

    // ---- proptest: dim_color ----

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn dim_color_identity(
            r in any::<u8>(),
            g in any::<u8>(),
            b in any::<u8>(),
        ) {
            let color = Color::Rgb(r, g, b);
            prop_assert_eq!(dim_color(color, 1.0), color);
        }

        #[test]
        fn dim_color_zero_factor_yields_black(
            r in any::<u8>(),
            g in any::<u8>(),
            b in any::<u8>(),
        ) {
            let result = dim_color(Color::Rgb(r, g, b), 0.0);
            prop_assert_eq!(result, Color::Rgb(0, 0, 0));
        }

        #[test]
        fn dim_color_non_rgb_passthrough(
            f in prop::num::f64::ANY,
        ) {
            // Non-RGB colours should never be modified, regardless of factor.
            prop_assert_eq!(dim_color(Color::Reset, f), Color::Reset);
            prop_assert_eq!(dim_color(Color::Indexed(123), f), Color::Indexed(123));
        }

        #[test]
        fn dim_color_channel_never_exceeds_original(
            r in any::<u8>(),
            g in any::<u8>(),
            b in any::<u8>(),
            f in 0.0f64..=1.0f64,
        ) {
            let result = dim_color(Color::Rgb(r, g, b), f);
            if let Color::Rgb(r2, g2, b2) = result {
                prop_assert!(r2 <= r, "red {} > original {}", r2, r);
                prop_assert!(g2 <= g, "green {} > original {}", g2, g);
                prop_assert!(b2 <= b, "blue {} > original {}", b2, b);
            } else {
                panic!("expected Rgb, got {:?}", result);
            }
        }

        #[test]
        fn dim_color_hue_preserved_within_tolerance(
            r in any::<u8>(),
            g in any::<u8>(),
            b in any::<u8>(),
            f in 0.0f64..=1.0f64,
        ) {
            let result = dim_color(Color::Rgb(r, g, b), f);
            if let Color::Rgb(r2, g2, b2) = result {
                // Cross-multiplication to check channel ratios are preserved.
                // Each channel has f64→u8 truncation error < 1, so:
                //   |e2*r - e1*g| ≤ max(r, g)  where e1,e2 ∈ [0,1)
                // This is a tight bound with no false negatives.
                if r != 0 && g != 0 && r2 != 0 && g2 != 0 {
                    let diff = (r2 as i32 * g as i32 - g2 as i32 * r as i32)
                        .unsigned_abs();
                    prop_assert!(diff <= r.max(g) as u32,
                        "r:g ratio not preserved: {}:{} vs {}:{} (f={})",
                        r, g, r2, g2, f);
                }
                if r != 0 && b != 0 && r2 != 0 && b2 != 0 {
                    let diff = (r2 as i32 * b as i32 - b2 as i32 * r as i32)
                        .unsigned_abs();
                    prop_assert!(diff <= r.max(b) as u32,
                        "r:b ratio not preserved: {}:{} vs {}:{} (f={})",
                        r, b, r2, b2, f);
                }
            }
        }
    }
}

pub struct Santui {
    /// All plugin lifecycle management.
    pub(super) plugin_manager: plugin_manager::PluginManager,
    /// In-app event bus for decoupled communication.
    pub(super) event_bus: crate::event::EventBus,
    /// Authentication handle (set by main.rs before run()).
    pub(super) auth: Option<Arc<dyn AuthHandle>>,
    /// Centralized application state.
    pub(super) app_state: app_state::AppState,
    /// Manages theme selection, preview, and theme-picker UI state.
    pub(super) theme_manager: theme_manager::ThemeManager,
    /// Command palette overlay state and key handling.
    palette_controller: palette_controller::PaletteController,
    /// Hot-reloadable configuration manager.
    pub(super) config_manager: crate::config::ConfigManager,
    /// Starfield background animation.
    pub(super) starfield: starfield::Starfield,
    /// Pre-built splash logo lines, invalidated on theme change.
    cached_logo: Option<Vec<ratatui::text::Line<'static>>>,
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
            app_state: app_state::AppState::new(theme),
            theme_manager,
            palette_controller: palette_controller::PaletteController::new(),
            config_manager: ConfigManager::new(std::path::PathBuf::new()),
            starfield: starfield::Starfield::new(),
            cached_logo: None,
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

    /// Set the config directory and load (or create) `config.toml`.
    /// Call before `run()`.
    pub fn set_config_dir(&mut self, dir: std::path::PathBuf) {
        self.config_manager = ConfigManager::new(dir.clone());
        self.theme_manager.load_user_themes(&dir);
        self.apply_config();
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
        }

        // Apply custom color overrides.
        if let Some(custom) = &self.config_manager.config().custom_theme {
            let mut t = self.app_state.theme.clone();
            if let Some(ref v) = custom.accent {
                if let Some(c) = parse_hex(v) {
                    t.accent = c;
                }
            }
            if let Some(ref v) = custom.highlight {
                if let Some(c) = parse_hex(v) {
                    t.highlight = c;
                }
            }
            if let Some(ref v) = custom.logo {
                if let Some(c) = parse_hex(v) {
                    t.logo = c;
                }
            }
            if let Some(ref v) = custom.text {
                if let Some(c) = parse_hex(v) {
                    t.text = c;
                }
            }
            if let Some(ref v) = custom.text_muted {
                if let Some(c) = parse_hex(v) {
                    t.text_muted = c;
                }
            }
            if let Some(ref v) = custom.background {
                if let Some(c) = parse_hex(v) {
                    t.background = c;
                }
            }
            if let Some(ref v) = custom.background_panel {
                if let Some(c) = parse_hex(v) {
                    t.background_panel = c;
                }
            }
            if let Some(ref v) = custom.background_overlay {
                if let Some(c) = parse_hex(v) {
                    t.background_overlay = c;
                }
            }
            if let Some(ref v) = custom.border {
                if let Some(c) = parse_hex(v) {
                    t.border = c;
                }
            }
            if let Some(ref v) = custom.success {
                if let Some(c) = parse_hex(v) {
                    t.success = c;
                }
            }
            if let Some(ref v) = custom.error {
                if let Some(c) = parse_hex(v) {
                    t.error = c;
                }
            }
            if let Some(ref v) = custom.inverted_text {
                if let Some(c) = parse_hex(v) {
                    t.inverted_text = c;
                }
            }
            self.event_bus.emit(crate::event::Event::ThemeChanged(t));
        }

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
                    crossterm::cursor::Show,
                );
            }
        }
        let _guard = TerminalGuard;

        // Resize starfield to match actual terminal dimensions.
        let (term_w, term_h) = crossterm::terminal::size()?;
        self.starfield.resize(term_w, term_h);

        let mut ctx = PluginContext {
            theme: self.app_state.theme.clone(),
            auth: self.auth.clone(),
            data_dir: self.plugin_manager.data_dir().to_path_buf(),
        };
        self.plugin_manager.init_all(&mut ctx)?;

        // Populate palette "Plugins" category from registry.toml.
        self.plugin_manager.read_registry_installed();

        while self.app_state.running {
            // Check for external signals (SIGINT/SIGTERM via ctrlc handler).
            if SIGINT.load(Ordering::SeqCst) {
                self.app_state.running = false;
                break;
            }
            self.plugin_manager.tick_all();

            // Poll for config changes (hot-reload).
            self.config_manager.poll();
            if self.config_manager.dirty {
                self.apply_config();
                ctx.theme = self.app_state.theme.clone();
            }

            // Check for plugin binary updates (hot-reload).
            self.plugin_manager.check_reloads(&mut ctx);

            // Poll registry.toml for changes (registry plugin writes it).
            self.plugin_manager.poll_registry_installed();

            // Drain the event bus and forward events to subsystems.
            let events = self.event_bus.drain();
            if events
                .iter()
                .any(|e| matches!(e, crate::event::Event::ThemeChanged(_)))
            {
                self.cached_logo = None;
            }
            self.app_state.process_events(&events);
            self.plugin_manager.process_events(&events);

            // Check for pending non-blocking sign-in results.
            if let Some(ref auth) = self.auth {
                if let Some(result) = auth.drain_pending_sign_in() {
                    match result {
                        Ok(user) => {
                            self.plugin_manager.on_user_update_all(Some(&user));
                            self.event_bus.emit(crate::event::Event::UserUpdated);
                        }
                        Err(e) => {
                            log::error!("[auth] background sign-in error: {e}");
                        }
                    }
                }
            }

            self.starfield.tick = self.starfield.tick.wrapping_add(1);
            self.starfield.update();

            terminal.draw(|f| self.render(f))?;

            if crossterm::event::poll(self.config_manager.tick_rate())? {
                if let Event::Key(key) = crossterm::event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    self.handle_key(key);
                }
            }
        }

        Ok(())
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
                self.plugin_manager.render(idx, f, chunks[0]);
            }
        }

        let hints = self
            .plugin_manager
            .active()
            .map(|idx| self.plugin_manager.status_hints(idx))
            .unwrap_or_default();
        let current_user = self.auth.as_ref().and_then(|a| a.current_user());
        let auth_message = self.auth.as_ref().and_then(|a| a.auth_message());
        status_bar::StatusBar {
            theme: &self.app_state.theme,
            palette_open: self.palette_controller.is_open(),
            theme_picker_open: self.app_state.theme_picker_open,
            about_open: self.app_state.show_about,
            plugin_active: self.plugin_manager.active().is_some(),
            active_plugin_hints: &hints,
            user: current_user.as_ref(),
            config_error: self.config_manager.error(),
            auth_message: auth_message.as_deref(),
            plugin_errors: self.plugin_manager.crashed_plugins(),
        }
        .render(f, chunks[1]);

        if self.palette_controller.is_open() || self.app_state.theme_picker_open {
            let dim_bg = self.app_state.theme.background_overlay;
            let buf = f.buffer_mut();
            const DIM: f64 = 0.45;
            for y in chunks[0].top()..chunks[0].bottom() {
                for x in chunks[0].left()..chunks[0].right() {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        let mut style = cell.style();
                        if let Some(fg) = style.fg {
                            style.fg = Some(dim_color(fg, DIM));
                        }
                        if let Some(bg) = style.bg {
                            style.bg = Some(dim_color(bg, DIM));
                        } else {
                            style.bg = Some(dim_bg);
                        }
                        cell.set_style(style);
                    }
                }
            }
        }

        if self.palette_controller.is_open() {
            let cmds = self.plugin_manager.commands();
            self.palette_controller.render(
                f,
                chunks[0],
                &self.app_state.theme,
                self.starfield.tick,
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
                self.starfield.tick,
            );
        }
    }
}
