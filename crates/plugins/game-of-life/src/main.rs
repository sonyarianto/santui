use std::io::{BufRead, BufReader, Write};

use rand::RngExt;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

const GRID_W: usize = 40;
const GRID_H: usize = 20;

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    grid: [[bool; GRID_W]; GRID_H],
    running: bool,
    generation: u64,
    alive: usize,
}

impl Default for App {
    fn default() -> Self {
        let mut grid = [[false; GRID_W]; GRID_H];
        let mut rng = rand::rng();
        for row in grid.iter_mut() {
            for cell in row.iter_mut() {
                *cell = rng.random_bool(0.3);
            }
        }
        let alive = grid.iter().flatten().filter(|&&c| c).count();
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            grid,
            running: false,
            generation: 0,
            alive,
        }
    }
}

impl App {
    fn count_neighbors(&self, x: usize, y: usize) -> u8 {
        let mut n = 0u8;
        for dy in [-1i32, 0, 1] {
            for dx in [-1i32, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = (x as i32 + dx).rem_euclid(GRID_W as i32) as usize;
                let ny = (y as i32 + dy).rem_euclid(GRID_H as i32) as usize;
                if self.grid[ny][nx] {
                    n += 1;
                }
            }
        }
        n
    }

    fn step(&mut self) {
        let mut next = [[false; GRID_W]; GRID_H];
        let mut alive = 0;
        for (y, row) in next.iter_mut().enumerate() {
            for (x, cell) in row.iter_mut().enumerate() {
                let n = self.count_neighbors(x, y);
                let cur = self.grid[y][x];
                let new = matches!((cur, n), (true, 2) | (true, 3) | (false, 3));
                *cell = new;
                if new {
                    alive += 1;
                }
            }
        }
        self.grid = next;
        self.generation += 1;
        self.alive = alive;
    }

    fn toggle_cell(&mut self, x: usize, y: usize) {
        if x < GRID_W && y < GRID_H {
            self.grid[y][x] = !self.grid[y][x];
            self.alive = self.grid.iter().flatten().filter(|&&c| c).count();
        }
    }

    fn clear(&mut self) {
        self.grid = [[false; GRID_W]; GRID_H];
        self.generation = 0;
        self.alive = 0;
    }

    fn random_fill(&mut self) {
        let mut rng = rand::rng();
        for row in self.grid.iter_mut() {
            for cell in row.iter_mut() {
                *cell = rng.random_bool(0.3);
            }
        }
        self.alive = self.grid.iter().flatten().filter(|&&c| c).count();
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char(' ') => {
                self.running = !self.running;
                true
            }
            IpcKey::Char('s') if !modifiers.ctrl => {
                self.step();
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.clear();
                self.running = false;
                true
            }
            IpcKey::Char('r') if !modifiers.ctrl => {
                self.random_fill();
                self.generation = 0;
                true
            }
            IpcKey::Char('+') | IpcKey::Right => {
                if !self.running {
                    self.step();
                }
                true
            }
            IpcKey::Up | IpcKey::Char('w') => {
                if !self.running {
                    self.toggle_cell(GRID_W / 2, 1);
                }
                true
            }
            IpcKey::Down | IpcKey::Char('s') => {
                if !self.running {
                    self.toggle_cell(GRID_W / 2, GRID_H - 2);
                }
                true
            }
            IpcKey::Left | IpcKey::Char('a') => {
                if !self.running {
                    self.toggle_cell(1, GRID_H / 2);
                }
                true
            }
            IpcKey::Char('d') => {
                if !self.running {
                    self.toggle_cell(GRID_W - 2, GRID_H / 2);
                }
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
    let w = app.area.w.max(50);
    let h = app.area.h.max(16);

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
        title: Some(" Game of Life ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let status = if app.running { "Running" } else { "Paused" };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!("Gen: {}  Alive: {}  [{status}]", app.generation, app.alive),
        fg: if app.running {
            Some(t.success)
        } else {
            Some(t.text_muted)
        },
        bg: None,
        bold: app.running,
        modifiers: 0,
    });

    let grid_y = 3;
    for y in 0..GRID_H.min((h - 5) as usize) {
        let mut row = String::with_capacity(GRID_W + 2);
        row.push('│');
        for x in 0..GRID_W.min((w - 6) as usize) {
            row.push(if app.grid[y][x] { '█' } else { '·' });
        }
        row.push('│');
        cmds.push(RenderCmd::Text {
            x: 2,
            y: grid_y + y as u16,
            text: row,
            fg: Some(t.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: app.status(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(1),
        text:
            "space play/pause \u{b7} s step \u{b7} c clear \u{b7} r random \u{b7} + step \u{b7} esc"
                .into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
}

impl App {
    fn status(&self) -> String {
        if self.running {
            format!(
                "Gen {} · {} alive — press space to pause",
                self.generation, self.alive
            )
        } else {
            "Paused · space play · s step · c clear · r random · esc quit".into()
        }
    }
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
    vec![("Media & Fun".into(), "Open Game of Life".into())]
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
                if app.running {
                    app.step();
                    app.dirty = true;
                }
                false
            }
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[game-of-life] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
