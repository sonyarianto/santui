use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitSystem {
    Metric,
    Imperial,
}

impl UnitSystem {
    fn label(self) -> &'static str {
        match self {
            Self::Metric => "Metric (kg/cm)",
            Self::Imperial => "Imperial (lb/ft+in)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Weight,
    Height1,
    Height2,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    unit: UnitSystem,
    weight: String,
    height1: String,
    height2: String,
    focus: Focus,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            unit: UnitSystem::Metric,
            weight: "70".into(),
            height1: "175".into(),
            height2: String::new(),
            focus: Focus::Weight,
            status: "Tab focus \u{b7} u unit \u{b7} type values \u{b7} c copy \u{b7} esc".into(),
        }
    }
}

impl App {
    fn calculate_bmi(&self) -> Option<f64> {
        let w: f64 = self.weight.trim().parse().ok()?;
        if w <= 0.0 {
            return None;
        }
        let h_m = match self.unit {
            UnitSystem::Metric => {
                let h_cm: f64 = self.height1.trim().parse().ok()?;
                h_cm / 100.0
            }
            UnitSystem::Imperial => {
                let ft: f64 = self.height1.trim().parse().ok()?;
                let inch: f64 = if self.height2.trim().is_empty() {
                    0.0
                } else {
                    self.height2.trim().parse().ok()?
                };
                (ft * 12.0 + inch) * 0.0254
            }
        };
        if h_m <= 0.0 {
            return None;
        }
        Some(w / (h_m * h_m))
    }

    fn category(bmi: f64) -> (&'static str, [u8; 3]) {
        if bmi < 18.5 {
            ("Underweight", [100, 180, 255])
        } else if bmi < 25.0 {
            ("Normal", [127, 216, 143])
        } else if bmi < 30.0 {
            ("Overweight", [255, 200, 100])
        } else {
            ("Obese", [224, 108, 117])
        }
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::Weight => Focus::Height1,
                    Focus::Height1 if self.unit == UnitSystem::Imperial => Focus::Height2,
                    Focus::Height1 => Focus::Weight,
                    Focus::Height2 => Focus::Weight,
                };
                true
            }
            IpcKey::Char('u') if !modifiers.ctrl => {
                self.unit = match self.unit {
                    UnitSystem::Metric => UnitSystem::Imperial,
                    UnitSystem::Imperial => UnitSystem::Metric,
                };
                self.height2.clear();
                self.focus = Focus::Weight;
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_result();
                true
            }
            IpcKey::Backspace => {
                match self.focus {
                    Focus::Weight => {
                        self.weight.pop();
                    }
                    Focus::Height1 => {
                        self.height1.pop();
                    }
                    Focus::Height2 => {
                        self.height2.pop();
                    }
                }
                true
            }
            IpcKey::Char(c) if is_valid_input(c) => {
                match self.focus {
                    Focus::Weight => self.weight.push(c),
                    Focus::Height1 => self.height1.push(c),
                    Focus::Height2 => self.height2.push(c),
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn copy_result(&mut self) {
        let Some(bmi) = self.calculate_bmi() else {
            self.status = "Enter valid numbers".into();
            return;
        };
        let (cat, _) = Self::category(bmi);
        let text = format!("BMI: {bmi:.1} ({cat})");
        match copy_to_clipboard(&text) {
            Ok(()) => self.status = "Copied to clipboard".into(),
            Err(e) => self.status = format!("Clipboard error: {e}"),
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

fn is_valid_input(c: char) -> bool {
    c.is_ascii_digit() || c == '.'
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(42);
    let h = app.area.h.max(18);

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
        title: Some(" BMI Calculator ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: format!("Unit  {}", app.unit.label()),
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let focus_prefix = |f: Focus, current: Focus| -> &str {
        if f == current {
            "> "
        } else {
            "  "
        }
    };

    let weight_label = match app.unit {
        UnitSystem::Metric => "Weight (kg)",
        UnitSystem::Imperial => "Weight (lb)",
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 4,
        text: format!(
            "{}{}: {}",
            focus_prefix(Focus::Weight, app.focus),
            weight_label,
            app.weight
        ),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Weight,
        modifiers: 0,
    });

    let h1_label = match app.unit {
        UnitSystem::Metric => "Height (cm)",
        UnitSystem::Imperial => "Height (ft)",
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 5,
        text: format!(
            "{}{}: {}",
            focus_prefix(Focus::Height1, app.focus),
            h1_label,
            app.height1
        ),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Height1,
        modifiers: 0,
    });

    if app.unit == UnitSystem::Imperial {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 6,
            text: format!(
                "{}Height (in): {}",
                focus_prefix(Focus::Height2, app.focus),
                if app.height2.is_empty() {
                    "0"
                } else {
                    &app.height2
                }
            ),
            fg: Some(t.text),
            bg: None,
            bold: app.focus == Focus::Height2,
            modifiers: 0,
        });
    }

    let result_y = if app.unit == UnitSystem::Imperial {
        8
    } else {
        7
    };
    let out_box_y = result_y;
    let out_box_w = w.saturating_sub(4);
    let out_box_h = 5;
    cmds.push(RenderCmd::Border {
        x: 2,
        y: out_box_y,
        w: out_box_w,
        h: out_box_h,
        fg: t.accent,
        borders: BORDER_ALL,
        bg: Some(t.background),
        title: None,
        title_fg: None,
        title_dash_fg: None,
        border_type: None,
    });

    if let Some(bmi) = app.calculate_bmi() {
        let (cat, color) = App::category(bmi);
        cmds.push(RenderCmd::Text {
            x: 4,
            y: out_box_y + 1,
            text: format!("BMI: {bmi:.1}"),
            fg: Some(t.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 4,
            y: out_box_y + 2,
            text: format!("Category: {cat}"),
            fg: Some(color),
            bg: None,
            bold: true,
            modifiers: 0,
        });

        let min_normal = 18.5;
        let max_normal = 25.0;
        let h = match app.unit {
            UnitSystem::Metric => {
                let h_cm: f64 = app.height1.trim().parse().unwrap_or(170.0);
                h_cm / 100.0
            }
            UnitSystem::Imperial => {
                let ft: f64 = app.height1.trim().parse().unwrap_or(5.0);
                let inch: f64 = app.height2.trim().parse().unwrap_or(0.0);
                (ft * 12.0 + inch) * 0.0254
            }
        };
        let h2 = h * h;
        let min_w = min_normal * h2;
        let max_w = max_normal * h2;
        let unit_label = match app.unit {
            UnitSystem::Metric => "kg",
            UnitSystem::Imperial => "lb",
        };
        cmds.push(RenderCmd::Text {
            x: 4,
            y: out_box_y + 3,
            text: format!("Normal weight range: {min_w:.1}\u{2013}{max_w:.1} {unit_label}"),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else {
        cmds.push(RenderCmd::Text {
            x: 4,
            y: out_box_y + 2,
            text: "Enter valid weight and height".into(),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    cmds
}

fn hints() -> Vec<(String, String)> {
    vec![
        ("tab".into(), "focus".into()),
        ("u".into(), "unit".into()),
        ("type".into(), "values".into()),
        ("c".into(), "copy".into()),
        ("esc".into(), "back".into()),
    ]
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
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
    vec![("Plugins".into(), "Open BMI calculator".into())]
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
                log::error!("[bmi-calculator] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
