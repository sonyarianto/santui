mod handle_key;
mod palette;
mod screens;

use crate::plugin::{Plugin, PluginContext};
use crate::theme::Theme;
use crossterm::event::{Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;
use ratatui::Terminal;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const STAR_COUNT: usize = 88;
const SHOOTING_LIFETIME: u64 = 50;
const SHOOTING_COOLDOWN: u64 = 180;
const COMET_LIFETIME: u64 = 100;
const COMET_COOLDOWN: u64 = 500;

struct CmdItem {
    category: &'static str,
    label: &'static str,
}

const CMD_ITEMS: &[CmdItem] = &[
    CmdItem {
        category: "Modules",
        label: "Radio Streaming Player",
    },
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

fn pal_w(content_w: u16) -> u16 {
    let max = content_w.saturating_sub(2);
    if max < PAL_MIN_W {
        return max;
    }
    max.clamp(PAL_MIN_W, PAL_IDEAL_W)
}

fn max_list_h(content_h: u16) -> u16 {
    (content_h / 2).saturating_sub(6).max(3)
}

struct PaletteState {
    query: String,
    cursor: usize,
    scroll: u16,
}

pub struct Santui {
    plugins: Vec<Box<dyn Plugin>>,
    ctx: PluginContext,
    theme: Theme,
    themes: Vec<(&'static str, Theme)>,
    theme_idx: usize,
    active_plugin: Option<usize>,
    palette: Option<PaletteState>,
    show_about: bool,
    show_theme_picker: bool,
    theme_picker_query: String,
    theme_picker_cursor: usize,
    theme_picker_scroll: u16,
    theme_picker_orig_idx: usize,
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
        let themes = Theme::all();
        let theme = themes[1].1.clone();
        Santui {
            plugins: Vec::new(),
            ctx: PluginContext::new(),
            theme,
            themes,
            theme_idx: 1,
            active_plugin: None,
            palette: None,
            show_about: false,
            show_theme_picker: false,
            theme_picker_query: String::new(),
            theme_picker_cursor: 0,
            theme_picker_scroll: 0,
            theme_picker_orig_idx: 0,
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

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        self.ctx.theme = self.theme.clone();
        for p in &mut self.plugins {
            p.init(&mut self.ctx)?;
        }

        let tick_rate = Duration::from_millis(100);

        while self.running {
            for p in &mut self.plugins {
                p.tick();
            }

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

        match self.active_plugin {
            None => {
                if self.show_about {
                    self.render_about(f, chunks[0]);
                } else {
                    self.render_splash(f, chunks[0]);
                }
            }
            Some(idx) => {
                self.plugins[idx].render(f, chunks[0]);
            }
        }

        self.render_status_bar(f, chunks[1]);

        if self.palette.is_some() || self.show_theme_picker {
            let dim_bg = self.theme.background_overlay;
            let buf = f.buffer_mut();
            for y in area.top()..area.bottom() {
                for x in area.left()..area.right() {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        let mut style = cell.style();
                        style.bg = Some(dim_bg);
                        cell.set_style(style);
                    }
                }
            }
        }

        if self.palette.is_some() {
            self.render_palette(f, chunks[0]);
        }

        if self.show_theme_picker {
            self.render_theme_picker(f, chunks[0]);
        }
    }
}
