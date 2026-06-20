mod handle_key;
mod palette;
mod palette_widget;
mod plugin_manager;
mod registry;
mod registry_screen;
mod screens;
mod status_bar;
mod theme_manager;

use crate::config::ConfigManager;
use crate::plugin::{Plugin, PluginContext};
use crate::theme::Theme;
use crossterm::event::{Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Color;
use ratatui::Frame;
use ratatui::Terminal;
use santui_registry::Registry as PluginRegistry;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const STAR_COUNT: usize = 88;
const SHOOTING_LIFETIME: u64 = 50;
const SHOOTING_COOLDOWN: u64 = 180;
const COMET_LIFETIME: u64 = 100;
const COMET_COOLDOWN: u64 = 500;
pub(super) struct CmdItem {
    pub(super) category: &'static str,
    pub(super) label: &'static str,
}

/// Index into either the built-in CMD_ITEMS, dynamic plugin items, or plugin-registered commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemIndex {
    Builtin(usize),
    Dynamic(usize),
    PluginCmd(usize),
}

const CMD_ITEMS: &[CmdItem] = &[
    CmdItem {
        category: "Auth",
        label: "Sign in with Google",
    },
    CmdItem {
        category: "Auth",
        label: "Sign in with GitHub",
    },
    CmdItem {
        category: "Auth",
        label: "Sign out",
    },
    CmdItem {
        category: "System",
        label: "Plugin registry",
    },
    CmdItem {
        category: "System",
        label: "Switch theme",
    },
    CmdItem {
        category: "System",
        label: "About",
    },
];

struct Star {
    x: u16,
    y: u16,
    phase: u16,
    mag: u8,
    freq: u16,
    tint: u8,
}

struct ShootingStar {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    age: u64,
    kind: u8,
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
}

pub struct Santui {
    /// All plugin lifecycle management.
    pub(super) plugin_manager: plugin_manager::PluginManager,
    /// In-app event bus for decoupled communication.
    pub(super) event_bus: crate::event::EventBus,
    ctx: PluginContext,
    registry: Option<PluginRegistry>,
    theme: Theme,
    /// Manages theme selection, preview, and theme-picker UI state.
    pub(super) theme_manager: theme_manager::ThemeManager,
    palette: Option<palette_widget::PaletteWidget>,
    /// Registry screen state.
    pub(super) registry_screen: registry_screen::RegistryScreen,
    /// Hot-reloadable configuration manager.
    pub(super) config_manager: crate::config::ConfigManager,
    show_about: bool,
    /// Dynamic palette items from enabled registry plugins: (category, plugin_id, name).
    pub(super) dynamic_items: Vec<(String, String, String)>,
    /// Factory to create a Box<dyn Plugin> from (id, name, binary_path).
    /// Set by main.rs before run().
    pub(super) plugin_factory: Option<crate::plugin::PluginFactory>,
    running: bool,
    tick: u64,
    stars: Vec<Star>,
    shooting: Option<ShootingStar>,
    shooting_cooldown: u64,
}

impl Default for Santui {
    fn default() -> Self {
        Self::new()
    }
}

impl Santui {
    pub fn new() -> Self {
        let theme_manager = theme_manager::ThemeManager::new();
        let theme = theme_manager.current.clone();
        Santui {
            plugin_manager: plugin_manager::PluginManager::new(),
            event_bus: crate::event::EventBus::new(),
            ctx: PluginContext::new(),
            registry: None,
            theme,
            theme_manager,
            palette: None,
            registry_screen: registry_screen::RegistryScreen::new(),
            config_manager: ConfigManager::new(std::path::PathBuf::new()),
            show_about: false,
            dynamic_items: Vec::new(),
            plugin_factory: None,
            running: true,
            tick: 0,
            stars: {
                let mut s = Vec::with_capacity(STAR_COUNT);
                let mut h = 0x9e3779b97f4a7c15u64;
                for _ in 0..STAR_COUNT {
                    h = h
                        .wrapping_mul(0x5851f42d4c957f2d)
                        .wrapping_add(0x14057b7ef767814f);
                    let a = h >> 32;
                    let b = h >> 16;
                    let c = h;
                    s.push(Star {
                        x: (a % 1009) as u16,
                        y: (b % 1009) as u16,
                        phase: (c % 628) as u16,
                        mag: {
                            let m = ((c >> 8) & 0xff) as u8;
                            if m < 100 {
                                0
                            } else if m < 200 {
                                1
                            } else if m < 240 {
                                2
                            } else {
                                3
                            }
                        },
                        freq: (4 + ((c >> 12) & 0x3f)) as u16,
                        tint: (c >> 20 & 0xff) as u8,
                    });
                }
                s
            },
            shooting: None,
            shooting_cooldown: 0,
        }
    }

    /// Rebuild dynamic palette items from the plugin registry.
    /// Call this whenever a plugin is installed or toggled.
    pub(super) fn refresh_dynamic_items(&mut self) {
        self.dynamic_items.clear();
        if let Some(ref reg) = self.registry {
            for plugin in &reg.available {
                let enabled = reg.installed.iter().any(|p| {
                    p.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim_end_matches(".exe"))
                        == Some(&plugin.id)
                        && p.enabled
                });
                if enabled {
                    self.dynamic_items.push((
                        "Modules".into(),
                        plugin.id.clone(),
                        plugin.name.clone(),
                    ));
                }
            }
        }
    }

    fn update_stars(&mut self) {
        let n = (self.tick ^ 0xdeadbeef)
            .wrapping_mul(1103515245)
            .wrapping_add(12345);
        let r = n >> 16;
        self.shooting_cooldown = self.shooting_cooldown.saturating_sub(1);
        let kind = if (r & 0x80) == 0 { 0 } else { 1 };
        if self.shooting.is_none() && self.shooting_cooldown == 0 && (r & 0x3f) < 6 {
            let (speed, max_extra) = if kind == 0 { (1.0, 1.2) } else { (0.2, 0.4) };
            let side = r & 3;
            let (x, y, dx, dy) = match side {
                0 => (
                    0.0,
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    speed + ((r >> 12 & 0x7f) as f64 / 256.0) * max_extra,
                    speed * 0.6 + ((r >> 19 & 0x7f) as f64 / 256.0) * max_extra,
                ),
                1 => (
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    0.0,
                    speed * 0.5 + ((r >> 12 & 0x7f) as f64 / 256.0) * max_extra,
                    speed + ((r >> 19 & 0x7f) as f64 / 256.0) * max_extra,
                ),
                _ => (
                    1.0,
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    -speed - ((r >> 12 & 0x7f) as f64 / 256.0) * max_extra,
                    speed * 0.6 + ((r >> 19 & 0x7f) as f64 / 256.0) * max_extra,
                ),
            };
            self.shooting = Some(ShootingStar {
                x,
                y,
                dx,
                dy,
                age: 0,
                kind,
            });
        }
        if let Some(ref mut s) = self.shooting {
            let speed = if s.kind == 0 { 100.0 } else { 180.0 };
            s.x += s.dx / speed;
            s.y += s.dy / speed;
            s.age += 1;
        }
        let shooting_expired = self.shooting.as_ref().is_some_and(|s| {
            let max_age = if s.kind == 0 {
                SHOOTING_LIFETIME
            } else {
                COMET_LIFETIME
            };
            s.age > max_age || s.x < -0.3 || s.x > 1.3 || s.y > 1.3
        });
        if shooting_expired {
            let kind = self.shooting.as_ref().map(|s| s.kind).unwrap_or(0);
            let cooldown = if kind == 0 {
                SHOOTING_COOLDOWN
            } else {
                COMET_COOLDOWN
            };
            self.shooting = None;
            self.shooting_cooldown = cooldown + (r & 0xff);
        }
    }

    /// Set the config directory and load (or create) `config.toml`.
    /// Call before `run()`.
    pub fn set_config_dir(&mut self, dir: std::path::PathBuf) {
        self.config_manager = ConfigManager::new(dir);
        self.apply_config();
    }

    /// Apply the loaded config (theme, custom colors) to the current app state.
    pub(super) fn apply_config(&mut self) {
        let cfg = self.config_manager.config().clone();

        // Apply default theme if specified.
        if let Some(ref theme_name) = cfg.theme {
            let lower = theme_name.to_lowercase();
            if let Some(idx) = self
                .theme_manager
                .themes
                .iter()
                .position(|(n, _)| n.to_lowercase() == lower)
            {
                self.select_theme(idx);
            }
        }

        // Apply custom color overrides.
        if let Some(ref custom) = cfg.custom_theme {
            let mut t = self.theme.clone();
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
            self.theme = t;
            self.ctx.theme = self.theme.clone();
            self.plugin_manager.on_theme_change_all(&self.theme);
            self.event_bus.emit(crate::event::Event::ThemeChanged);
        }

        self.config_manager.ack();
    }

    /// Get the currently selected theme name.
    pub fn current_theme_name(&self) -> &'static str {
        self.theme_manager.themes[self.theme_manager.current_idx].0
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugin_manager.register(plugin);
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        self.ctx.theme = self.theme_manager.current.clone();
        self.plugin_manager.init_all(&mut self.ctx)?;

        let tick_rate = Duration::from_millis(100);

        while self.running {
            self.plugin_manager.tick_all();

            // Poll for config changes (hot-reload).
            self.config_manager.poll();
            if self.config_manager.dirty {
                self.apply_config();
            }

            // Drain the event bus and forward events to the plugin manager.
            let events = self.event_bus.drain();
            self.plugin_manager.process_events(&events);

            self.tick = self.tick.wrapping_add(1);
            self.update_stars();

            terminal.draw(|f| self.render(f))?;

            if crossterm::event::poll(tick_rate)? {
                if let Event::Key(key) = crossterm::event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    self.handle_key(key);
                }
            }
        }

        disable_raw_mode()?;
        execute!(std::io::stdout(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }

    fn render(&self, f: &mut Frame) {
        let area = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        match self.plugin_manager.active() {
            None => {
                if self.show_about {
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
        status_bar::StatusBar {
            theme: &self.theme,
            palette_open: self.palette.is_some(),
            theme_picker_open: self.theme_manager.picker_open,
            about_open: self.show_about,
            plugin_active: self.plugin_manager.active().is_some(),
            active_plugin_hints: &hints,
        }
        .render(f, chunks[1]);

        if self.palette.is_some() || self.theme_manager.picker_open {
            let dim_bg = self.theme.background_overlay;
            let buf = f.buffer_mut();
            // Scale both foreground AND background brightness by 45%.
            // For cells with an explicit background (e.g. the gold
            // highlight on the selected radio station), dimming both
            // preserves the contrast ratio — the text stays readable.
            // Cells without an explicit bg fall back to dim_bg.
            const DIM: f64 = 0.45;
            for y in area.top()..area.bottom() {
                for x in area.left()..area.right() {
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

        if let Some(ref pal) = self.palette {
            let cmds = self.plugin_manager.commands();
            pal.render(
                f,
                chunks[0],
                &self.theme,
                self.tick,
                &self.dynamic_items,
                cmds,
            );
        }

        if self.theme_manager.picker_open {
            self.theme_manager.render_picker(f, chunks[0], self.tick);
        }

        if self.registry_screen.open {
            self.registry_screen
                .render(f, chunks[0], &self.theme, &self.registry);
        }
    }
}
