use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct Entry {
    input: String,
    slug: String,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    input: String,
    slug: String,
    status: String,
    history: Vec<Entry>,
    cursor: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            input: String::new(),
            slug: String::new(),
            status: "Type to generate slug".into(),
            history: Vec::new(),
            cursor: 0,
        }
    }
}

impl App {
    fn build_slug(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut last_was_sep = true;
        for c in text.chars() {
            if c.is_ascii_alphanumeric() {
                result.push(c.to_ascii_lowercase());
                last_was_sep = false;
            } else if !last_was_sep && (c == ' ' || c == '-' || c == '_' || c == '.' || c == '/') {
                result.push('-');
                last_was_sep = true;
            }
        }
        let trimmed = result.trim_matches('-');
        if trimmed.is_empty() {
            String::new()
        } else {
            trimmed.to_string()
        }
    }

    fn process(&mut self) {
        self.slug = Self::build_slug(&self.input);
    }

    fn add_to_history(&mut self) {
        if self.input.is_empty() || self.slug.is_empty() {
            return;
        }
        self.history.insert(
            0,
            Entry {
                input: self.input.clone(),
                slug: self.slug.clone(),
            },
        );
        self.history.truncate(100);
        self.cursor = 0;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_current();
                true
            }
            IpcKey::Enter => {
                if !self.input.is_empty() {
                    self.add_to_history();
                    self.copy_current();
                }
                true
            }
            IpcKey::Backspace => {
                self.input.pop();
                self.process();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.input.push(c);
                self.process();
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                if self.cursor < self.history.len().saturating_sub(1) {
                    self.cursor += 1;
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn copy_current(&mut self) {
        let slug = if self.cursor == 0 && !self.history.is_empty() {
            Some(self.slug.clone())
        } else {
            self.history.get(self.cursor).map(|e| e.slug.clone())
        };
        let Some(ref slug) = slug else { return };
        if slug.is_empty() {
            return;
        }
        match copy_to_clipboard(slug) {
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
        title: Some(" Slug Generator ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: "Input".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    let input_display = if app.input.is_empty() {
        "(type here)".into()
    } else {
        app.input.clone()
    };
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: input_display,
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let slug_y = 5;
    cmds.push(RenderCmd::Text {
        x: 2,
        y: slug_y,
        text: "Slug".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let slug_box_y = slug_y + 1;
    let slug_box_w = w.saturating_sub(4);
    let slug_box_h = 3;
    cmds.push(RenderCmd::Border {
        x: 2,
        y: slug_box_y,
        w: slug_box_w,
        h: slug_box_h,
        fg: t.accent,
        borders: BORDER_ALL,
        bg: Some(t.background),
        title: None,
        title_fg: None,
        title_dash_fg: None,
        border_type: None,
    });

    let display = if app.slug.is_empty() {
        "(empty)".into()
    } else {
        app.slug.clone()
    };
    cmds.push(RenderCmd::Text {
        x: 4,
        y: slug_box_y + 1,
        text: display,
        fg: Some(t.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let hist_y = slug_box_y + slug_box_h + 1;
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
        .map(|e| format!("{:<30}  {}", e.input, e.slug))
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
    cmds
}

fn hints() -> Vec<(String, String)> {
    vec![
        ("enter".into(), "copy+save".into()),
        ("c".into(), "copy".into()),
        ("up/down".into(), "history".into()),
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
    vec![("Plugins".into(), "Open slug generator".into())]
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
                log::error!("[slug-generator] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
