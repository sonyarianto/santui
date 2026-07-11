use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UuidVersion {
    V4,
    V7,
}

impl UuidVersion {
    fn label(self) -> &'static str {
        match self {
            Self::V4 => "v4 (random)",
            Self::V7 => "v7 (time-ordered)",
        }
    }
}

#[derive(Debug, Clone)]
struct UuidEntry {
    value: String,
    version: UuidVersion,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    version: UuidVersion,
    history: Vec<UuidEntry>,
    cursor: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            version: UuidVersion::V4,
            history: Vec::new(),
            cursor: 0,
            status: "Press g to generate".into(),
        };
        app.generate();
        app.cursor = 0;
        app
    }
}

impl App {
    fn generate(&mut self) {
        let value = match self.version {
            UuidVersion::V4 => uuid::Uuid::new_v4().to_string(),
            UuidVersion::V7 => uuid::Uuid::now_v7().to_string(),
        };
        self.history.insert(
            0,
            UuidEntry {
                value,
                version: self.version,
            },
        );
        self.history.truncate(100);
        self.cursor = 0;
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char('g') if !modifiers.ctrl => {
                self.generate();
                self.status = format!("Generated UUID {}", self.version.label());
                true
            }
            IpcKey::Char('v') if !modifiers.ctrl => {
                self.toggle_version();
                self.generate();
                self.status = format!("Switched to {}", self.version.label());
                true
            }
            IpcKey::Tab => {
                self.toggle_version();
                self.generate();
                self.status = format!("Switched to {}", self.version.label());
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_current();
                true
            }
            IpcKey::Enter => {
                self.copy_current();
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

    fn toggle_version(&mut self) {
        self.version = match self.version {
            UuidVersion::V4 => UuidVersion::V7,
            UuidVersion::V7 => UuidVersion::V4,
        };
    }

    fn copy_current(&mut self) {
        let Some(entry) = self.history.get(self.cursor) else {
            return;
        };
        match copy_to_clipboard(&entry.value) {
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
        title: Some(" UUID Generator ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    // Version indicator
    let ver_y = 2;
    let v4_label = if app.version == UuidVersion::V4 {
        "[v4]".to_string()
    } else {
        "v4 ".into()
    };
    let v7_label = if app.version == UuidVersion::V7 {
        "[v7]".to_string()
    } else {
        "v7 ".into()
    };
    let version_line = format!("Version  {v4_label}  {v7_label}");

    cmds.push(RenderCmd::Text {
        x: 2,
        y: ver_y,
        text: version_line,
        fg: Some(t.text),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    // Inner box around generated UUID
    let uuid_box_y = ver_y + 2;
    let uuid_box_w = w.saturating_sub(4);
    let uuid_box_h = 5;

    cmds.push(RenderCmd::Border {
        x: 2,
        y: uuid_box_y,
        w: uuid_box_w,
        h: uuid_box_h,
        fg: t.accent,
        borders: BORDER_ALL,
        bg: Some(t.background),
        title: None,
        title_fg: None,
        title_dash_fg: None,
        border_type: None,
    });

    if let Some(entry) = app.history.get(app.cursor) {
        let uuid_text = &entry.value;
        let text_w = uuid_box_w.saturating_sub(4) as usize;
        let display = if uuid_text.len() <= text_w {
            uuid_text.clone()
        } else {
            format!("{}{}", &uuid_text[..text_w.saturating_sub(1)], "\u{2026}")
        };
        let text_x = 2 + (uuid_box_w.saturating_sub(display.len() as u16)) / 2;
        let text_y = uuid_box_y + uuid_box_h / 2;

        cmds.push(RenderCmd::Text {
            x: text_x,
            y: text_y,
            text: display,
            fg: Some(t.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        });
    }

    // History section
    let hist_header_y = uuid_box_y + uuid_box_h + 1;
    cmds.push(RenderCmd::Text {
        x: 2,
        y: hist_header_y,
        text: "History".into(),
        fg: Some(t.text),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let list_start_y = hist_header_y + 1;
    let bottom_space = 3;
    let list_h = h.saturating_sub(list_start_y + bottom_space).max(1);
    let list_w = w.saturating_sub(4);

    let visible_count = list_h as usize;
    let total = app.history.len();
    let start = if total <= visible_count {
        0
    } else {
        let off = app.cursor.saturating_sub(visible_count / 2);
        off.min(total.saturating_sub(visible_count))
    };

    let items: Vec<String> = app
        .history
        .iter()
        .skip(start)
        .take(visible_count)
        .map(|e| {
            let ver = match e.version {
                UuidVersion::V4 => "v4",
                UuidVersion::V7 => "v7",
            };
            format!("{:<36}  {}", e.value, ver)
        })
        .collect();

    let vis_selected = if app.cursor >= start && app.cursor < start + visible_count {
        Some(app.cursor - start)
    } else {
        None
    };

    cmds.push(RenderCmd::List {
        x: 2,
        y: list_start_y,
        w: list_w,
        h: list_h,
        items,
        selected: vis_selected,
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

    // Status bar
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

fn hints() -> Vec<(String, String)> {
    vec![
        ("g".into(), "generate".into()),
        ("v".into(), "version".into()),
        ("tab".into(), "version".into()),
        ("c".into(), "copy".into()),
        ("enter".into(), "copy".into()),
        ("esc".into(), "back".into()),
    ]
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("Utilities".into(), "Open UUID generator".into())]
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
                app.generate();
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
                log::error!("[uuid-generator] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_v4_uuid() {
        let mut app = App::default();
        let before = app.history.len();
        app.version = UuidVersion::V4;
        app.generate();
        let entry = &app.history[0];
        assert_eq!(app.history.len(), before + 1);
        assert_eq!(entry.version, UuidVersion::V4);
        assert_eq!(entry.value.len(), 36);
        assert_eq!(entry.value.chars().filter(|&c| c == '-').count(), 4);
    }

    #[test]
    fn generates_v7_uuid() {
        let mut app = App::default();
        let before = app.history.len();
        app.version = UuidVersion::V7;
        app.generate();
        let entry = &app.history[0];
        assert_eq!(app.history.len(), before + 1);
        assert_eq!(entry.version, UuidVersion::V7);
        assert_eq!(entry.value.len(), 36);
    }

    #[test]
    fn toggle_version() {
        let mut app = App::default();
        assert_eq!(app.version, UuidVersion::V4);
        app.toggle_version();
        assert_eq!(app.version, UuidVersion::V7);
        app.toggle_version();
        assert_eq!(app.version, UuidVersion::V4);
    }

    #[test]
    fn cursor_bounds() {
        let mut app = App::default();
        app.history.clear();
        app.history.push(UuidEntry {
            value: "a".repeat(36),
            version: UuidVersion::V4,
        });
        app.history.push(UuidEntry {
            value: "b".repeat(36),
            version: UuidVersion::V4,
        });
        app.cursor = 0;
        app.handle_key(IpcKey::Down, IpcKeyModifiers::default());
        assert_eq!(app.cursor, 0);
        app.handle_key(IpcKey::Up, IpcKeyModifiers::default());
        assert_eq!(app.cursor, 1);
        app.handle_key(IpcKey::Up, IpcKeyModifiers::default());
        assert_eq!(app.cursor, 1);
    }

    #[test]
    fn esc_not_consumed() {
        let mut app = App::default();
        assert!(!app.handle_key(IpcKey::Esc, IpcKeyModifiers::default()));
    }

    #[test]
    fn history_truncates_at_100() {
        let mut app = App::default();
        app.history.clear();
        for _ in 0..150 {
            app.generate();
        }
        assert_eq!(app.history.len(), 100);
    }
}
