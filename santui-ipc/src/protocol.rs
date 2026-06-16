use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeData {
    pub text: [u8; 3],
    pub text_muted: [u8; 3],
    pub accent: [u8; 3],
    pub highlight: [u8; 3],
    pub border: [u8; 3],
    pub success: [u8; 3],
    pub error: [u8; 3],
    pub background_panel: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Area {
    pub w: u16,
    pub h: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Enter,
    Esc,
    Backspace,
    Char(char),
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
    Init { theme: ThemeData, area: Area },
    Key { key: IpcKey },
    Tick,
    Focus,
    Blur,
    ThemeChange { theme: ThemeData },
    Resize { area: Area },
    Shutdown,
    UserUpdate { user: Option<UserData> },
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginRequest {
    SignIn { provider: String },
    SignOut,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginMsg {
    pub commands: Vec<RenderCmd>,
    pub hints: Vec<(String, String)>,
    #[serde(default)]
    pub request: Option<PluginRequest>,
}
