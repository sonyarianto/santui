use std::io::{BufRead, BufReader, Write};
use std::net::ToSocketAddrs;

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    domain: String,
    results: Vec<String>,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            domain: String::new(),
            results: Vec::new(),
            status: "Enter domain \u{b7} enter lookup \u{b7} c copy \u{b7} esc".into(),
        }
    }
}

impl App {
    fn lookup(&mut self) {
        let domain = self.domain.trim();
        if domain.is_empty() {
            self.status = "Enter a domain name".into();
            return;
        }
        self.results.clear();
        let addr = format!("{domain}:80");
        match addr.to_socket_addrs() {
            Ok(addrs) => {
                let ips: Vec<_> = addrs.map(|a| a.ip()).collect();
                if ips.is_empty() {
                    self.results.push("No records found".into());
                } else {
                    for ip in &ips {
                        let record_type = if ip.is_ipv4() { "A" } else { "AAAA" };
                        self.results.push(format!("{record_type}   {}", ip));
                    }
                }
                self.status = format!("Found {} record(s) for {domain}", ips.len());
            }
            Err(e) => {
                self.results.push(format!("Error: {e}"));
                self.status = format!("Lookup failed: {e}");
            }
        }
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Enter => {
                self.lookup();
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                let text = self.results.join("\n");
                if text.is_empty() {
                    self.status = "Nothing to copy".into();
                } else {
                    match copy_to_clipboard(&text) {
                        Ok(()) => self.status = "Copied results".into(),
                        Err(e) => self.status = format!("Clipboard error: {e}"),
                    }
                }
                true
            }
            IpcKey::Backspace => {
                self.domain.pop();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.domain.push(c);
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
    let w = app.area.w.max(46);
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
        title: Some(" DNS Lookup ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: "Domain:".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: if app.domain.is_empty() {
            "(type domain)".into()
        } else {
            app.domain.clone()
        },
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let box_y = 4;
    let box_w = w.saturating_sub(4);
    let box_h = h.saturating_sub(6).max(4);

    cmds.push(RenderCmd::Border {
        x: 2,
        y: box_y,
        w: box_w,
        h: box_h,
        fg: t.accent,
        borders: BORDER_ALL,
        bg: Some(t.background),
        title: Some(" Results ".into()),
        title_fg: Some(t.accent),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    if app.results.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 4,
            y: box_y + 1,
            text: "Press Enter to look up DNS records".into(),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
    } else {
        for (i, line) in app
            .results
            .iter()
            .enumerate()
            .take(box_h.saturating_sub(2) as usize)
        {
            cmds.push(RenderCmd::Text {
                x: 4,
                y: box_y + 1 + i as u16,
                text: line.clone(),
                fg: Some(t.text),
                bg: None,
                bold: false,
                modifiers: 0,
            });
        }
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
        text: "type domain \u{b7} enter lookup \u{b7} c copy results \u{b7} esc".into(),
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
    vec![("Network & Diagnostics".into(), "Open DNS lookup".into())]
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
                log::error!("[dns-lookup] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
