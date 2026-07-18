use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Total,
    People,
    Tip,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    total: String,
    people: String,
    tip: String,
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
            total: "100".into(),
            people: "4".into(),
            tip: "15".into(),
            focus: Focus::Total,
            status: "Tab focus \u{b7} type values \u{b7} c copy \u{b7} esc".into(),
        }
    }
}

impl App {
    fn calc(&self) -> Option<Split> {
        let total: f64 = self.total.trim().parse().ok()?;
        let people: usize = self.people.trim().parse().ok()?;
        let tip: f64 = self.tip.trim().parse().ok()?;
        if people == 0 || total < 0.0 || tip < 0.0 {
            return None;
        }
        let tip_amount = total * tip / 100.0;
        let grand = total + tip_amount;
        let per_person = grand / people as f64;
        Some(Split {
            total,
            tip_amount,
            grand,
            per_person,
        })
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::Total => Focus::People,
                    Focus::People => Focus::Tip,
                    Focus::Tip => Focus::Total,
                };
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_result();
                true
            }
            IpcKey::Backspace => {
                match self.focus {
                    Focus::Total => {
                        self.total.pop();
                    }
                    Focus::People => {
                        self.people.pop();
                    }
                    Focus::Tip => {
                        self.tip.pop();
                    }
                }
                true
            }
            IpcKey::Char(c) if is_num(c) => {
                match self.focus {
                    Focus::Total => self.total.push(c),
                    Focus::People => self.people.push(c),
                    Focus::Tip => self.tip.push(c),
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn copy_result(&mut self) {
        let Some(s) = self.calc() else {
            self.status = "Enter valid numbers".into();
            return;
        };
        let text = format!(
            "Per person: ${:.2} (total ${:.2} incl. ${:.2} tip)",
            s.per_person, s.grand, s.tip_amount
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

struct Split {
    total: f64,
    tip_amount: f64,
    grand: f64,
    per_person: f64,
}

fn is_num(c: char) -> bool {
    c.is_ascii_digit() || c == '.'
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
        title: Some(" Expense Splitter ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    let fp = |f: Focus| -> &str {
        if app.focus == f {
            "> "
        } else {
            "  "
        }
    };

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: format!("{}{}Total ($): {}", fp(Focus::Total), "  ", app.total),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Total,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: format!("{}{}People: {}", fp(Focus::People), "  ", app.people),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::People,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 4,
        text: format!("{}{}Tip (%): {}", fp(Focus::Tip), "  ", app.tip),
        fg: Some(t.text),
        bg: None,
        bold: app.focus == Focus::Tip,
        modifiers: 0,
    });

    let box_y = 6;
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

    if let Some(s) = app.calc() {
        let rows = vec![
            format!("Subtotal:      ${:.2}", s.total),
            format!("Tip amount:    ${:.2}", s.tip_amount),
            format!("Grand total:   ${:.2}", s.grand),
            format!("Per person:    ${:.2}", s.per_person),
        ];
        for (i, r) in rows.into_iter().enumerate() {
            cmds.push(RenderCmd::Text {
                x: 4,
                y: box_y + 1 + i as u16,
                text: r,
                fg: Some(if i == 3 { t.accent } else { t.text }),
                bg: None,
                bold: i == 3,
                modifiers: 0,
            });
        }
    } else {
        cmds.push(RenderCmd::Text {
            x: 4,
            y: box_y + 2,
            text: "Enter valid split parameters".into(),
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
    vec![("Utilities".into(), "Open expense splitter".into())]
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
                log::error!("[expense-splitter] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
