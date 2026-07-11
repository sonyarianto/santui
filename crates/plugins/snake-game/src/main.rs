use std::io::{BufRead, BufReader, Write};

use rand::{Rng, RngExt};
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

const GRID_W: usize = 30;
const GRID_H: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    snake: Vec<(usize, usize)>,
    food: (usize, usize),
    dir: Dir,
    next_dir: Dir,
    score: u32,
    high_score: u32,
    game_over: bool,
    paused: bool,
    speed: u64,
    tick_count: u64,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let mut rng = rand::rng();
        let snake = vec![(GRID_W / 2, GRID_H / 2)];
        let food = place_food(&snake, &mut rng);
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            snake,
            food,
            dir: Dir::Right,
            next_dir: Dir::Right,
            score: 0,
            high_score: 0,
            game_over: false,
            paused: false,
            speed: 5,
            tick_count: 0,
            status: "Arrow keys \u{b7} space pause \u{b7} r reset \u{b7} esc quit".into(),
        }
    }
}

fn place_food(snake: &[(usize, usize)], rng: &mut impl Rng) -> (usize, usize) {
    loop {
        let x = rng.random_range(0..GRID_W);
        let y = rng.random_range(0..GRID_H);
        if !snake.contains(&(x, y)) {
            return (x, y);
        }
    }
}

impl App {
    fn reset(&mut self) {
        let mut rng = rand::rng();
        self.snake = vec![(GRID_W / 2, GRID_H / 2)];
        self.food = place_food(&self.snake, &mut rng);
        self.dir = Dir::Right;
        self.next_dir = Dir::Right;
        self.score = 0;
        self.game_over = false;
        self.paused = false;
        self.tick_count = 0;
    }

    fn tick(&mut self) {
        if self.game_over || self.paused {
            return;
        }
        self.tick_count += 1;
        if !self.tick_count.is_multiple_of(self.speed) {
            return;
        }

        self.dir = self.next_dir;
        let (hx, hy) = self.snake[0];
        let (nx, ny) = match self.dir {
            Dir::Up => (hx, hy.wrapping_sub(1)),
            Dir::Down => (hx, hy.wrapping_add(1)),
            Dir::Left => (hx.wrapping_sub(1), hy),
            Dir::Right => (hx.wrapping_add(1), hy),
        };

        if nx >= GRID_W || ny >= GRID_H || self.snake.contains(&(nx, ny)) {
            self.game_over = true;
            if self.score > self.high_score {
                self.high_score = self.score;
            }
            self.status = format!("Game Over! Score: {}", self.score);
            return;
        }

        self.snake.insert(0, (nx, ny));
        if (nx, ny) == self.food {
            self.score += 1;
            let mut rng = rand::rng();
            self.food = place_food(&self.snake, &mut rng);
            self.speed = 5.max(20 - (self.score / 3) as u64);
        } else {
            self.snake.pop();
        }
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Up | IpcKey::Char('w') if !modifiers.ctrl => {
                if self.dir != Dir::Down {
                    self.next_dir = Dir::Up;
                }
                true
            }
            IpcKey::Down | IpcKey::Char('s') if !modifiers.ctrl => {
                if self.dir != Dir::Up {
                    self.next_dir = Dir::Down;
                }
                true
            }
            IpcKey::Left | IpcKey::Char('a') if !modifiers.ctrl => {
                if self.dir != Dir::Right {
                    self.next_dir = Dir::Left;
                }
                true
            }
            IpcKey::Right | IpcKey::Char('d') if !modifiers.ctrl => {
                if self.dir != Dir::Left {
                    self.next_dir = Dir::Right;
                }
                true
            }
            IpcKey::Char(' ') if !modifiers.ctrl => {
                self.paused = !self.paused;
                self.status = if self.paused {
                    "Paused".into()
                } else {
                    "Playing".into()
                };
                true
            }
            IpcKey::Char('r') if !modifiers.ctrl => {
                self.reset();
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(44);
    let h = app.area.h.max(14);

    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: t.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: t.border,
        borders: BORDER_ALL,
        bg: Some(t.background_panel),
        title: Some(" Snake ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!("Score: {}  High Score: {}", app.score, app.high_score),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let grid_x = 2u16;
    let grid_y = 3u16;
    let grid_w = GRID_W.min((w - 6) as usize);
    let grid_h = GRID_H.min((h - 6) as usize);

    cmds.push(RenderCmd::Border {
        x: grid_x,
        y: grid_y,
        w: grid_w as u16 + 2,
        h: grid_h as u16 + 2,
        fg: t.border,
        borders: BORDER_ALL,
        bg: Some(t.background),
        title: None,
        title_fg: None,
        title_dash_fg: None,
        border_type: None,
    });

    for y in 0..grid_h {
        let mut row = String::with_capacity(grid_w);
        for x in 0..grid_w {
            let pos = (x, y);
            if pos == app.food {
                row.push('*');
            } else if let Some(i) = app.snake.iter().position(|&p| p == pos) {
                row.push(if i == 0 { '@' } else { '#' });
            } else {
                row.push(' ');
            }
        }
        cmds.push(RenderCmd::Text {
            x: grid_x + 1,
            y: grid_y + 1 + y as u16,
            text: row,
            fg: Some(t.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    if app.game_over {
        cmds.push(RenderCmd::Text {
            x: grid_x + 2,
            y: grid_y + grid_h as u16 / 2,
            text: "GAME OVER - press R to restart".into(),
            fg: Some(t.error),
            bg: None,
            bold: true,
            modifiers: 0,
        });
    }

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: app.status.clone(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text: "arrows/wasd move \u{b7} space pause \u{b7} r reset \u{b7} esc quit".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

fn default_theme() -> ThemeData {
    ThemeData {
        text: [220; 3],
        text_muted: [140; 3],
        accent: [180; 3],
        highlight: [220; 3],
        logo: [255; 3],
        background: [0; 3],
        background_panel: [20; 3],
        background_overlay: [10; 3],
        border: [150; 3],
        success: [127, 216, 143],
        error: [224, 108, 117],
        inverted_text: [20; 3],
    }
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("Media & Fun".into(), "Open Snake game".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val, "hints": [], "palette_commands": palette_commands(),
        "request": null, "plugin_message": null, "consumed": consumed,
    });
    if let Ok(json_str) = serde_json::to_string(&json) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{json_str}");
        let _ = out.flush();
    }
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = App::default();
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        let trimmed = line.trim_end();
        let msg = serde_json::from_str::<HostMsg>(trimmed);
        let consumed = match msg {
            Ok(HostMsg::Init { theme, area, .. }) => {
                app.theme = theme;
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Resize { area }) => {
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::ThemeChange { theme }) => {
                app.theme = theme;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Key { key, modifiers }) => app.handle_key(key, modifiers),
            Ok(HostMsg::PaletteCommand { .. }) => {
                app.dirty = true;
                true
            }
            Ok(HostMsg::Shutdown) => break,
            Ok(HostMsg::Tick) => {
                app.tick();
                app.dirty = true;
                false
            }
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[snake-game] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
