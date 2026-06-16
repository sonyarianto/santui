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
const STAR_COUNT: usize = 80;
const SHOOTING_LIFETIME: u64 = 80;
const SHOOTING_COOLDOWN: u64 = 300;

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
}

struct ShootingStar {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    age: u64,
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
            stars: (0..STAR_COUNT)
                .map(|i| Star {
                    x: ((i * 157 + 11) * 131 % 997) as u16,
                    y: ((i * 311 + 7) * 173 % 997) as u16,
                    phase: ((i * 53 + 13) * 71 % 628) as u16,
                })
                .collect(),
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
        if self.shooting.is_none() && self.shooting_cooldown == 0 && (r & 0x3f) < 8 {
            let side = r & 3;
            let (x, y, dx, dy) = match side {
                0 => (
                    0.0,
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    0.6 + (r >> 12 & 0x7f) as f64 / 256.0,
                    0.4 + (r >> 19 & 0x7f) as f64 / 256.0,
                ),
                1 => (
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    0.0,
                    0.3 + (r >> 12 & 0x7f) as f64 / 256.0,
                    0.6 + (r >> 19 & 0x7f) as f64 / 256.0,
                ),
                2 => (
                    1.0,
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    -0.6 - (r >> 12 & 0x7f) as f64 / 256.0,
                    0.3 + (r >> 19 & 0x7f) as f64 / 256.0,
                ),
                _ => (
                    (r >> 2 & 0x3ff) as f64 / 1024.0,
                    0.0,
                    0.3 + (r >> 12 & 0x7f) as f64 / 256.0,
                    0.6 + (r >> 19 & 0x7f) as f64 / 256.0,
                ),
            };
            self.shooting = Some(ShootingStar {
                x,
                y,
                dx,
                dy,
                age: 0,
            });
        }
        if let Some(ref mut s) = self.shooting {
            s.x += s.dx / 100.0;
            s.y += s.dy / 100.0;
            s.age += 1;
            if s.age > SHOOTING_LIFETIME || s.x < -0.2 || s.x > 1.2 || s.y > 1.2 {
                self.shooting = None;
                self.shooting_cooldown = SHOOTING_COOLDOWN + (r & 0x1ff);
            }
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

        if self.palette.is_some() {
            self.render_palette(f, chunks[0]);
        }

        if self.show_theme_picker {
            self.render_theme_picker(f, chunks[0]);
        }

        self.render_status_bar(f, chunks[1]);
    }
}
