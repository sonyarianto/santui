use std::io::{BufRead, BufReader};

use rand::RngExt;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct ResultData {
    ping: f64,
    download: f64,
    upload: f64,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    progress: f64,
    testing: bool,
    result: Option<ResultData>,
    status: String,
    tick_counter: u32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            progress: 0.0,
            testing: false,
            result: None,
            status: String::from("Press Enter to start speed test"),
            tick_counter: 0,
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Enter => {
                if !self.testing {
                    self.start_test();
                }
                true
            }
            IpcKey::Esc => false,
            _ => true,
        }
    }

    fn start_test(&mut self) {
        self.testing = true;
        self.progress = 0.0;
        self.result = None;
        self.tick_counter = 0;
        self.status = String::from("Testing...");
    }

    fn handle_tick(&mut self) {
        if self.testing {
            self.tick_counter += 1;
            self.progress = (self.tick_counter as f64) / 50.0;
            if self.progress >= 1.0 {
                self.progress = 1.0;
                self.testing = false;
                let mut rng = rand::rng();
                self.result = Some(ResultData {
                    ping: rng.random_range(5.0..80.0),
                    download: rng.random_range(20.0..500.0),
                    upload: rng.random_range(10.0..200.0),
                });
                self.status = String::from("Test complete!");
            }
            self.dirty = true;
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
    let h = app.area.h.max(12);

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
        title: Some(String::from(" Speed Test ")),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: String::from("Network Speed Test"),
        fg: Some(t.text),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    cmds.push(RenderCmd::Gauge {
        x: 2,
        y: 3,
        w: w.saturating_sub(4),
        h: 1,
        ratio: app.progress,
        label: Some(format!("{:.0}%", app.progress * 100.0)),
        style: TextStyle {
            fg: Some(t.text),
            bg: Some(t.background_panel),
            bold: false,
            modifiers: 0,
        },
        gauge_style: TextStyle {
            fg: Some(t.highlight),
            bg: Some(t.background_panel),
            bold: false,
            modifiers: 0,
        },
    });

    if let Some(ref res) = app.result {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 5,
            text: format!("Ping:     {:.1} ms", res.ping),
            fg: Some(if res.ping < 30.0 {
                t.success
            } else if res.ping < 60.0 {
                t.text
            } else {
                t.error
            }),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 6,
            text: format!("Download: {:.1} Mbps", res.download),
            fg: Some(t.success),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 7,
            text: format!("Upload:   {:.1} Mbps", res.upload),
            fg: Some(t.success),
            bg: None,
            bold: false,
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
        ("enter".into(), "start test".into()),
        ("esc".into(), "back".into()),
    ]
}

use santui_ipc::protocol::TextStyle;

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
    vec![("Plugins".into(), "Open Speed Test".into())]
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
            Ok(HostMsg::PaletteCommand { .. }) => {
                app.dirty = true;
                true
            }
            Ok(HostMsg::Tick) => {
                app.handle_tick();
                false
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
                log::error!("[speed-test] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
