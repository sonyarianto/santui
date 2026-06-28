use serde::{Deserialize, Serialize};

/// Bitmask values for `RenderCmd::Border.borders`, matching ratatui `Borders`.
pub const BORDER_NONE: u8 = 0;
pub const BORDER_LEFT: u8 = 1;
pub const BORDER_RIGHT: u8 = 2;
pub const BORDER_TOP: u8 = 4;
pub const BORDER_BOTTOM: u8 = 8;
pub const BORDER_ALL: u8 = 15;

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
        /// Path to the santui config directory (~/.santui).
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
    PaletteCommand {
        index: u32,
    },
    PluginMessage {
        from: String,
        action: String,
        data: String,
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
    },
    /// Renders wrapped text within a rectangle.
    Paragraph {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        text: String,
        style: TextStyle,
        wrap: bool,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginMsg {
    pub commands: Vec<RenderCmd>,
    pub hints: Vec<(String, String)>,
    #[serde(default)]
    pub palette_commands: Vec<(String, String)>,
    #[serde(default)]
    pub request: Option<PluginRequest>,
    /// Whether the last host message was consumed (handled) by the plugin.
    /// Used by the host to decide whether to fall back to default handling
    /// (e.g. Esc → close plugin if the plugin did not consume it).
    #[serde(default)]
    pub consumed: bool,
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
            },
            RenderCmd::Text {
                x: 1,
                y: 2,
                text: "test".into(),
                fg: None,
                bg: Some([0, 0, 0]),
                bold: false,
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
            consumed: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: PluginMsg = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.commands.len(), 1);
        assert_eq!(decoded.hints.len(), 1);
        assert!(decoded.request.is_none());
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
