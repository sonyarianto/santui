use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DAILY_GOAL: u32 = 8;
const CUP: &str = "\u{1f6b0}";
const EMPTY: &str = "\u{25cb}";

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    count: u32,
    last_reset_day: u64,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            count: 0,
            last_reset_day: day_number(),
            status: "Press + to add water / - to remove / r reset / esc".into(),
        }
    }
}

fn day_number() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
        / 86400
}

impl App {
    fn check_reset(&mut self) {
        let today = day_number();
        if today != self.last_reset_day {
            self.count = 0;
            self.last_reset_day = today;
            self.dirty = true;
        }
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        self.check_reset();
        match key {
            IpcKey::Esc => false,
            IpcKey::Char('+') | IpcKey::Up => {
                if self.count < 99 {
                    self.count += 1;
                }
                true
            }
            IpcKey::Char('-') | IpcKey::Down => {
                self.count = self.count.saturating_sub(1);
                true
            }
            IpcKey::Char('r') => {
                self.count = 0;
                true
            }
            _ => true,
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let t = &self.theme;
        let w = self.area.w.max(40);
        let h = self.area.h.max(12);
        let mut cmds: Vec<Value> = Vec::new();

        cmds.push(json!({"Rect": {"x": 0, "y": 0, "w": w, "h": h, "bg": t.background}}));
        cmds.push(json!({"Border": {
            "x": 0, "y": 0, "w": w, "h": h, "fg": t.border, "borders": BORDER_ALL,
            "bg": t.background_panel, "title": " Water Tracker ",
            "title_fg": t.text, "title_dash_fg": t.border, "border_type": null,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": 1,
            "text": format!("Goal: {DAILY_GOAL} glasses per day"),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        let bar_y = 3;
        let bar_w = w.saturating_sub(6).min(30) as usize;

        let fill = self.count.min(DAILY_GOAL) as usize;
        let bar_fill = fill.min(bar_w);
        let bar_empty = bar_w.saturating_sub(bar_fill);
        let bar = "\u{2588}".repeat(bar_fill) + &"\u{2591}".repeat(bar_empty);
        cmds.push(json!({"Text": {
            "x": 2, "y": bar_y, "text": bar, "fg": t.success, "bg": null,
            "bold": false, "modifiers": 0,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": bar_y + 1,
            "text": format!("{}/{} glasses", self.count, DAILY_GOAL),
            "fg": t.text, "bg": null, "bold": true, "modifiers": 0,
        }}));

        let cups_y = bar_y + 3;
        let cups: String = (0..DAILY_GOAL)
            .map(|i| if i < self.count { CUP } else { EMPTY })
            .collect::<Vec<_>>()
            .join(" ");
        cmds.push(json!({"Text": {
            "x": 2, "y": cups_y, "text": cups, "fg": t.accent, "bg": null,
            "bold": false, "modifiers": 0,
        }}));

        let pct = if self.count >= DAILY_GOAL {
            100
        } else {
            (self.count as f32 / DAILY_GOAL as f32 * 100.0) as u32
        };
        let done = if self.count >= DAILY_GOAL {
            " Goal reached! \u{1f389}"
        } else {
            ""
        };
        cmds.push(json!({"Text": {
            "x": 2, "y": cups_y + 2,
            "text": format!("{pct}% of daily goal{done}"),
            "fg": if self.count >= DAILY_GOAL { t.success } else { t.text_muted },
            "bg": null, "bold": false, "modifiers": 0,
        }}));

        cmds.push(json!({"Text": {
            "x": 2, "y": h.saturating_sub(2),
            "text": self.status.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0,
        }}));

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
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
    vec![("Plugins".into(), "Water Tracker".into())]
}

fn key_hints() -> Vec<(String, String)> {
    vec![
        ("esc".into(), "close".into()),
        ("+".into(), "add glass".into()),
        ("-".into(), "remove glass".into()),
        ("r".into(), "reset".into()),
    ]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app
            .render()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect(),
        hints: key_hints(),
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
    loop {
        line.clear();
        if reader.read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
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
            Ok(
                HostMsg::Tick
                | HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[water-tracker] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
