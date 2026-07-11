use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Inhale,
    Hold,
    Exhale,
    Rest,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    phase: Phase,
    countdown: u32,
    total_cycles: u32,
    cycle: u32,
    running: bool,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            phase: Phase::Inhale,
            countdown: 4,
            total_cycles: 0,
            cycle: 1,
            running: false,
            status: "space start \u{b7} r reset \u{b7} esc quit".into(),
        }
    }
}

impl App {
    fn phase_duration(&self, phase: Phase) -> u32 {
        match phase {
            Phase::Inhale => 4,
            Phase::Hold => 4,
            Phase::Exhale => 4,
            Phase::Rest => 2,
        }
    }

    fn phase_label(&self, phase: Phase) -> &'static str {
        match phase {
            Phase::Inhale => "Breathe In",
            Phase::Hold => "Hold",
            Phase::Exhale => "Breathe Out",
            Phase::Rest => "Rest",
        }
    }

    fn tick(&mut self) {
        if !self.running {
            return;
        }
        self.dirty = true;
        self.countdown = self.countdown.saturating_sub(1);
        if self.countdown == 0 {
            self.phase = match self.phase {
                Phase::Inhale => Phase::Hold,
                Phase::Hold => Phase::Exhale,
                Phase::Exhale => Phase::Rest,
                Phase::Rest => {
                    self.cycle += 1;
                    self.total_cycles += 1;
                    Phase::Inhale
                }
            };
            self.countdown = self.phase_duration(self.phase);
        }
    }

    fn reset(&mut self) {
        self.phase = Phase::Inhale;
        self.countdown = 4;
        self.cycle = 1;
        self.running = false;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char(' ') if !modifiers.ctrl => {
                if !self.running {
                    self.running = true;
                    self.status = "Breathing...".into();
                } else {
                    self.running = false;
                    self.status = "Paused".into();
                }
                true
            }
            IpcKey::Char('r') if !modifiers.ctrl => {
                self.reset();
                self.status = "Reset".into();
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
    let w = app.area.w.max(42);
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
        title: Some(" Breathing Exercise ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let label = app.phase_label(app.phase);
    let (fg, symbol) = match app.phase {
        Phase::Inhale => (t.success, "\u{25B2}"),
        Phase::Hold => (t.text, "\u{25A0}"),
        Phase::Exhale => (t.accent, "\u{25BC}"),
        Phase::Rest => (t.text_muted, "\u{25CF}"),
    };

    let progress = app.countdown as f64 / app.phase_duration(app.phase) as f64;
    let bar_w = w.saturating_sub(12) as usize;
    let fill = (progress * bar_w as f64) as usize;
    let bar: String = "█".repeat(fill) + &"░".repeat(bar_w.saturating_sub(fill));

    let center_x = w / 2;
    let label_len = label.len() as u16 + 4;

    cmds.push(RenderCmd::Text {
        x: center_x.saturating_sub(label_len / 2),
        y: 2,
        text: format!("{symbol}  {label}"),
        fg: Some(fg),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 4,
        text: format!("Cycle: {}  Total: {}", app.cycle, app.total_cycles),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 6,
        text: format!("Countdown: {}s", app.countdown),
        fg: Some(t.text),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 8,
        text: bar,
        fg: Some(fg),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    if !app.running {
        cmds.push(RenderCmd::Text {
            x: center_x.saturating_sub(10),
            y: h.saturating_sub(5),
            text: if app.countdown == 4 && app.cycle == 1 && app.total_cycles == 0 {
                "Press Space to begin".into()
            } else {
                "Paused".into()
            },
            fg: Some(t.text_muted),
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
    cmds
}

fn hints() -> Vec<(String, String)> {
    vec![
        ("space".into(), "start/pause".into()),
        ("r".into(), "reset".into()),
        ("esc".into(), "back".into()),
    ]
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
    vec![("Health & Wellness".into(), "Open breathing exercise".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val, "hints": hints(), "palette_commands": palette_commands(),
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
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[breathing-exercise] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
