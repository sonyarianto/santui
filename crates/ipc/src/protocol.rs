use serde::{Deserialize, Serialize};

/// Bitmask values for `RenderCmd::Border.borders`, matching ratatui `Borders`.
pub const BORDER_NONE: u8 = 0;
pub const BORDER_LEFT: u8 = 1;
pub const BORDER_RIGHT: u8 = 2;
pub const BORDER_TOP: u8 = 4;
pub const BORDER_BOTTOM: u8 = 8;
pub const BORDER_ALL: u8 = 15;

/// Modifier flags matching ratatui `Modifier` bits. Combine with bitwise OR.
pub const MOD_BOLD: u16 = 0b0000_0000_0000_0001;
pub const MOD_DIM: u16 = 0b0000_0000_0000_0010;
pub const MOD_ITALIC: u16 = 0b0000_0000_0000_0100;
pub const MOD_UNDERLINED: u16 = 0b0000_0000_0000_1000;
pub const MOD_SLOW_BLINK: u16 = 0b0000_0000_0001_0000;
pub const MOD_RAPID_BLINK: u16 = 0b0000_0000_0010_0000;
pub const MOD_REVERSED: u16 = 0b0000_0000_0100_0000;
pub const MOD_HIDDEN: u16 = 0b0000_0000_1000_0000;
pub const MOD_CROSSED_OUT: u16 = 0b0000_0001_0000_0000;

/// Border type constants matching ratatui `BorderType`.
pub const BORDER_TYPE_PLAIN: u8 = 0;
pub const BORDER_TYPE_ROUNDED: u8 = 1;
pub const BORDER_TYPE_DOUBLE: u8 = 2;
pub const BORDER_TYPE_THICK: u8 = 3;

/// Alignment constants for `Paragraph`.
pub const ALIGN_LEFT: u8 = 0;
pub const ALIGN_CENTER: u8 = 1;
pub const ALIGN_RIGHT: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeData {
    pub text: [u8; 3],
    pub text_muted: [u8; 3],
    pub accent: [u8; 3],
    pub highlight: [u8; 3],
    pub logo: [u8; 3],
    pub background: [u8; 3],
    pub background_panel: [u8; 3],
    pub background_overlay: [u8; 3],
    pub border: [u8; 3],
    pub success: [u8; 3],
    pub error: [u8; 3],
    pub inverted_text: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Area {
    pub w: u16,
    pub h: u16,
}

/// A single log entry captured by the host's LoggerBuffer and forwarded
/// to the log-viewer plugin via [`HostMsg::LogEntries`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// Modifier key flags sent alongside key events.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct IpcKeyModifiers {
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcKey {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Enter,
    Esc,
    Backspace,
    Tab,
    BackTab,
    Delete,
    Insert,
    Home,
    End,
    Char(char),
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MouseEventKind {
    Down,
    Up,
    Drag,
    Moved,
    ScrollDown,
    ScrollUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IpcMouseEvent {
    pub x: u16,
    pub y: u16,
    pub button: MouseButton,
    pub kind: MouseEventKind,
    pub modifiers: IpcKeyModifiers,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserData {
    pub id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HostMsg {
    Init {
        theme: ThemeData,
        area: Area,
        /// Path to the santui config directory (platform-standard).
        /// Used by the registry plugin for filesystem operations.
        data_dir: String,
    },
    Key {
        key: IpcKey,
        #[serde(default)]
        modifiers: IpcKeyModifiers,
    },
    Tick,
    Focus,
    Blur,
    ThemeChange {
        theme: ThemeData,
    },
    Resize {
        area: Area,
    },
    Shutdown,
    UserUpdate {
        user: Option<UserData>,
    },
    Mouse {
        event: IpcMouseEvent,
    },
    PaletteCommand {
        index: u32,
    },
    PluginMessage {
        from: String,
        action: String,
        data: String,
    },
    /// Response to a `PluginRequest::DbGet` or `DbSet`.
    /// `value` is `None` when the requested key does not exist.
    DbValue {
        key: String,
        #[serde(default)]
        value: Option<String>,
    },
    /// Log entries captured by the host's runtime logger. Sent on every
    /// tick to the log-viewer plugin while there are pending entries.
    LogEntries {
        entries: Vec<LogEntry>,
    },
}

/// Serializable style information for IPC render commands.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct TextStyle {
    #[serde(default)]
    pub fg: Option<[u8; 3]>,
    #[serde(default)]
    pub bg: Option<[u8; 3]>,
    #[serde(default)]
    pub bold: bool,
    /// Bitmask of `MOD_*` constants. Applied in addition to `bold`.
    #[serde(default)]
    pub modifiers: u16,
}

/// A styled span of text for rich inline formatting within `Paragraph`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanData {
    pub text: String,
    #[serde(default)]
    pub style: TextStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderCmd {
    Text {
        x: u16,
        y: u16,
        text: String,
        fg: Option<[u8; 3]>,
        bg: Option<[u8; 3]>,
        bold: bool,
        /// Bitmask of `MOD_*` constants.
        #[serde(default)]
        modifiers: u16,
    },
    Clear {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    },
    /// Fill a rectangle with a background color.
    Rect {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        bg: [u8; 3],
    },
    /// Dim an area: darken existing foreground/background colours and apply
    /// `bg` to cells that have no explicit background.  Analogous to the
    /// host's `DimOverlay` widget.
    Dim {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        bg: [u8; 3],
    },
    /// Draw a box border around a rectangle, with optional background fill and title.
    /// `borders` is a bitmask matching ratatui `Borders` (1=LEFT, 2=RIGHT, 4=TOP, 8=BOTTOM, 15=ALL).
    /// When `bg` and `title` are set, this is equivalent to a native ratatui `Block` with title.
    /// When `title_dash_fg` is set, the title is rendered inline with dashes in that color.
    Border {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        fg: [u8; 3],
        borders: u8,
        bg: Option<[u8; 3]>,
        title: Option<String>,
        title_fg: Option<[u8; 3]>,
        title_dash_fg: Option<[u8; 3]>,
        /// Border type: `BORDER_TYPE_PLAIN` (default), `BORDER_TYPE_ROUNDED`, `BORDER_TYPE_DOUBLE`, `BORDER_TYPE_THICK`.
        #[serde(default)]
        border_type: Option<u8>,
    },
    /// Renders wrapped text within a rectangle. When `spans` is set, each span
    /// can have its own style for inline rich-text formatting.
    Paragraph {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        #[serde(default)]
        text: String,
        #[serde(default)]
        style: TextStyle,
        #[serde(default)]
        wrap: bool,
        /// Optional rich-text spans. When set, `text` is ignored.
        #[serde(default)]
        spans: Option<Vec<SpanData>>,
        /// Text alignment: `ALIGN_LEFT` (default), `ALIGN_CENTER`, `ALIGN_RIGHT`.
        #[serde(default)]
        alignment: Option<u8>,
    },
    /// A scrollable list with selection highlighting.
    List {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        items: Vec<String>,
        selected: Option<usize>,
        style: TextStyle,
        highlight_style: TextStyle,
    },
    /// A table with header, rows, and selection highlighting.
    Table {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        header: Vec<String>,
        header_style: TextStyle,
        rows: Vec<Vec<String>>,
        column_widths: Vec<u16>,
        selected: Option<usize>,
        style: TextStyle,
        highlight_style: TextStyle,
        /// Row index (relative to `rows`) of the "currently active" item
        /// (e.g. the station currently playing). Rendered with `current_style`.
        current_row: Option<usize>,
        #[serde(default)]
        current_style: Option<TextStyle>,
        /// Per-cell styles. `cell_styles[row][col]` sets the style for that cell.
        /// `None` entries use the row's default style.
        #[serde(default)]
        cell_styles: Option<Vec<Vec<Option<TextStyle>>>>,
    },
    /// A progress bar / gauge.
    Gauge {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        /// Progress ratio (0.0 to 1.0).
        ratio: f64,
        /// Optional label rendered in the center of the gauge.
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        style: TextStyle,
        #[serde(default)]
        gauge_style: TextStyle,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginRequest {
    SignIn {
        provider: String,
    },
    SignOut,
    /// Signal that the registry plugin has modified the installed plugin list.
    /// The host should re-read `registry.toml` and refresh the palette.
    PluginsChanged,
    /// Request the host to read a value from the central user_data table.
    /// The host responds with a `HostMsg::DbValue`.
    DbGet {
        key: String,
    },
    /// Request the host to write a value to the central user_data table.
    /// The host responds with a `HostMsg::DbValue` containing the stored value.
    DbSet {
        key: String,
        value: String,
    },
    /// Request the host to activate/launch a plugin by its id.
    LaunchPlugin {
        id: String,
        name: String,
    },
}

/// Outgoing plugin-to-plugin message sent by a plugin to another plugin.
/// The host fills in the `from` field before forwarding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMessage {
    pub to: String,
    pub action: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginMsg {
    pub commands: Vec<RenderCmd>,
    pub hints: Vec<(String, String)>,
    #[serde(default)]
    pub palette_commands: Vec<(String, String)>,
    #[serde(default)]
    pub request: Option<PluginRequest>,
    /// Outgoing plugin-to-plugin message.
    /// The host forwards this to the target plugin.
    #[serde(default)]
    pub plugin_message: Option<PluginMessage>,
    /// Whether the last host message was consumed (handled) by the plugin.
    /// Used by the host to decide whether to fall back to default handling
    /// (e.g. Esc → close plugin if the plugin did not consume it).
    #[serde(default)]
    pub consumed: bool,
}

use std::io::{self, BufRead, Write};

/// Read a `PluginMsg` from a binary length-prefixed bincode frame.
///
/// Auto-detects JSON Lines format: if the first available byte is `{`, falls
/// back to reading a line and parsing with `serde_json`. Otherwise reads a
/// binary frame (`[4-byte LE length][N bytes of bincode]`).
/// Read a `PluginMsg` from a binary length-prefixed bincode frame.
///
/// Format: `[4-byte LE length][N bytes of bincode]`
pub fn read_plugin_msg<R: BufRead>(r: &mut R) -> io::Result<PluginMsg> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    let (decoded, _): (PluginMsg, _) =
        bincode::serde::decode_from_slice(&payload, bincode::config::standard())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(decoded)
}

/// Write a `PluginMsg` as a binary length-prefixed bincode frame.
///
/// Format: `[4-byte LE length][N bytes of bincode]`
pub fn write_plugin_msg<W: Write>(w: &mut W, msg: &PluginMsg) -> io::Result<()> {
    let payload = bincode::serde::encode_to_vec(msg, bincode::config::standard())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let len = payload.len() as u32;
    w.write_all(&len.to_le_bytes())?;
    w.write_all(&payload)?;
    w.flush()
}

/// Read a `HostMsg` from stdin using JSON Lines format (host → plugin
/// direction stays JSON for backward compat).
pub fn read_host_msg_json<R: BufRead>(r: &mut R) -> io::Result<HostMsg> {
    let mut line = String::new();
    r.read_line(&mut line)?;
    if line.is_empty() {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
    serde_json::from_str(trimmed).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_equality() {
        assert_eq!(Area { w: 80, h: 24 }, Area { w: 80, h: 24 });
        assert_ne!(Area { w: 80, h: 24 }, Area { w: 80, h: 25 });
        assert_ne!(Area { w: 80, h: 24 }, Area { w: 81, h: 24 });
    }

    #[test]
    fn theme_data_round_trip() {
        let data = ThemeData {
            text: [220, 220, 220],
            text_muted: [140, 140, 140],
            accent: [157, 124, 216],
            highlight: [250, 178, 131],
            logo: [255, 185, 0],
            background: [20, 20, 20],
            background_panel: [20, 20, 20],
            background_overlay: [10, 10, 10],
            border: [250, 178, 131],
            success: [127, 216, 143],
            error: [224, 108, 117],
            inverted_text: [20, 20, 20],
        };
        let json = serde_json::to_string(&data).unwrap();
        let decoded: ThemeData = serde_json::from_str(&json).unwrap();
        assert_eq!(data.text, decoded.text);
        assert_eq!(data.accent, decoded.accent);
    }

    #[test]
    fn ipc_key_round_trip() {
        let cases = vec![
            IpcKey::Up,
            IpcKey::Down,
            IpcKey::Left,
            IpcKey::Right,
            IpcKey::PageUp,
            IpcKey::PageDown,
            IpcKey::Enter,
            IpcKey::Esc,
            IpcKey::Backspace,
            IpcKey::Tab,
            IpcKey::BackTab,
            IpcKey::Delete,
            IpcKey::Insert,
            IpcKey::Home,
            IpcKey::End,
            IpcKey::Char('x'),
            IpcKey::F(1),
            IpcKey::F(12),
        ];
        for key in cases {
            let json = serde_json::to_string(&key).unwrap();
            let decoded: IpcKey = serde_json::from_str(&json).unwrap();
            assert_eq!(
                serde_json::to_string(&key).unwrap(),
                serde_json::to_string(&decoded).unwrap()
            );
        }
    }

    #[test]
    fn host_msg_init_round_trip() {
        let theme = ThemeData {
            text: [255; 3],
            text_muted: [200; 3],
            accent: [100; 3],
            highlight: [150; 3],
            logo: [180; 3],
            background: [5; 3],
            background_panel: [10; 3],
            background_overlay: [2; 3],
            border: [150; 3],
            success: [0; 3],
            error: [255; 3],
            inverted_text: [200; 3],
        };
        let area = Area { w: 120, h: 30 };
        let msg = HostMsg::Init {
            theme: theme.clone(),
            area,
            data_dir: "/tmp/.santui".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: HostMsg = serde_json::from_str(&json).unwrap();
        match decoded {
            HostMsg::Init {
                theme: t, area: a, ..
            } => {
                assert_eq!(t.text, theme.text);
                assert_eq!(a, area);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn host_msg_round_trip_all_variants() {
        let theme = ThemeData {
            text: [1, 2, 3],
            text_muted: [4, 5, 6],
            accent: [7, 8, 9],
            highlight: [10, 11, 12],
            logo: [13, 14, 15],
            background: [16, 17, 18],
            background_panel: [19, 20, 21],
            background_overlay: [22, 23, 24],
            border: [25, 26, 27],
            success: [28, 29, 30],
            error: [31, 32, 33],
            inverted_text: [34, 35, 36],
        };
        let area = Area { w: 80, h: 24 };
        let msgs: Vec<HostMsg> = vec![
            HostMsg::Init {
                theme: theme.clone(),
                area,
                data_dir: "/tmp/.santui".into(),
            },
            HostMsg::Key {
                key: IpcKey::Char('q'),
                modifiers: IpcKeyModifiers::default(),
            },
            HostMsg::Key {
                key: IpcKey::Char('s'),
                modifiers: IpcKeyModifiers {
                    ctrl: true,
                    ..Default::default()
                },
            },
            HostMsg::Tick,
            HostMsg::Focus,
            HostMsg::Blur,
            HostMsg::ThemeChange {
                theme: theme.clone(),
            },
            HostMsg::Resize {
                area: Area { w: 100, h: 40 },
            },
            HostMsg::Mouse {
                event: IpcMouseEvent {
                    x: 10,
                    y: 5,
                    button: MouseButton::Left,
                    kind: MouseEventKind::Down,
                    modifiers: IpcKeyModifiers::default(),
                },
            },
            HostMsg::Shutdown,
            HostMsg::UserUpdate { user: None },
            HostMsg::UserUpdate {
                user: Some(UserData {
                    id: "42".into(),
                    email: "test@test.com".into(),
                    name: "Test".into(),
                    avatar_url: None,
                    provider: "github".into(),
                }),
            },
        ];
        for msg in msgs {
            let json = serde_json::to_string(&msg).unwrap();
            let decoded: HostMsg = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&decoded).unwrap();
            assert_eq!(json, json2, "round-trip failed for {msg:?}");
        }
    }

    #[test]
    fn render_cmd_round_trip() {
        let cmds = vec![
            RenderCmd::Clear {
                x: 0,
                y: 0,
                w: 80,
                h: 24,
            },
            RenderCmd::Text {
                x: 10,
                y: 5,
                text: "Hello".into(),
                fg: Some([255, 0, 0]),
                bg: None,
                bold: true,
                modifiers: 0,
            },
            RenderCmd::Text {
                x: 1,
                y: 2,
                text: "test".into(),
                fg: None,
                bg: Some([0, 0, 0]),
                bold: false,
                modifiers: 0,
            },
            RenderCmd::Rect {
                x: 1,
                y: 1,
                w: 10,
                h: 5,
                bg: [20, 20, 20],
            },
            RenderCmd::Dim {
                x: 0,
                y: 0,
                w: 30,
                h: 20,
                bg: [22, 23, 24],
            },
            RenderCmd::Border {
                x: 0,
                y: 0,
                w: 60,
                h: 20,
                fg: [250, 178, 131],
                borders: 15,
                bg: Some([20, 20, 20]),
                title: Some("Test".into()),
                title_fg: Some([255, 255, 255]),
                title_dash_fg: Some([250, 178, 131]),
                border_type: None,
            },
        ];
        for cmd in cmds {
            let json = serde_json::to_string(&cmd).unwrap();
            let decoded: RenderCmd = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&decoded).unwrap();
            assert_eq!(json, json2);
        }
    }

    #[test]
    fn plugin_msg_round_trip() {
        let msg = PluginMsg {
            commands: vec![RenderCmd::Clear {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            }],
            hints: vec![("key".into(), "desc".into())],
            palette_commands: vec![],
            request: None,
            plugin_message: None,
            consumed: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: PluginMsg = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.commands.len(), 1);
        assert_eq!(decoded.hints.len(), 1);
        assert!(decoded.request.is_none());
        assert!(decoded.plugin_message.is_none());
    }

    #[test]
    fn plugin_msg_with_request() {
        let msg = PluginMsg {
            commands: vec![],
            hints: vec![],
            palette_commands: vec![],
            request: Some(PluginRequest::SignIn {
                provider: "google".into(),
            }),
            plugin_message: None,
            consumed: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: PluginMsg = serde_json::from_str(&json).unwrap();
        match decoded.request {
            Some(PluginRequest::SignIn { provider }) => assert_eq!(provider, "google"),
            _ => panic!("expected SignIn"),
        }
    }

    #[test]
    fn user_data_round_trip() {
        let user = UserData {
            id: "1".into(),
            email: "user@example.com".into(),
            name: "User".into(),
            avatar_url: Some("https://example.com/avatar.png".into()),
            provider: "github".into(),
        };
        let json = serde_json::to_string(&user).unwrap();
        let decoded: UserData = serde_json::from_str(&json).unwrap();
        assert_eq!(user.id, decoded.id);
        assert_eq!(user.email, decoded.email);
        assert_eq!(user.name, decoded.name);
        assert_eq!(user.avatar_url, decoded.avatar_url);
        assert_eq!(user.provider, decoded.provider);
    }

    #[test]
    fn plugin_request_sign_out() {
        let req = PluginRequest::SignOut;
        let json = serde_json::to_string(&req).unwrap();
        let decoded: PluginRequest = serde_json::from_str(&json).unwrap();
        match decoded {
            PluginRequest::SignOut => {}
            _ => panic!("expected SignOut"),
        }
    }
}
