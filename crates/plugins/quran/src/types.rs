use serde::{Deserialize, Serialize};

pub const DB_KEY: &str = "quran-reader-preferences";
pub const ARABIC_EDITION: &str = "quran-uthmani";
pub const DEFAULT_TRANSLATION: &str = "en.sahih";
pub const DEFAULT_RECITER: &str = "ar.alafasy";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurahSummary {
    pub number: u16,
    pub name: String,
    pub english_name: String,
    pub english_translation: String,
    pub ayah_count: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ayah {
    pub number: u16,
    pub arabic: String,
    pub translation: String,
    pub audio_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurahContent {
    pub summary: SurahSummary,
    pub ayahs: Vec<Ayah>,
    pub translation_edition: String,
    pub reciter: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisplayMode {
    Arabic,
    Translation,
    Both,
}

impl DisplayMode {
    pub fn next(self) -> Self {
        match self {
            Self::Arabic => Self::Translation,
            Self::Translation => Self::Both,
            Self::Both => Self::Arabic,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Arabic => "Arabic",
            Self::Translation => "Translation",
            Self::Both => "Arabic + Translation",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub translation_edition: String,
    pub reciter: String,
    pub display_mode: DisplayMode,
    pub last_surah: Option<u16>,
    pub last_ayah: Option<u16>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            translation_edition: DEFAULT_TRANSLATION.into(),
            reciter: DEFAULT_RECITER.into(),
            display_mode: DisplayMode::Both,
            last_surah: None,
            last_ayah: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    SurahList,
    Reader,
    TranslationPicker,
    ReciterPicker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioState {
    Unavailable(String),
    Stopped,
    Buffering { surah: u16, ayah: u16 },
    Playing { surah: u16, ayah: u16 },
    Paused { surah: u16, ayah: u16 },
    Error(String),
}

impl AudioState {
    pub fn label(&self) -> String {
        match self {
            Self::Unavailable(e) => format!("audio unavailable: {e}"),
            Self::Stopped => "stopped".into(),
            Self::Buffering { surah, ayah } => format!("buffering {surah}:{ayah}"),
            Self::Playing { surah, ayah } => format!("playing {surah}:{ayah}"),
            Self::Paused { surah, ayah } => format!("paused {surah}:{ayah}"),
            Self::Error(e) => format!("audio error: {e}"),
        }
    }
}

pub enum FetchMsg {
    SurahList(Result<Vec<SurahSummary>, String>),
    Surah(Result<SurahContent, String>),
}

pub enum MpvCmd {
    Load { url: String, surah: u16, ayah: u16 },
    PlaySurah { ayahs: Vec<(String, u16, u16)> },
    TogglePause,
    Stop,
    Quit,
}

pub enum MpvMsg {
    Started { surah: u16, ayah: u16 },
    EndFile,
    Error(String),
    AyahStarted { index: usize },
}

pub fn translation_options() -> Vec<&'static str> {
    vec!["en.sahih", "en.asad", "id.indonesian"]
}

pub fn reciter_options() -> Vec<&'static str> {
    vec![
        "ar.alafasy",
        "ar.abdulbasitmurattal",
        "ar.husary",
        "ar.minshawi",
    ]
}
