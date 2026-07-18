use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    tick: u64,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            tick: 0,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_tick(&mut self) {
        self.tick += 1;
        self.dirty = true;
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn mock_cpu(tick: u64) -> f64 {
    let base = 45.0 + ((tick as f64) * 0.7).sin() * 25.0;
    (base + ((tick as f64) * 1.3).cos() * 15.0).clamp(5.0, 95.0) / 100.0
}

fn mock_mem_used(tick: u64) -> u64 {
    8192 + ((tick as f64) * 0.3).sin() as u64 * 2048
}

fn mock_mem_total() -> u64 {
    16384
}

fn mock_disk_used(tick: u64) -> u64 {
    120 + ((tick as f64) * 0.1).sin() as u64 * 20
}

fn mock_disk_total() -> u64 {
    256
}

fn mock_uptime(tick: u64) -> String {
    let secs = tick * 2;
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    format!("{}d {}h {}m", d, h, m)
}

fn mock_processes(tick: u64) -> u64 {
    180 + ((tick as f64) * 0.5).sin() as u64 * 20
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(56);
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
        title: Some(" Dashboard ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let cpu = mock_cpu(app.tick);
    let mem_used = mock_mem_used(app.tick);
    let mem_total = mock_mem_total();
    let disk_used = mock_disk_used(app.tick);
    let disk_total = mock_disk_total();
    let uptime = mock_uptime(app.tick);
    let procs = mock_processes(app.tick);

    // CPU
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!("CPU:  {:.1}%", cpu * 100.0),
        fg: Some(t.text),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Gauge {
        x: 2,
        y: 2,
        w: w.saturating_sub(4),
        h: 1,
        ratio: cpu,
        label: Some(format!("{:.1}%", cpu * 100.0)),
        style: TextStyle {
            fg: Some(t.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        gauge_style: TextStyle {
            fg: None,
            bg: Some(t.accent),
            bold: false,
            modifiers: 0,
        },
    });

    // Memory
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 4,
        text: format!("Memory: {} MB / {} MB", mem_used, mem_total),
        fg: Some(t.text),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Gauge {
        x: 2,
        y: 5,
        w: w.saturating_sub(4),
        h: 1,
        ratio: mem_used as f64 / mem_total as f64,
        label: Some(format!("{}%", mem_used * 100 / mem_total)),
        style: TextStyle {
            fg: Some(t.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        gauge_style: TextStyle {
            fg: None,
            bg: Some(t.highlight),
            bold: false,
            modifiers: 0,
        },
    });

    // Disk
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 7,
        text: format!("Disk: {} GB / {} GB", disk_used, disk_total),
        fg: Some(t.text),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Gauge {
        x: 2,
        y: 8,
        w: w.saturating_sub(4),
        h: 1,
        ratio: disk_used as f64 / disk_total as f64,
        label: Some(format!("{}%", disk_used * 100 / disk_total)),
        style: TextStyle {
            fg: Some(t.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        gauge_style: TextStyle {
            fg: None,
            bg: Some(t.success),
            bold: false,
            modifiers: 0,
        },
    });

    // Uptime
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 10,
        text: format!("Uptime: {}", uptime),
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    // Processes
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 11,
        text: format!("Processes: {}", procs),
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    // Status bar
    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: "Tick counts update mock values".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds
}

fn hints() -> Vec<(String, String)> {
    vec![("esc".into(), "back".into())]
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
    vec![("System Monitoring".into(), "Open system dashboard".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: hints(),
        palette_commands: palette_commands(),
        request: None,
        plugin_message: None,
        consumed,
    };
    let mut out = std::io::stdout().lock();
    let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &msg);
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
            Ok(HostMsg::Tick) => {
                app.handle_tick();
                false
            }
            Ok(HostMsg::PaletteCommand { .. }) => {
                app.dirty = true;
                true
            }
            Ok(HostMsg::Shutdown) => break,
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
                log::error!("[dashboard] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
