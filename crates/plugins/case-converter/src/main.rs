use std::io::{BufRead, BufReader};

use convert_case::{Case, Casing};
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    Snake,
    Kebab,
    Camel,
    Pascal,
    UpperSnake,
    Lower,
    Upper,
    Title,
}

impl Variant {
    fn label(self) -> &'static str {
        match self {
            Self::Snake => "snake_case",
            Self::Kebab => "kebab-case",
            Self::Camel => "camelCase",
            Self::Pascal => "PascalCase",
            Self::UpperSnake => "SCREAMING_SNAKE",
            Self::Lower => "lowercase",
            Self::Upper => "UPPERCASE",
            Self::Title => "Title Case",
        }
    }

    fn convert(self, input: &str) -> String {
        match self {
            Self::Snake => input.to_case(Case::Snake),
            Self::Kebab => input.to_case(Case::Kebab),
            Self::Camel => input.to_case(Case::Camel),
            Self::Pascal => input.to_case(Case::Pascal),
            Self::UpperSnake => input.to_case(Case::UpperSnake),
            Self::Lower => input.to_case(Case::Lower),
            Self::Upper => input.to_case(Case::Upper),
            Self::Title => input.to_case(Case::Title),
        }
    }
}

const VARIANTS: &[Variant] = &[
    Variant::Snake,
    Variant::Kebab,
    Variant::Camel,
    Variant::Pascal,
    Variant::UpperSnake,
    Variant::Lower,
    Variant::Upper,
    Variant::Title,
];

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    input: String,
    cursor: usize,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            input: String::new(),
            cursor: 0,
            status:
                "Type to convert \u{b7} \u{2191}\u{2193} select \u{b7} enter/c copy+save \u{b7} esc"
                    .into(),
        }
    }
}

impl App {
    fn current_variant(&self) -> Variant {
        VARIANTS[self.cursor]
    }

    fn current_conversion(&self) -> String {
        if self.input.is_empty() {
            String::new()
        } else {
            self.current_variant().convert(&self.input)
        }
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                if self.cursor < VARIANTS.len().saturating_sub(1) {
                    self.cursor += 1;
                }
                true
            }
            IpcKey::Tab => {
                if self.cursor < VARIANTS.len().saturating_sub(1) {
                    self.cursor += 1;
                } else {
                    self.cursor = 0;
                }
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                self.copy_current();
                true
            }
            IpcKey::Enter => {
                if !self.input.is_empty() {
                    self.copy_current();
                }
                true
            }
            IpcKey::Backspace => {
                self.input.pop();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.input.push(c);
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn copy_current(&mut self) {
        let converted = self.current_conversion();
        if converted.is_empty() {
            return;
        }
        match copy_to_clipboard(&converted) {
            Ok(()) => {
                self.status = format!("Copied {} to clipboard", self.current_variant().label())
            }
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
        title: Some(" Case Converter ".into()),
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

    let out_y = 5;
    cmds.push(RenderCmd::Text {
        x: 2,
        y: out_y,
        text: "Conversions".into(),
        fg: Some(t.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    let list_start = out_y + 1;
    let bottom_space = 3;
    let list_h = h.saturating_sub(list_start + bottom_space).max(1);
    let list_w = w.saturating_sub(4);

    let items: Vec<String> = VARIANTS
        .iter()
        .map(|v| {
            let converted = if app.input.is_empty() {
                v.label().to_string()
            } else {
                v.convert(&app.input)
            };
            format!("{:<20}  {}", v.label(), converted)
        })
        .collect();

    cmds.push(RenderCmd::List {
        x: 2,
        y: list_start,
        w: list_w,
        h: list_h,
        items,
        selected: Some(app.cursor),
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
        ("type".into(), "type".into()),
        ("tab/↑↓".into(), "select".into()),
        ("enter/c".into(), "copy+save".into()),
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
    vec![]
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
                log::error!("[case-converter] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
