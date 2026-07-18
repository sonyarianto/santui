use std::collections::BTreeMap;
use std::ffi::CString;
use std::io::{BufRead, BufReader};
use std::sync::{mpsc, Arc};
use std::thread;

use libloading::Library;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginRequest, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use serde::{Deserialize, Serialize};

const DB_KEY: &str = "quran-reader-preferences";
const ARABIC_EDITION: &str = "quran-uthmani";
const DEFAULT_TRANSLATION: &str = "en.sahih";
const DEFAULT_RECITER: &str = "ar.alafasy";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SurahSummary {
    number: u16,
    name: String,
    english_name: String,
    english_translation: String,
    ayah_count: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Ayah {
    number: u16,
    arabic: String,
    translation: String,
    audio_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SurahContent {
    summary: SurahSummary,
    ayahs: Vec<Ayah>,
    translation_edition: String,
    reciter: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum DisplayMode {
    Arabic,
    Translation,
    Both,
}

impl DisplayMode {
    fn next(self) -> Self {
        match self {
            Self::Arabic => Self::Translation,
            Self::Translation => Self::Both,
            Self::Both => Self::Arabic,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Arabic => "Arabic",
            Self::Translation => "Translation",
            Self::Both => "Arabic + Translation",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Preferences {
    translation_edition: String,
    reciter: String,
    display_mode: DisplayMode,
    last_surah: Option<u16>,
    last_ayah: Option<u16>,
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
enum Screen {
    SurahList,
    Reader,
    TranslationPicker,
    ReciterPicker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AudioState {
    Unavailable(String),
    Stopped,
    Buffering { surah: u16, ayah: u16 },
    Playing { surah: u16, ayah: u16 },
    Paused { surah: u16, ayah: u16 },
    Error(String),
}

impl AudioState {
    fn label(&self) -> String {
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

enum FetchMsg {
    SurahList(Result<Vec<SurahSummary>, String>),
    Surah(Result<SurahContent, String>),
}

enum MpvCmd {
    Load { url: String, surah: u16, ayah: u16 },
    TogglePause,
    Stop,
    Quit,
}

enum MpvMsg {
    Started { surah: u16, ayah: u16 },
    EndFile,
    Error(String),
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    prefs: Preferences,
    screen: Screen,
    surahs: Vec<SurahSummary>,
    content_cache: BTreeMap<u16, SurahContent>,
    selected_surah: usize,
    selected_ayah: usize,
    scroll: usize,
    search: String,
    picker_cursor: usize,
    fetching: bool,
    status: String,
    rx_fetch: Option<mpsc::Receiver<FetchMsg>>,
    tx_mpv: Option<mpsc::Sender<MpvCmd>>,
    rx_mpv: Option<mpsc::Receiver<MpvMsg>>,
    mpv_thread: Option<thread::JoinHandle<()>>,
    audio_state: AudioState,
    play_surah_mode: bool,
    repeat_ayah: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 100, h: 30 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet { key: DB_KEY.into() }),
            prefs: Preferences::default(),
            screen: Screen::SurahList,
            surahs: Vec::new(),
            content_cache: BTreeMap::new(),
            selected_surah: 0,
            selected_ayah: 0,
            scroll: 0,
            search: String::new(),
            picker_cursor: 0,
            fetching: false,
            status: "Fetching surah list…".into(),
            rx_fetch: None,
            tx_mpv: None,
            rx_mpv: None,
            mpv_thread: None,
            audio_state: AudioState::Stopped,
            play_surah_mode: false,
            repeat_ayah: false,
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(tx) = &self.tx_mpv {
            let _ = tx.send(MpvCmd::Quit);
        }
        if let Some(handle) = self.mpv_thread.take() {
            let _ = handle.join();
        }
    }
}

impl App {
    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.start_fetch_surahs();
        self.init_audio();
        self.dirty = true;
    }

    fn init_audio(&mut self) {
        let (tx_cmd, rx_cmd) = mpsc::channel();
        let (tx_msg, rx_msg) = mpsc::channel();
        match Mpv::new() {
            Ok(mpv) => {
                self.tx_mpv = Some(tx_cmd);
                self.rx_mpv = Some(rx_msg);
                self.audio_state = AudioState::Stopped;
                self.mpv_thread = Some(thread::spawn(move || mpv_thread(mpv, rx_cmd, tx_msg)));
            }
            Err(e) => self.audio_state = AudioState::Unavailable(e),
        }
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.dirty = true;
        match self.screen {
            Screen::SurahList => self.handle_list_key(key),
            Screen::Reader => self.handle_reader_key(key),
            Screen::TranslationPicker => self.handle_translation_key(key),
            Screen::ReciterPicker => self.handle_reciter_key(key),
        }
    }

    fn handle_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.selected_surah = self.selected_surah.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.filtered_surahs().len().saturating_sub(1);
                self.selected_surah = self.selected_surah.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::PageUp => {
                let page = (self.area.h.saturating_sub(7).max(4) as usize).saturating_sub(1);
                self.selected_surah = self.selected_surah.saturating_sub(page);
                true
            }
            IpcKey::PageDown => {
                let max = self.filtered_surahs().len().saturating_sub(1);
                let page = (self.area.h.saturating_sub(7).max(4) as usize).saturating_sub(1);
                self.selected_surah = (self.selected_surah + page).min(max);
                true
            }
            IpcKey::Enter => {
                self.open_selected_surah();
                true
            }
            IpcKey::Char('s') | IpcKey::Char('/') => {
                self.search.clear();
                self.status = "Search surahs".into();
                true
            }
            IpcKey::Char('e') => {
                self.screen = Screen::TranslationPicker;
                self.picker_cursor = translation_options()
                    .iter()
                    .position(|e| *e == self.prefs.translation_edition)
                    .unwrap_or(0);
                true
            }
            IpcKey::Char('r') => {
                self.screen = Screen::ReciterPicker;
                self.picker_cursor = reciter_options()
                    .iter()
                    .position(|e| *e == self.prefs.reciter)
                    .unwrap_or(0);
                true
            }
            IpcKey::Char('R') => {
                self.start_fetch_surahs();
                true
            }
            IpcKey::Backspace => {
                if !self.search.is_empty() {
                    self.search.pop();
                    self.selected_surah = 0;
                    true
                } else {
                    false
                }
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.search.push(c);
                self.selected_surah = 0;
                true
            }
            IpcKey::Esc if !self.search.is_empty() => {
                self.search.clear();
                self.selected_surah = 0;
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_reader_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.selected_ayah = self.selected_ayah.saturating_sub(1);
                self.adjust_scroll();
                self.save_prefs();
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self
                    .current_content()
                    .map(|c| c.ayahs.len().saturating_sub(1))
                    .unwrap_or(0);
                self.selected_ayah = self.selected_ayah.min(max).saturating_add(1).min(max);
                self.adjust_scroll();
                self.save_prefs();
                true
            }
            IpcKey::PageUp => {
                self.selected_ayah = self.selected_ayah.saturating_sub(10);
                self.adjust_scroll();
                self.save_prefs();
                true
            }
            IpcKey::PageDown => {
                let max = self
                    .current_content()
                    .map(|c| c.ayahs.len().saturating_sub(1))
                    .unwrap_or(0);
                self.selected_ayah = (self.selected_ayah + 10).min(max);
                self.adjust_scroll();
                self.save_prefs();
                true
            }
            IpcKey::Char('t') => {
                self.prefs.display_mode = self.prefs.display_mode.next();
                self.save_prefs();
                true
            }
            IpcKey::Char('r') => {
                self.repeat_ayah = !self.repeat_ayah;
                self.status = format!(
                    "Repeat ayah {}",
                    if self.repeat_ayah { "on" } else { "off" }
                );
                true
            }
            IpcKey::Char('x') => {
                self.stop_audio();
                true
            }
            IpcKey::Char('a') => {
                self.play_surah_mode = true;
                self.play_current_ayah();
                true
            }
            IpcKey::Char(' ') => {
                self.toggle_play_pause();
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::SurahList;
                true
            }
            _ => false,
        }
    }

    fn handle_translation_key(&mut self, key: IpcKey) -> bool {
        let options = translation_options();
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.picker_cursor = self.picker_cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = options.len().saturating_sub(1);
                self.picker_cursor = self.picker_cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Enter => {
                self.prefs.translation_edition = options[self.picker_cursor].into();
                self.content_cache.clear();
                self.screen = Screen::SurahList;
                self.save_prefs();
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::SurahList;
                true
            }
            _ => false,
        }
    }

    fn handle_reciter_key(&mut self, key: IpcKey) -> bool {
        let options = reciter_options();
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.picker_cursor = self.picker_cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = options.len().saturating_sub(1);
                self.picker_cursor = self.picker_cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Enter => {
                self.prefs.reciter = options[self.picker_cursor].into();
                self.content_cache.clear();
                self.screen = Screen::SurahList;
                self.save_prefs();
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::SurahList;
                true
            }
            _ => false,
        }
    }

    fn handle_tick(&mut self) {
        if let Some(rx) = self.rx_fetch.take() {
            match rx.try_recv() {
                Ok(FetchMsg::SurahList(result)) => {
                    self.fetching = false;
                    self.handle_surah_list(result);
                    self.dirty = true;
                }
                Ok(FetchMsg::Surah(result)) => {
                    self.fetching = false;
                    self.handle_surah_content(result);
                    self.dirty = true;
                }
                Err(mpsc::TryRecvError::Empty) => self.rx_fetch = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.fetching = false;
                    self.status = "Fetch worker stopped".into();
                    self.dirty = true;
                }
            }
        }
        if let Some(rx) = self.rx_mpv.take() {
            while let Ok(msg) = rx.try_recv() {
                self.handle_mpv_msg(msg);
            }
            self.rx_mpv = Some(rx);
        }
    }

    fn handle_surah_list(&mut self, result: Result<Vec<SurahSummary>, String>) {
        match result {
            Ok(list) => {
                self.surahs = list;
                self.status = format!("Loaded {} surahs", self.surahs.len());
                if let Some(last) = self.prefs.last_surah {
                    if let Some(idx) = self.surahs.iter().position(|s| s.number == last) {
                        self.selected_surah = idx;
                    }
                }
            }
            Err(e) => self.status = format!("Surah list error: {e}"),
        }
    }

    fn handle_surah_content(&mut self, result: Result<SurahContent, String>) {
        match result {
            Ok(content) => {
                let number = content.summary.number;
                self.content_cache.insert(number, content);
                self.screen = Screen::Reader;
                self.selected_ayah = self.prefs.last_ayah.unwrap_or(1).saturating_sub(1) as usize;
                self.adjust_scroll();
                self.status = "Surah loaded".into();
            }
            Err(e) => self.status = format!("Surah fetch error: {e}"),
        }
    }

    fn handle_mpv_msg(&mut self, msg: MpvMsg) {
        match msg {
            MpvMsg::Started { surah, ayah } => {
                self.audio_state = AudioState::Playing { surah, ayah }
            }
            MpvMsg::Error(e) => self.audio_state = AudioState::Error(e),
            MpvMsg::EndFile => self.handle_audio_end(),
        }
        self.dirty = true;
    }

    fn handle_audio_end(&mut self) {
        if self.repeat_ayah {
            self.play_current_ayah();
            return;
        }
        if self.play_surah_mode {
            let max = self
                .current_content()
                .map(|c| c.ayahs.len().saturating_sub(1))
                .unwrap_or(0);
            if self.selected_ayah < max {
                self.selected_ayah += 1;
                self.adjust_scroll();
                self.play_current_ayah();
                return;
            }
        }
        self.audio_state = AudioState::Stopped;
        self.play_surah_mode = false;
    }

    fn start_fetch_surahs(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.fetching = true;
        self.status = "Fetching surah list…".into();
        thread::spawn(move || {
            let _ = tx.send(FetchMsg::SurahList(fetch_surah_list()));
        });
    }

    fn open_selected_surah(&mut self) {
        let Some(summary) = self.filtered_surahs().get(self.selected_surah).cloned() else {
            return;
        };
        self.prefs.last_surah = Some(summary.number);
        self.prefs.last_ayah = Some(1);
        self.save_prefs();
        if self.content_cache.contains_key(&summary.number) {
            self.screen = Screen::Reader;
            self.selected_ayah = 0;
            self.scroll = 0;
            return;
        }
        let translation = self.prefs.translation_edition.clone();
        let reciter = self.prefs.reciter.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.fetching = true;
        self.status = format!("Fetching Surah {}…", summary.english_name);
        thread::spawn(move || {
            let _ = tx.send(FetchMsg::Surah(fetch_surah_content(
                summary,
                &translation,
                &reciter,
            )));
        });
    }

    fn filtered_surahs(&self) -> Vec<SurahSummary> {
        let query = self.search.trim().to_lowercase();
        self.surahs
            .iter()
            .filter(|surah| {
                query.is_empty()
                    || surah.number.to_string() == query
                    || surah.english_name.to_lowercase().contains(&query)
                    || surah.english_translation.to_lowercase().contains(&query)
                    || surah.name.contains(&self.search)
            })
            .cloned()
            .collect()
    }

    fn current_surah_number(&self) -> Option<u16> {
        self.prefs.last_surah
    }
    fn current_content(&self) -> Option<&SurahContent> {
        self.current_surah_number()
            .and_then(|n| self.content_cache.get(&n))
    }

    fn adjust_scroll(&mut self) {
        let visible = self.area.h.saturating_sub(8).max(1) as usize;
        if self.selected_ayah < self.scroll {
            self.scroll = self.selected_ayah;
        }
        if self.selected_ayah >= self.scroll + visible {
            self.scroll = self.selected_ayah.saturating_sub(visible.saturating_sub(1));
        }
        self.prefs.last_ayah = Some((self.selected_ayah + 1) as u16);
    }

    fn play_current_ayah(&mut self) {
        if let AudioState::Unavailable(_) = &self.audio_state {
            self.status = "Audio unavailable; reading still works".into();
            return;
        }
        let Some(content) = self.current_content() else {
            return;
        };
        let Some(ayah) = content.ayahs.get(self.selected_ayah) else {
            return;
        };
        let Some(url) = ayah.audio_url.clone() else {
            self.audio_state = AudioState::Error("missing audio URL".into());
            return;
        };
        let surah = content.summary.number;
        let ayah_no = ayah.number;
        if let Some(tx) = &self.tx_mpv {
            let _ = tx.send(MpvCmd::Load {
                url,
                surah,
                ayah: ayah_no,
            });
            self.audio_state = AudioState::Buffering {
                surah,
                ayah: ayah_no,
            };
        }
    }

    fn toggle_play_pause(&mut self) {
        match self.audio_state.clone() {
            AudioState::Playing { surah, ayah } => {
                if let Some(tx) = &self.tx_mpv {
                    let _ = tx.send(MpvCmd::TogglePause);
                }
                self.audio_state = AudioState::Paused { surah, ayah };
            }
            AudioState::Paused { surah, ayah } => {
                if let Some(tx) = &self.tx_mpv {
                    let _ = tx.send(MpvCmd::TogglePause);
                }
                self.audio_state = AudioState::Playing { surah, ayah };
            }
            _ => {
                self.play_surah_mode = false;
                self.play_current_ayah();
            }
        }
    }

    fn stop_audio(&mut self) {
        if let Some(tx) = &self.tx_mpv {
            let _ = tx.send(MpvCmd::Stop);
        }
        self.audio_state = AudioState::Stopped;
        self.play_surah_mode = false;
    }

    fn save_prefs(&mut self) {
        self.pending_request = Some(PluginRequest::DbSet {
            key: DB_KEY.into(),
            value: serde_json::to_string(&self.prefs).unwrap_or_default(),
        });
    }

    fn load_prefs(&mut self, json: &str) {
        if let Ok(prefs) = serde_json::from_str::<Preferences>(json) {
            self.prefs = prefs;
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

fn translation_options() -> Vec<&'static str> {
    vec!["en.sahih", "en.asad", "id.indonesian"]
}
fn reciter_options() -> Vec<&'static str> {
    vec![
        "ar.alafasy",
        "ar.abdulbasitmurattal",
        "ar.husary",
        "ar.minshawi",
    ]
}

fn fetch_json(url: &str) -> Result<serde_json::Value, String> {
    let mut resp = ureq::get(url).call().map_err(|e| e.to_string())?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

fn fetch_surah_list() -> Result<Vec<SurahSummary>, String> {
    parse_surah_list_value(&fetch_json("https://api.alquran.cloud/v1/surah")?)
}

fn parse_surah_list_value(value: &serde_json::Value) -> Result<Vec<SurahSummary>, String> {
    let data = value["data"]
        .as_array()
        .ok_or_else(|| "missing data array".to_string())?;
    let mut out = Vec::new();
    for item in data {
        out.push(SurahSummary {
            number: item["number"]
                .as_u64()
                .ok_or_else(|| "missing surah number".to_string())? as u16,
            name: item["name"].as_str().unwrap_or_default().to_string(),
            english_name: item["englishName"].as_str().unwrap_or_default().to_string(),
            english_translation: item["englishNameTranslation"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            ayah_count: item["numberOfAyahs"].as_u64().unwrap_or(0) as u16,
        });
    }
    Ok(out)
}

fn fetch_surah_content(
    summary: SurahSummary,
    translation: &str,
    reciter: &str,
) -> Result<SurahContent, String> {
    let n = summary.number;
    let arabic = fetch_json(&format!(
        "https://api.alquran.cloud/v1/surah/{n}/{ARABIC_EDITION}"
    ))?;
    let trans = fetch_json(&format!(
        "https://api.alquran.cloud/v1/surah/{n}/{translation}"
    ))?;
    let audio = fetch_json(&format!("https://api.alquran.cloud/v1/surah/{n}/{reciter}"))?;
    let ayahs = parse_surah_ayahs(&arabic, &trans, &audio)?;
    Ok(SurahContent {
        summary,
        ayahs,
        translation_edition: translation.into(),
        reciter: reciter.into(),
    })
}

fn parse_surah_ayahs(
    arabic: &serde_json::Value,
    translation: &serde_json::Value,
    audio: &serde_json::Value,
) -> Result<Vec<Ayah>, String> {
    let arabic_arr = arabic["data"]["ayahs"]
        .as_array()
        .ok_or_else(|| "missing Arabic ayahs".to_string())?;
    let trans_arr = translation["data"]["ayahs"]
        .as_array()
        .ok_or_else(|| "missing translation ayahs".to_string())?;
    let audio_arr = audio["data"]["ayahs"]
        .as_array()
        .ok_or_else(|| "missing audio ayahs".to_string())?;
    let mut out = Vec::new();
    for (idx, item) in arabic_arr.iter().enumerate() {
        let number = item["numberInSurah"].as_u64().unwrap_or((idx + 1) as u64) as u16;
        out.push(Ayah {
            number,
            arabic: item["text"].as_str().unwrap_or_default().to_string(),
            translation: trans_arr
                .get(idx)
                .and_then(|v| v["text"].as_str())
                .unwrap_or_default()
                .to_string(),
            audio_url: audio_arr
                .get(idx)
                .and_then(|v| v["audio"].as_str())
                .map(str::to_string),
        });
    }
    Ok(out)
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let theme = app.theme.clone();
    let w = app.area.w.max(76);
    let h = app.area.h.max(18);
    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: theme.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        borders: BORDER_ALL,
        bg: Some(theme.background_panel),
        title: Some(" Quran ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    match app.screen {
        Screen::SurahList => render_surah_list(app, &mut cmds, &theme, w, h),
        Screen::Reader => render_reader(app, &mut cmds, &theme, w, h),
        Screen::TranslationPicker => render_picker(
            app,
            &mut cmds,
            &theme,
            w,
            h,
            "Translation",
            &translation_options(),
        ),
        Screen::ReciterPicker => {
            render_picker(app, &mut cmds, &theme, w, h, "Reciter", &reciter_options())
        }
    }
    cmds
}

fn render_surah_list(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let list = app.filtered_surahs();
    let header = format!(
        "Surahs: {} · Search: {} · Translation: {} · Reciter: {}",
        app.surahs.len(),
        app.search,
        app.prefs.translation_edition,
        app.prefs.reciter
    );
    push_text(
        cmds,
        2,
        2,
        truncate(&header, w as usize - 4),
        theme.text,
        true,
    );
    let list_h = h.saturating_sub(7).max(4);
    let items: Vec<String> = list
        .iter()
        .map(|s| {
            format!(
                "{:>3}. {:<24} {:<24} {} ayahs",
                s.number, s.english_name, s.english_translation, s.ayah_count
            )
        })
        .collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 4,
        w: w.saturating_sub(4),
        h: list_h,
        items,
        selected: Some(app.selected_surah.min(list.len().saturating_sub(1))),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
    });
    push_text(
        cmds,
        2,
        h.saturating_sub(2),
        status_line(app),
        theme.text_muted,
        false,
    );
}

fn render_reader(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let Some(content) = app.current_content() else {
        push_text(cmds, 2, 4, "No surah loaded", theme.error, true);
        return;
    };
    let header = format!(
        "{} ({}) · {} · mode: {} · audio: {}",
        content.summary.english_name,
        content.summary.name,
        content.summary.english_translation,
        app.prefs.display_mode.label(),
        app.audio_state.label()
    );
    push_text(
        cmds,
        2,
        2,
        truncate(&header, w as usize - 4),
        theme.text,
        true,
    );
    let list_h = h.saturating_sub(7).max(4);
    let items: Vec<String> = content
        .ayahs
        .iter()
        .skip(app.scroll)
        .take(list_h as usize)
        .map(|ayah| ayah_row(ayah, app.prefs.display_mode, w.saturating_sub(8) as usize))
        .collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 4,
        w: w.saturating_sub(4),
        h: list_h,
        items,
        selected: app.selected_ayah.checked_sub(app.scroll),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
    });
    push_text(
        cmds,
        2,
        h.saturating_sub(2),
        status_line(app),
        theme.text_muted,
        false,
    );
}

fn render_picker(
    app: &App,
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    w: u16,
    h: u16,
    title: &str,
    options: &[&str],
) {
    push_text(
        cmds,
        2,
        2,
        format!("Choose {title} · Enter select · Esc cancel"),
        theme.text,
        true,
    );
    let items: Vec<String> = options.iter().map(|s| (*s).to_string()).collect();
    cmds.push(RenderCmd::List {
        x: 2,
        y: 4,
        w: w.saturating_sub(4),
        h: h.saturating_sub(7),
        items,
        selected: Some(app.picker_cursor.min(options.len().saturating_sub(1))),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
    });
    push_text(
        cmds,
        2,
        h.saturating_sub(1),
        status_line(app),
        theme.text_muted,
        false,
    );
}

fn ayah_row(ayah: &Ayah, mode: DisplayMode, width: usize) -> String {
    let text = match mode {
        DisplayMode::Arabic => ayah.arabic.clone(),
        DisplayMode::Translation => ayah.translation.clone(),
        DisplayMode::Both => format!("{}  /  {}", ayah.arabic, ayah.translation),
    };
    format!("{:>3}. {}", ayah.number, truncate(&text, width))
}

fn status_line(app: &App) -> String {
    let repeat = if app.repeat_ayah {
        "repeat on"
    } else {
        "repeat off"
    };
    let fetching = if app.fetching { " · fetching" } else { "" };
    format!("{} · {}{}", app.status, repeat, fetching)
}

fn hints(screen: Screen) -> Vec<(String, String)> {
    match screen {
        Screen::SurahList => vec![
            ("enter".into(), "read".into()),
            ("/".into(), "search".into()),
            ("e".into(), "translation".into()),
            ("r".into(), "reciter".into()),
            ("R".into(), "refresh".into()),
            ("pgup/pgdn".into(), "scroll".into()),
            ("esc".into(), "back".into()),
        ],
        Screen::Reader => vec![
            ("j/k".into(), "scroll".into()),
            ("space".into(), "ayah".into()),
            ("a".into(), "play surah".into()),
            ("x".into(), "stop".into()),
            ("t".into(), "mode".into()),
            ("r".into(), "repeat".into()),
            ("esc".into(), "list".into()),
        ],
        Screen::TranslationPicker | Screen::ReciterPicker => vec![
            ("up/down".into(), "navigate".into()),
            ("enter".into(), "select".into()),
            ("esc".into(), "back".into()),
        ],
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars.saturating_sub(1) {
            out.push('…');
            return out;
        }
        out.push(ch);
    }
    out
}

fn push_text(
    cmds: &mut Vec<RenderCmd>,
    x: u16,
    y: u16,
    text: impl Into<String>,
    fg: [u8; 3],
    bold: bool,
) {
    cmds.push(RenderCmd::Text {
        x,
        y,
        text: text.into(),
        fg: Some(fg),
        bg: None,
        bold,
        modifiers: 0,
    });
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
    vec![("Reference".into(), "Open Quran".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: hints(app.screen),
        palette_commands: palette_commands(),
        request: app.pending_request.take(),
        plugin_message: None,
        consumed,
    };
    let mut out = std::io::stdout().lock();
    let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &msg);
}

// Minimal libmpv wrapper for audio-only recitation playback.
type MpvHandle = std::ffi::c_void;
const MPV_EVENT_NONE: u32 = 0;
const MPV_EVENT_SHUTDOWN: u32 = 1;
const MPV_EVENT_END_FILE: u32 = 25;

type CreateFn = unsafe extern "C" fn() -> *mut MpvHandle;
type InitializeFn = unsafe extern "C" fn(*mut MpvHandle) -> i32;
type SetOptFn = unsafe extern "C" fn(*mut MpvHandle, *const i8, *const i8) -> i32;
type CommandFn = unsafe extern "C" fn(*mut MpvHandle, *const *const i8) -> i32;
type WaitEventFn = unsafe extern "C" fn(*mut MpvHandle, f64) -> *mut MpvEvent;
type DestroyFn = unsafe extern "C" fn(*mut MpvHandle);

#[repr(C)]
struct MpvEvent {
    event_id: u32,
    error: i32,
    reply_userdata: u64,
    data: *mut std::ffi::c_void,
}
struct MpvFuncs {
    create: CreateFn,
    initialize: InitializeFn,
    set_option: SetOptFn,
    command: CommandFn,
    wait_event: WaitEventFn,
    destroy: DestroyFn,
}
struct Mpv {
    handle: *mut MpvHandle,
    _lib: Arc<Library>,
    funcs: Box<MpvFuncs>,
}
unsafe impl Send for Mpv {}

impl Mpv {
    fn new() -> Result<Self, String> {
        let native_names = [
            "libmpv-2.dll",
            "libmpv.so.2",
            "libmpv.so",
            "libmpv.2.dylib",
            "libmpv.dylib",
        ];
        let native_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("native")));
        let lib = unsafe {
            native_dir
                .as_ref()
                .and_then(|d| {
                    native_names
                        .iter()
                        .find_map(|n| Library::new(d.join(n)).ok())
                })
                .or_else(|| {
                    std::env::var_os("SANTUI_NATIVE_DIR").and_then(|d| {
                        native_names
                            .iter()
                            .find_map(|n| Library::new(std::path::PathBuf::from(&d).join(n)).ok())
                    })
                })
                .or_else(|| Library::new("libmpv-2.dll").ok())
                .or_else(|| Library::new("mpv.dll").ok())
                .or_else(|| Library::new("libmpv.so.2").ok())
                .or_else(|| Library::new("libmpv.so").ok())
                .or_else(|| Library::new("/opt/homebrew/lib/libmpv.2.dylib").ok())
                .or_else(|| Library::new("/opt/homebrew/lib/libmpv.dylib").ok())
                .or_else(|| Library::new("/usr/local/lib/libmpv.2.dylib").ok())
                .or_else(|| Library::new("/usr/local/lib/libmpv.dylib").ok())
                .or_else(|| Library::new("libmpv.2.dylib").ok())
                .or_else(|| Library::new("libmpv.dylib").ok())
        }
        .ok_or_else(|| {
            "libmpv not found; install mpv/libmpv or use bundled native deps".to_string()
        })?;
        let lib = Arc::new(lib);
        macro_rules! sym {
            ($name:literal) => {{
                unsafe { lib.get($name).map(|s| *s).map_err(|e| e.to_string())? }
            }};
        }
        let funcs = Box::new(MpvFuncs {
            create: sym!(b"mpv_create\0"),
            initialize: sym!(b"mpv_initialize\0"),
            set_option: sym!(b"mpv_set_option_string\0"),
            command: sym!(b"mpv_command\0"),
            wait_event: sym!(b"mpv_wait_event\0"),
            destroy: sym!(b"mpv_destroy\0"),
        });
        let handle = unsafe { (funcs.create)() };
        if handle.is_null() {
            return Err("mpv_create returned null".into());
        }
        let mpv = Self {
            handle,
            _lib: lib,
            funcs,
        };
        for (k, v) in [
            ("config", "no"),
            ("vo", "null"),
            ("audio-client-name", "santui-quran-reader"),
        ] {
            mpv.set_option(k, v)?;
        }
        mpv.initialize()?;
        Ok(mpv)
    }
    fn set_option(&self, name: &str, value: &str) -> Result<(), String> {
        let n = CString::new(name).map_err(|e| e.to_string())?;
        let v = CString::new(value).map_err(|e| e.to_string())?;
        rc(
            unsafe { (self.funcs.set_option)(self.handle, n.as_ptr(), v.as_ptr()) },
            name,
        )
    }
    fn initialize(&self) -> Result<(), String> {
        rc(
            unsafe { (self.funcs.initialize)(self.handle) },
            "initialize",
        )
    }
    fn command(&self, args: &[&str]) -> Result<(), String> {
        let cstrs = args
            .iter()
            .map(|a| CString::new(*a).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut ptrs: Vec<*const i8> = cstrs.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        rc(
            unsafe { (self.funcs.command)(self.handle, ptrs.as_ptr()) },
            "command",
        )
    }
    fn load_url(&self, url: &str) -> Result<(), String> {
        self.command(&["loadfile", url, "replace"])
    }
    fn toggle_pause(&self) -> Result<(), String> {
        self.command(&["cycle", "pause"])
    }
    fn stop(&self) -> Result<(), String> {
        self.command(&["stop"])
    }
    fn wait_event(&self, timeout: f64) -> Option<u32> {
        unsafe {
            let ev = (self.funcs.wait_event)(self.handle, timeout);
            if ev.is_null() || (*ev).event_id == MPV_EVENT_NONE {
                None
            } else {
                Some((*ev).event_id)
            }
        }
    }
    fn destroy(&self) {
        unsafe { (self.funcs.destroy)(self.handle) }
    }
}
fn rc(code: i32, ctx: &str) -> Result<(), String> {
    if code >= 0 {
        Ok(())
    } else {
        Err(format!("mpv {ctx} failed: {code}"))
    }
}
fn mpv_thread(mpv: Mpv, rx_cmd: mpsc::Receiver<MpvCmd>, tx_msg: mpsc::Sender<MpvMsg>) {
    loop {
        if let Some(id) = mpv.wait_event(0.1) {
            if id == MPV_EVENT_SHUTDOWN {
                break;
            }
            if id == MPV_EVENT_END_FILE {
                let _ = tx_msg.send(MpvMsg::EndFile);
            }
        }
        while let Ok(cmd) = rx_cmd.try_recv() {
            match cmd {
                MpvCmd::Load { url, surah, ayah } => match mpv.load_url(&url) {
                    Ok(()) => {
                        let _ = tx_msg.send(MpvMsg::Started { surah, ayah });
                    }
                    Err(e) => {
                        let _ = tx_msg.send(MpvMsg::Error(e));
                    }
                },
                MpvCmd::TogglePause => {
                    if let Err(e) = mpv.toggle_pause() {
                        let _ = tx_msg.send(MpvMsg::Error(e));
                    }
                }
                MpvCmd::Stop => {
                    if let Err(e) = mpv.stop() {
                        let _ = tx_msg.send(MpvMsg::Error(e));
                    }
                }
                MpvCmd::Quit => {
                    mpv.destroy();
                    return;
                }
            }
        }
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
                app.handle_init(theme, area);
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
            Ok(HostMsg::Key { key, .. }) => app.handle_key(key),
            Ok(HostMsg::Tick) => {
                app.handle_tick();
                false
            }
            Ok(HostMsg::DbValue { key, value }) => {
                if key == DB_KEY {
                    if let Some(json) = value {
                        app.load_prefs(&json);
                    }
                    app.dirty = true;
                }
                false
            }
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::PaletteCommand { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[quran-reader] parse error: {e}: {trimmed}");
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
    const SURAH_LIST: &str = r#"{"data":[{"number":1,"name":"الفاتحة","englishName":"Al-Faatiha","englishNameTranslation":"The Opening","numberOfAyahs":7}]}"#;
    const SURAH_AR: &str = r#"{"data":{"ayahs":[{"numberInSurah":1,"text":"بسم الله"}]}}"#;
    const SURAH_TR: &str =
        r#"{"data":{"ayahs":[{"numberInSurah":1,"text":"In the name of Allah"}]}}"#;
    const SURAH_AUDIO: &str =
        r#"{"data":{"ayahs":[{"numberInSurah":1,"audio":"https://example.com/1.mp3"}]}}"#;

    #[test]
    fn parses_surah_list() {
        let value: serde_json::Value = serde_json::from_str(SURAH_LIST).unwrap();
        let list = parse_surah_list_value(&value).unwrap();
        assert_eq!(list[0].number, 1);
        assert_eq!(list[0].english_name, "Al-Faatiha");
    }

    #[test]
    fn parses_merged_ayahs() {
        let ar: serde_json::Value = serde_json::from_str(SURAH_AR).unwrap();
        let tr: serde_json::Value = serde_json::from_str(SURAH_TR).unwrap();
        let au: serde_json::Value = serde_json::from_str(SURAH_AUDIO).unwrap();
        let ayahs = parse_surah_ayahs(&ar, &tr, &au).unwrap();
        assert_eq!(ayahs[0].arabic, "بسم الله");
        assert_eq!(ayahs[0].translation, "In the name of Allah");
        assert_eq!(
            ayahs[0].audio_url.as_deref(),
            Some("https://example.com/1.mp3")
        );
    }

    #[test]
    fn display_mode_cycles() {
        assert_eq!(DisplayMode::Arabic.next(), DisplayMode::Translation);
        assert_eq!(DisplayMode::Translation.next(), DisplayMode::Both);
        assert_eq!(DisplayMode::Both.next(), DisplayMode::Arabic);
    }

    #[test]
    fn filters_surahs() {
        let mut app = App::default();
        app.surahs = parse_surah_list_value(&serde_json::from_str(SURAH_LIST).unwrap()).unwrap();
        app.search = "opening".into();
        assert_eq!(app.filtered_surahs().len(), 1);
    }

    #[test]
    fn preferences_roundtrip() {
        let prefs = Preferences {
            last_surah: Some(2),
            last_ayah: Some(3),
            ..Preferences::default()
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let decoded: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.last_surah, Some(2));
        assert_eq!(decoded.display_mode, DisplayMode::Both);
    }
}
