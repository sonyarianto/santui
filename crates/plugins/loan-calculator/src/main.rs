use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Principal,
    Rate,
    Years,
    Down,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    principal: String,
    rate: String,
    years: String,
    down: String,
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
            principal: "200000".into(),
            rate: "5.5".into(),
            years: "30".into(),
            down: String::new(),
            focus: Focus::Principal,
            status: "Tab focus \u{b7} type values \u{b7} c copy \u{b7} esc".into(),
        }
    }
}

impl App {
    fn calc(&self) -> Option<Amortization> {
        let p: f64 = self.principal.trim().parse().ok()?;
        let r: f64 = self.rate.trim().parse().ok()?;
        let y: f64 = self.years.trim().parse().ok()?;
        if p <= 0.0 || r < 0.0 || y <= 0.0 {
            return None;
        }
        let down: f64 = if self.down.trim().is_empty() {
            0.0
        } else {
            self.down.trim().parse().ok()?
        };
        let loan = (p - down).max(0.0);
        if loan == 0.0 {
            return Some(Amortization {
                monthly: 0.0,
                total_paid: 0.0,
                total_interest: 0.0,
                loan,
            });
        }
        let n = (y * 12.0) as u32;
        let monthly_rate = r / 100.0 / 12.0;
        let monthly = if monthly_rate == 0.0 {
            loan / n as f64
        } else {
            loan * monthly_rate / (1.0 - (1.0 + monthly_rate).powf(-(n as f64)))
        };
        let total_paid = monthly * n as f64;
        let total_interest = total_paid - loan;
        Some(Amortization {
            monthly,
            total_paid,
            total_interest,
            loan,
        })
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::Principal => Focus::Rate,
                    Focus::Rate => Focus::Years,
                    Focus::Years => Focus::Down,
                    Focus::Down => Focus::Principal,
                };
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_result();
                true
            }
            IpcKey::Backspace => {
                match self.focus {
                    Focus::Principal => {
                        self.principal.pop();
                    }
                    Focus::Rate => {
                        self.rate.pop();
                    }
                    Focus::Years => {
                        self.years.pop();
                    }
                    Focus::Down => {
                        self.down.pop();
                    }
                }
                true
            }
            IpcKey::Char(c) if is_numeric(c) => {
                match self.focus {
                    Focus::Principal => self.principal.push(c),
                    Focus::Rate => self.rate.push(c),
                    Focus::Years => self.years.push(c),
                    Focus::Down => self.down.push(c),
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn copy_result(&mut self) {
        let Some(a) = self.calc() else {
            self.status = "Enter valid numbers".into();
            return;
        };
        let text = format!(
            "Monthly: {:.2}, Total: {:.2}, Interest: {:.2}",
            a.monthly, a.total_paid, a.total_interest
        );
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

struct Amortization {
    monthly: f64,
    total_paid: f64,
    total_interest: f64,
    loan: f64,
}

fn is_numeric(c: char) -> bool {
    c.is_ascii_digit() || c == '.' || c == '-'
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(46);
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
        title: Some(" Loan Calculator ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let focus_prefix = |f: Focus| -> &str {
        if app.focus == f {
            "> "
        } else {
            "  "
        }
    };

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: format!(
            "{}{}Principal ($): {}",
            focus_prefix(Focus::Principal),
            "  ",
            app.principal
        ),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Principal,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: format!(
            "{}{}Annual rate (%): {}",
            focus_prefix(Focus::Rate),
            "  ",
            app.rate
        ),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Rate,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 4,
        text: format!(
            "{}{}Term (years): {}",
            focus_prefix(Focus::Years),
            "  ",
            app.years
        ),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Years,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 5,
        text: format!(
            "{}{}Down payment ($): {}",
            focus_prefix(Focus::Down),
            "  ",
            if app.down.is_empty() { "0" } else { &app.down }
        ),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Down,
        modifiers: 0,
    });

    let box_y = 7;
    let box_w = w.saturating_sub(4);
    let box_h = 7;
    cmds.push(RenderCmd::Border {
        x: 2,
        y: box_y,
        w: box_w,
        h: box_h,
        fg: t.accent,
        borders: BORDER_ALL,
        bg: Some(t.background),
        title: None,
        title_fg: None,
        title_dash_fg: None,
        border_type: None,
    });

    if let Some(a) = app.calc() {
        let rows = vec![
            format!("Loan amount:    ${:.2}", a.loan),
            format!("Monthly payment: ${:.2}", a.monthly),
            format!("Total payments:  ${:.2}", a.total_paid),
            format!("Total interest:  ${:.2}", a.total_interest),
        ];
        for (i, r) in rows.into_iter().enumerate() {
            cmds.push(RenderCmd::Text {
                x: 4,
                y: box_y + 1 + i as u16,
                text: r,
                fg: Some(if i == 1 { t.accent } else { t.text }),
                bg: None,
                bold: i == 1,
                modifiers: 0,
            });
        }
    } else {
        cmds.push(RenderCmd::Text {
            x: 4,
            y: box_y + 2,
            text: "Enter valid loan parameters".into(),
            fg: Some(t.text_muted),
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
        ("tab".into(), "focus".into()),
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
    vec![("Utilities".into(), "Open loan calculator".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": hints(),
        "palette_commands": palette_commands(),
        "request": null,
        "plugin_message": null,
        "consumed": consumed,
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
                log::error!("[loan-calculator] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
