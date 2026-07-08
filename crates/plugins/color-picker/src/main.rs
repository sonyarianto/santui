use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    red: u8,
    green: u8,
    blue: u8,
    hex: String,
    focus: Focus,
    status: String,
    history: Vec<String>,
    cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Red,
    Green,
    Blue,
    Hex,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            red: 100,
            green: 150,
            blue: 200,
            hex: String::new(),
            focus: Focus::Hex,
            status: "Type hex #RRGGBB \u{b7} tab focus \u{b7} c copy".into(),
            history: Vec::new(),
            cursor: 0,
        };
        app.sync_hex();
        app
    }
}

impl App {
    fn sync_hex(&mut self) {
        self.hex = format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue);
    }

    fn parse_hex(&mut self, hex: &str) -> bool {
        let h = hex.trim_start_matches('#');
        if h.len() == 6 {
            let Ok(r) = u8::from_str_radix(&h[0..2], 16) else {
                return false;
            };
            let Ok(g) = u8::from_str_radix(&h[2..4], 16) else {
                return false;
            };
            let Ok(b) = u8::from_str_radix(&h[4..6], 16) else {
                return false;
            };
            self.red = r;
            self.green = g;
            self.blue = b;
            self.sync_hex();
            true
        } else {
            false
        }
    }

    fn hsl(&self) -> (f64, f64, f64) {
        let r = self.red as f64 / 255.0;
        let g = self.green as f64 / 255.0;
        let b = self.blue as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < 0.001 {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if (max - r).abs() < 0.001 {
            ((g - b) / d + if g < b { 6.0 } else { 0.0 }) * 60.0
        } else if (max - g).abs() < 0.001 {
            ((b - r) / d + 2.0) * 60.0
        } else {
            ((r - g) / d + 4.0) * 60.0
        };
        (h, s * 100.0, l * 100.0)
    }

    fn add_history(&mut self) {
        self.history.insert(0, self.hex.clone());
        self.history.truncate(100);
        self.cursor = 0;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::Red => Focus::Green,
                    Focus::Green => Focus::Blue,
                    Focus::Blue => Focus::Hex,
                    Focus::Hex => Focus::Red,
                };
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_current();
                true
            }
            IpcKey::Enter => {
                if !self.hex.is_empty() {
                    self.add_history();
                    self.copy_current();
                }
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                match self.focus {
                    Focus::Red => self.red = self.red.saturating_add(5),
                    Focus::Green => self.green = self.green.saturating_add(5),
                    Focus::Blue => self.blue = self.blue.saturating_add(5),
                    Focus::Hex => {
                        if self.cursor < self.history.len().saturating_sub(1) {
                            self.cursor += 1;
                        }
                    }
                }
                self.sync_hex();
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                match self.focus {
                    Focus::Red => self.red = self.red.saturating_sub(5),
                    Focus::Green => self.green = self.green.saturating_sub(5),
                    Focus::Blue => self.blue = self.blue.saturating_sub(5),
                    Focus::Hex => {
                        self.cursor = self.cursor.saturating_sub(1);
                    }
                }
                self.sync_hex();
                true
            }
            IpcKey::PageUp => {
                match self.focus {
                    Focus::Red => self.red = self.red.saturating_add(20),
                    Focus::Green => self.green = self.green.saturating_add(20),
                    Focus::Blue => self.blue = self.blue.saturating_add(20),
                    _ => {}
                }
                self.sync_hex();
                true
            }
            IpcKey::PageDown => {
                match self.focus {
                    Focus::Red => self.red = self.red.saturating_sub(20),
                    Focus::Green => self.green = self.green.saturating_sub(20),
                    Focus::Blue => self.blue = self.blue.saturating_sub(20),
                    _ => {}
                }
                self.sync_hex();
                true
            }
            IpcKey::Backspace => {
                if self.focus == Focus::Hex {
                    self.hex.pop();
                }
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                match self.focus {
                    Focus::Hex => {
                        if c.is_ascii_hexdigit() || c == '#' {
                            self.hex.push(c.to_ascii_uppercase());
                            let hex = self.hex.clone();
                            let _ = self.parse_hex(&hex);
                        }
                    }
                    Focus::Red | Focus::Green | Focus::Blue => {
                        if c.is_ascii_digit() {
                            let val: u16 = match self.focus {
                                Focus::Red => {
                                    u16::from(self.red) * 10 + c.to_digit(10).unwrap() as u16
                                }
                                Focus::Green => {
                                    u16::from(self.green) * 10 + c.to_digit(10).unwrap() as u16
                                }
                                Focus::Blue => {
                                    u16::from(self.blue) * 10 + c.to_digit(10).unwrap() as u16
                                }
                                _ => 0,
                            };
                            let clamped = val.min(255) as u8;
                            match self.focus {
                                Focus::Red => self.red = clamped,
                                Focus::Green => self.green = clamped,
                                Focus::Blue => self.blue = clamped,
                                _ => {}
                            }
                            self.sync_hex();
                        }
                    }
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn copy_current(&mut self) {
        if self.hex.is_empty() {
            return;
        }
        match copy_to_clipboard(&self.hex) {
            Ok(()) => self.status = format!("Copied {}", self.hex),
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
        title: Some(" Color Picker ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    // Color preview box
    let preview_x: u16 = 2;
    let preview_y: u16 = 2;
    let preview_w: u16 = 12;
    let preview_h: u16 = 5;
    let color_rgb = [app.red, app.green, app.blue];
    cmds.push(RenderCmd::Rect {
        x: preview_x,
        y: preview_y,
        w: preview_w,
        h: preview_h,
        bg: color_rgb,
    });
    cmds.push(RenderCmd::Border {
        x: preview_x,
        y: preview_y,
        w: preview_w,
        h: preview_h,
        fg: t.border,
        borders: BORDER_ALL,
        bg: Some(color_rgb),
        title: None,
        title_fg: None,
        title_dash_fg: None,
        border_type: None,
    });

    // Values
    let val_x = preview_x + preview_w + 2;
    let focus_prefix = |f: Focus| -> &str {
        if app.focus == f {
            ">"
        } else {
            " "
        }
    };

    let lines = vec![
        format!(
            "{} Hex  {}",
            focus_prefix(Focus::Hex),
            if app.hex.is_empty() {
                "#______"
            } else {
                &app.hex
            }
        ),
        format!("{} R    {}", focus_prefix(Focus::Red), app.red),
        format!("{} G    {}", focus_prefix(Focus::Green), app.green),
        format!("{} B    {}", focus_prefix(Focus::Blue), app.blue),
    ];
    for (i, line) in lines.into_iter().enumerate() {
        cmds.push(RenderCmd::Text {
            x: val_x,
            y: preview_y + i as u16,
            text: line,
            fg: Some(t.text),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    }

    let (hue, sat, light) = app.hsl();
    cmds.push(RenderCmd::Text {
        x: val_x,
        y: preview_y + 4,
        text: format!("    HSL  {hue:.0}\u{b0} {sat:.0}% {light:.0}%"),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    // History
    let hist_y = preview_y + preview_h + 1;
    cmds.push(RenderCmd::Text {
        x: 2,
        y: hist_y,
        text: "History".into(),
        fg: Some(t.text),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let list_start = hist_y + 1;
    let bottom_space = 3;
    let list_h = h.saturating_sub(list_start + bottom_space).max(1);
    let list_w = w.saturating_sub(4);
    let visible = list_h as usize;
    let total = app.history.len();
    let start = if total <= visible {
        0
    } else {
        app.cursor
            .saturating_sub(visible / 2)
            .min(total.saturating_sub(visible))
    };

    let items: Vec<String> = app
        .history
        .iter()
        .skip(start)
        .take(visible)
        .map(|hex| {
            let hex_clean = hex.trim_start_matches('#');
            if hex_clean.len() == 6 {
                let r = u8::from_str_radix(&hex_clean[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex_clean[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex_clean[4..6], 16).unwrap_or(0);
                let (hue, sat, light) = {
                    let rf = r as f64 / 255.0;
                    let gf = g as f64 / 255.0;
                    let bf = b as f64 / 255.0;
                    let mx = rf.max(gf).max(bf);
                    let mn = rf.min(gf).min(bf);
                    let l = (mx + mn) / 2.0;
                    let s = if (mx - mn).abs() < 0.001 {
                        0.0
                    } else {
                        let d = mx - mn;
                        if l > 0.5 {
                            d / (2.0 - mx - mn) * 100.0
                        } else {
                            d / (mx + mn) * 100.0
                        }
                    };
                    let h = if (mx - rf).abs() < 0.001 {
                        ((gf - bf) / (mx - mn) + if gf < bf { 6.0 } else { 0.0 }) * 60.0
                    } else if (mx - gf).abs() < 0.001 {
                        ((bf - rf) / (mx - mn) + 2.0) * 60.0
                    } else {
                        ((rf - gf) / (mx - mn) + 4.0) * 60.0
                    };
                    (h, s, l * 100.0)
                };
                format!(
                    "{hex}  rgb({},{},{})  hsl({:.0}\u{b0},{:.0}%,{:.0}%)",
                    r, g, b, hue, sat, light
                )
            } else {
                hex.clone()
            }
        })
        .collect();

    let vis_sel = if app.cursor >= start && app.cursor < start + visible {
        Some(app.cursor - start)
    } else {
        None
    };

    cmds.push(RenderCmd::List {
        x: 2,
        y: list_start,
        w: list_w,
        h: list_h,
        items,
        selected: vis_sel,
        style: TextStyle {
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(t.inverted_text),
            bg: Some(t.highlight),
            bold: true,
            modifiers: 0,
        },
    });

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
        text: "type hex #RRGGBB \u{b7} tab focus \u{b7} \u{2191}\u{2193} adjust/digits \u{b7} pgup/pgdn \u{b7} c/enter copy \u{b7} esc".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    cmds
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
    vec![("Utilities".into(), "Open color picker".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": [],
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
                | HostMsg::Mouse { .. },
            ) => false,
            Err(e) => {
                log::error!("[color-picker] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
