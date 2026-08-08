use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::thread;

mod api;

mod types;
mod ui;

use api::*;
use santui_ipc::mpv::*;
use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};
use santui_ipc::theme::default_theme;
use types::*;
use ui::*;

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
    translations: Vec<Edition>,
    reciters: Vec<Edition>,
    editions_loading: bool,
    picker: Option<Picker>,
    selected_surah: usize,
    selected_ayah: usize,
    scroll: usize,
    search: String,
    search_mode: bool,
    picker_cursor: usize,
    fetching: bool,
    status: String,
    rx_fetch: Option<mpsc::Receiver<FetchMsg>>,
    rx_editions: Option<mpsc::Receiver<FetchMsg>>,
    tx_mpv: Option<mpsc::Sender<MpvCmd>>,
    rx_mpv: Option<mpsc::Receiver<MpvMsg>>,
    mpv_thread: Option<thread::JoinHandle<()>>,
    audio_state: AudioState,
    play_surah_mode: bool,
    playing_surah: Option<u16>,
    repeat_ayah: bool,
    play_on_load: bool,
    playlist_mode: bool,
    cursor_visible: bool,
    tick_count: u64,
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
            translations: Vec::new(),
            reciters: Vec::new(),
            editions_loading: false,
            picker: None,
            selected_surah: 0,
            selected_ayah: 0,
            scroll: 0,
            search: String::new(),
            search_mode: false,
            picker_cursor: 0,
            fetching: false,
            status: String::new(),
            rx_fetch: None,
            rx_editions: None,
            tx_mpv: None,
            rx_mpv: None,
            mpv_thread: None,
            audio_state: AudioState::Stopped,
            play_surah_mode: false,
            playing_surah: None,
            repeat_ayah: false,
            play_on_load: false,
            playlist_mode: false,
            cursor_visible: true,
            tick_count: 0,
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
        self.start_fetch_editions();
        self.init_audio();
        self.dirty = true;
    }

    fn init_audio(&mut self) {
        let (tx_cmd, rx_cmd) = mpsc::channel();
        let (tx_msg, rx_msg) = mpsc::channel();
        match Mpv::new("santui-quran", &[]) {
            Ok((mpv, _errors)) => {
                self.tx_mpv = Some(tx_cmd);
                self.rx_mpv = Some(rx_msg);
                self.audio_state = AudioState::Stopped;
                self.mpv_thread = Some(thread::spawn(move || mpv_thread(mpv, rx_cmd, tx_msg)));
            }
            Err(e) => self.audio_state = AudioState::Unavailable(e.to_string()),
        }
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.dirty = true;
        if let Some(picker) = self.picker {
            return self.handle_picker_key(key, picker);
        }
        match self.screen {
            Screen::SurahList => self.handle_list_key(key),
            Screen::Reader => self.handle_reader_key(key),
        }
    }

    fn handle_picker_key(&mut self, key: IpcKey, picker: Picker) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.picker_cursor = self.picker_cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.picker_options(picker).len().saturating_sub(1);
                self.picker_cursor = self.picker_cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::PageUp => {
                let page = (self.area.h.saturating_sub(6).max(4) as usize).saturating_sub(1);
                self.picker_cursor = self.picker_cursor.saturating_sub(page);
                true
            }
            IpcKey::PageDown => {
                let max = self.picker_options(picker).len().saturating_sub(1);
                let page = (self.area.h.saturating_sub(6).max(4) as usize).saturating_sub(1);
                self.picker_cursor = (self.picker_cursor + page).min(max);
                true
            }
            IpcKey::Enter => {
                self.apply_picker_selection(picker);
                true
            }
            IpcKey::Esc => {
                self.picker = None;
                true
            }
            _ => false,
        }
    }

    fn picker_options(&self, picker: Picker) -> &[Edition] {
        match picker {
            Picker::Translation => &self.translations,
            Picker::Reciter => &self.reciters,
        }
    }

    fn open_picker(&mut self, picker: Picker) {
        let current = match picker {
            Picker::Translation => self.prefs.translation_edition.clone(),
            Picker::Reciter => self.prefs.reciter.clone(),
        };
        let cursor = self
            .picker_options(picker)
            .iter()
            .position(|e| e.identifier == current)
            .unwrap_or(0);
        self.picker_cursor = cursor;
        if self.picker_options(picker).is_empty() && !self.editions_loading {
            self.start_fetch_editions();
        }
        self.picker = Some(picker);
    }

    fn apply_picker_selection(&mut self, picker: Picker) {
        let Some(edition) = self.picker_options(picker).get(self.picker_cursor).cloned() else {
            return;
        };
        match picker {
            Picker::Translation => self.prefs.translation_edition = edition.identifier.clone(),
            Picker::Reciter => self.prefs.reciter = edition.identifier.clone(),
        }
        self.save_prefs();
        self.stop_audio();
        self.status = format!("{}: {}", picker_label(picker), edition.display_name());
        let staying_in_reader = self.screen == Screen::Reader;
        if staying_in_reader {
            let current = self.current_surah_number();
            self.content_cache.retain(|&k, _| Some(k) == current);
            self.picker = None;
            self.refetch_current_surah();
        } else {
            self.content_cache.clear();
            self.picker = None;
        }
    }

    fn refetch_current_surah(&mut self) {
        let Some(number) = self.current_surah_number() else {
            return;
        };
        let Some(summary) = self.content_cache.get(&number).map(|c| c.summary.clone()) else {
            return;
        };
        if self.fetching {
            return;
        }
        let translation = self.prefs.translation_edition.clone();
        let reciter = self.prefs.reciter.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.fetching = true;
        self.status = format!("Fetching Surah {}...", summary.english_name);
        thread::spawn(move || {
            let _ = tx.send(FetchMsg::Surah(fetch_surah_content(
                summary,
                &translation,
                &reciter,
            )));
        });
    }

    fn handle_list_key(&mut self, key: IpcKey) -> bool {
        if self.search_mode {
            return self.handle_list_search_key(key);
        }
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
                let page = (self.area.h.saturating_sub(6).max(4) as usize).saturating_sub(1);
                self.selected_surah = self.selected_surah.saturating_sub(page);
                true
            }
            IpcKey::PageDown => {
                let max = self.filtered_surahs().len().saturating_sub(1);
                let page = (self.area.h.saturating_sub(6).max(4) as usize).saturating_sub(1);
                self.selected_surah = (self.selected_surah + page).min(max);
                true
            }
            IpcKey::Enter => {
                self.open_selected_surah();
                true
            }
            IpcKey::Char('/') => {
                self.search.clear();
                self.search_mode = true;
                self.cursor_visible = true;
                self.status = "Search surahs".into();
                true
            }
            IpcKey::Char('e') => {
                self.open_picker(Picker::Translation);
                true
            }
            IpcKey::Char('r') => {
                self.open_picker(Picker::Reciter);
                true
            }
            IpcKey::Char('p') => {
                self.play_selected_surah();
                true
            }
            IpcKey::Char('s') => {
                self.stop_audio();
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_list_search_key(&mut self, key: IpcKey) -> bool {
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
                let page = (self.area.h.saturating_sub(6).max(4) as usize).saturating_sub(1);
                self.selected_surah = self.selected_surah.saturating_sub(page);
                true
            }
            IpcKey::PageDown => {
                let max = self.filtered_surahs().len().saturating_sub(1);
                let page = (self.area.h.saturating_sub(6).max(4) as usize).saturating_sub(1);
                self.selected_surah = (self.selected_surah + page).min(max);
                true
            }
            IpcKey::Enter => {
                self.search_mode = false;
                self.open_selected_surah();
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
            IpcKey::Esc => {
                self.search.clear();
                self.search_mode = false;
                self.status.clear();
                self.selected_surah = 0;
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.search.push(c);
                self.selected_surah = 0;
                true
            }
            _ => false,
        }
    }

    fn handle_reader_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.selected_ayah = self.selected_ayah.saturating_sub(1);
                self.adjust_scroll();
                if let Some(surah) = self.current_surah_number() {
                    self.track_ayah(surah, self.selected_ayah as u16 + 1);
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self
                    .current_content()
                    .map(|c| c.ayahs.len().saturating_sub(1))
                    .unwrap_or(0);
                self.selected_ayah = self.selected_ayah.min(max).saturating_add(1).min(max);
                self.adjust_scroll();
                if let Some(surah) = self.current_surah_number() {
                    self.track_ayah(surah, self.selected_ayah as u16 + 1);
                }
                true
            }
            IpcKey::PageUp => {
                self.selected_ayah = self.selected_ayah.saturating_sub(10);
                self.adjust_scroll();
                if let Some(surah) = self.current_surah_number() {
                    self.track_ayah(surah, self.selected_ayah as u16 + 1);
                }
                true
            }
            IpcKey::PageDown => {
                let max = self
                    .current_content()
                    .map(|c| c.ayahs.len().saturating_sub(1))
                    .unwrap_or(0);
                self.selected_ayah = (self.selected_ayah + 10).min(max);
                self.adjust_scroll();
                if let Some(surah) = self.current_surah_number() {
                    self.track_ayah(surah, self.selected_ayah as u16 + 1);
                }
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
            IpcKey::Char('s') => {
                self.stop_audio();
                true
            }
            IpcKey::Char('a') => {
                self.selected_ayah = 0;
                self.scroll = 0;
                self.play_all_ayahs();
                true
            }
            IpcKey::Char('p') => {
                self.toggle_play_pause();
                true
            }
            IpcKey::Char('e') => {
                self.open_picker(Picker::Translation);
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
        self.tick_count += 1;
        if self.search_mode && self.tick_count.is_multiple_of(6) {
            self.cursor_visible = !self.cursor_visible;
            self.dirty = true;
        }
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
                Ok(FetchMsg::Editions(result)) => self.handle_editions_result(result),
                Err(mpsc::TryRecvError::Empty) => self.rx_fetch = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.fetching = false;
                    self.status = "Fetch worker stopped".into();
                    self.dirty = true;
                }
            }
        }
        if let Some(rx) = self.rx_editions.take() {
            match rx.try_recv() {
                Ok(FetchMsg::Editions(result)) => self.handle_editions_result(result),
                Ok(_) => self.rx_editions = Some(rx),
                Err(mpsc::TryRecvError::Empty) => self.rx_editions = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => self.editions_loading = false,
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
                self.status = String::new();
                if let Some(last) = self.prefs.last_surah
                    && let Some(idx) = self.surahs.iter().position(|s| s.number == last)
                {
                    self.selected_surah = idx;
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
                let last = self.prefs.per_surah_ayah.get(&number).copied().unwrap_or(1);
                self.selected_ayah = last.saturating_sub(1) as usize;
                self.adjust_scroll();
                self.status.clear();
                if self.play_on_load {
                    self.play_on_load = false;
                    self.play_surah_mode = true;
                    self.play_all_ayahs();
                } else {
                    self.screen = Screen::Reader;
                }
                self.dirty = true;
            }
            Err(e) => {
                self.status = format!("Surah fetch error: {e}");
                self.dirty = true;
            }
        }
    }

    fn handle_mpv_msg(&mut self, msg: MpvMsg) {
        match msg {
            MpvMsg::Started { surah, ayah } => {
                self.playing_surah = Some(surah);
                self.audio_state = AudioState::Playing { surah, ayah };
                self.track_ayah(surah, ayah);
            }
            MpvMsg::AyahStarted { index } => {
                self.selected_ayah = index;
                self.adjust_scroll();
                let surah = self.playing_surah.or_else(|| self.current_surah_number());
                if let Some(surah) = surah {
                    let ayah = index as u16 + 1;
                    self.track_ayah(surah, ayah);
                    self.audio_state = AudioState::Playing { surah, ayah };
                }
            }
            MpvMsg::Error(e) => self.audio_state = AudioState::Error(e),
            MpvMsg::EndFile => self.handle_audio_end(),
        }
        self.dirty = true;
    }

    fn handle_audio_end(&mut self) {
        if self.playlist_mode {
            self.audio_state = AudioState::Stopped;
            self.play_surah_mode = false;
            self.playing_surah = None;
            return;
        }
        if self.repeat_ayah {
            self.play_current_ayah();
            return;
        }
        self.audio_state = AudioState::Stopped;
        self.play_surah_mode = false;
        self.playing_surah = None;
    }

    fn start_fetch_surahs(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.fetching = true;
        self.status.clear();
        thread::spawn(move || {
            let _ = tx.send(FetchMsg::SurahList(fetch_surah_list()));
        });
    }

    fn start_fetch_editions(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.rx_editions = Some(rx);
        self.editions_loading = true;
        thread::spawn(move || {
            let _ = tx.send(FetchMsg::Editions(fetch_editions()));
        });
    }

    fn handle_editions_result(&mut self, result: Result<Vec<Edition>, String>) {
        self.editions_loading = false;
        match result {
            Ok(list) => {
                self.translations = translation_editions(&list);
                self.reciters = reciter_editions(&list);
                self.status.clear();
            }
            Err(e) => self.status = format!("Editions error: {e}"),
        }
        self.dirty = true;
    }

    fn open_selected_surah(&mut self) {
        let Some(summary) = self.filtered_surahs().get(self.selected_surah).cloned() else {
            return;
        };
        self.prefs.last_surah = Some(summary.number);
        self.track_ayah(summary.number, 1);
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
        self.status = format!("Fetching Surah {}...", summary.english_name);
        thread::spawn(move || {
            let _ = tx.send(FetchMsg::Surah(fetch_surah_content(
                summary,
                &translation,
                &reciter,
            )));
        });
    }

    fn play_selected_surah(&mut self) {
        let Some(summary) = self.filtered_surahs().get(self.selected_surah).cloned() else {
            return;
        };
        self.prefs.last_surah = Some(summary.number);
        self.track_ayah(summary.number, 1);
        if self.content_cache.contains_key(&summary.number) {
            self.selected_ayah = 0;
            self.scroll = 0;
            self.play_surah_mode = true;
            self.play_all_ayahs();
            return;
        }
        self.play_on_load = true;
        let translation = self.prefs.translation_edition.clone();
        let reciter = self.prefs.reciter.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.fetching = true;
        self.status = format!("Fetching Surah {}...", summary.english_name);
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
        self.playlist_mode = false;
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

    fn play_all_ayahs(&mut self) {
        let Some(content) = self.current_content() else {
            return;
        };
        let surah = content.summary.number;
        let ayahs: Vec<(String, u16, u16)> = content
            .ayahs
            .iter()
            .filter_map(|a| {
                let url = a.audio_url.clone()?;
                Some((url, surah, a.number))
            })
            .collect();
        if ayahs.is_empty() {
            self.audio_state = AudioState::Error("no audio URLs".into());
            return;
        }
        let first_ayah = ayahs[0].2;
        self.playlist_mode = true;
        if let Some(tx) = &self.tx_mpv {
            let _ = tx.send(MpvCmd::PlaySurah { ayahs });
            self.audio_state = AudioState::Buffering {
                surah,
                ayah: first_ayah,
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
        self.playlist_mode = false;
        self.playing_surah = None;
    }

    fn track_ayah(&mut self, surah: u16, ayah: u16) {
        self.prefs.per_surah_ayah.insert(surah, ayah);
        self.save_prefs();
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

fn picker_label(picker: Picker) -> &'static str {
    match picker {
        Picker::Translation => "Translation",
        Picker::Reciter => "Reciter",
    }
}

fn respond(app: &mut App, consumed: bool) {
    santui_ipc::protocol::send_plugin_msg(
        app.render().to_vec(),
        hints(app.screen, app.picker, !app.surahs.is_empty()),
        vec![],
        app.pending_request.take(),
        None,
        consumed,
    );
}

fn mpv_thread(mut mpv: Mpv, rx_cmd: mpsc::Receiver<MpvCmd>, tx_msg: mpsc::Sender<MpvMsg>) {
    let mut playlist: Vec<(String, u16, u16)> = Vec::new();
    let mut playlist_index: usize = 0;
    let mut playlist_replaced = false;
    loop {
        if let Some(ev) = mpv.wait_event_raw(0.1) {
            if ev.event_id == MPV_EVENT_SHUTDOWN {
                break;
            }
            if ev.event_id == MPV_EVENT_START_FILE {
                if playlist_replaced {
                    playlist_replaced = false;
                } else if !playlist.is_empty() && playlist_index + 1 < playlist.len() {
                    playlist_index += 1;
                    let _ = tx_msg.send(MpvMsg::AyahStarted {
                        index: playlist_index,
                    });
                    if playlist_index + 1 == playlist.len() {
                        let _ = tx_msg.send(MpvMsg::EndFile);
                    }
                }
            }
            if ev.event_id == MPV_EVENT_END_FILE && playlist.is_empty() {
                let _ = tx_msg.send(MpvMsg::EndFile);
            }
        }
        while let Ok(cmd) = rx_cmd.try_recv() {
            match cmd {
                MpvCmd::Load { url, surah, ayah } => {
                    playlist.clear();
                    playlist_index = 0;
                    playlist_replaced = false;
                    match mpv.load_url(&url) {
                        Ok(()) => {
                            let _ = tx_msg.send(MpvMsg::Started { surah, ayah });
                        }
                        Err(e) => {
                            let _ = tx_msg.send(MpvMsg::Error(e.to_string()));
                        }
                    }
                }
                MpvCmd::PlaySurah { ayahs } => {
                    if let Some((first_url, surah, ayah)) = ayahs.first().cloned() {
                        playlist = ayahs;
                        playlist_index = 0;
                        playlist_replaced = true;
                        if let Err(e) = mpv.load_url(&first_url) {
                            let _ = tx_msg.send(MpvMsg::Error(e.to_string()));
                            playlist.clear();
                        }
                        for (url, _, _) in &playlist[1..] {
                            let _ = mpv.command(&["loadfile", url, "append"]);
                        }
                        let _ = tx_msg.send(MpvMsg::Started { surah, ayah });
                    }
                }
                MpvCmd::TogglePause => {
                    if let Err(e) = mpv.toggle_pause() {
                        let _ = tx_msg.send(MpvMsg::Error(e.to_string()));
                    }
                }
                MpvCmd::Stop => {
                    playlist.clear();
                    playlist_index = 0;
                    playlist_replaced = false;
                    if let Err(e) = mpv.stop() {
                        let _ = tx_msg.send(MpvMsg::Error(e.to_string()));
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
                log::error!("[quran] parse error: {e}: {trimmed}");
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
        let mut prefs = Preferences::default();
        prefs.last_surah = Some(2);
        prefs.per_surah_ayah.insert(2, 3);
        prefs.per_surah_ayah.insert(36, 5);
        let json = serde_json::to_string(&prefs).unwrap();
        let decoded: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.last_surah, Some(2));
        assert_eq!(*decoded.per_surah_ayah.get(&2).unwrap(), 3);
        assert_eq!(*decoded.per_surah_ayah.get(&36).unwrap(), 5);
        assert_eq!(decoded.display_mode, DisplayMode::Both);
    }

    fn edition(id: &str, name: &str) -> Edition {
        Edition {
            identifier: id.into(),
            name: name.into(),
            english_name: name.into(),
            language: "en".into(),
            format: "text".into(),
            kind: "translation".into(),
        }
    }

    fn test_content() -> SurahContent {
        SurahContent {
            summary: SurahSummary {
                number: 1,
                name: "الفاتحة".into(),
                english_name: "Al-Faatiha".into(),
                english_translation: "The Opening".into(),
                ayah_count: 7,
            },
            ayahs: vec![],
            translation_edition: "en.sahih".into(),
            reciter: "ar.alafasy".into(),
        }
    }

    #[test]
    fn picker_opens_at_current_pref() {
        let mut app = App::default();
        app.translations = vec![
            edition("en.sahih", "Saheeh International"),
            edition("id.indonesian", "Bahasa Indonesia"),
        ];
        app.prefs.translation_edition = "id.indonesian".into();
        app.handle_key(IpcKey::Char('e'));
        assert_eq!(app.picker, Some(Picker::Translation));
        assert_eq!(app.picker_cursor, 1);
        app.handle_key(IpcKey::Esc);
        assert_eq!(app.picker, None);
        app.handle_key(IpcKey::Char('r'));
        assert_eq!(app.picker, Some(Picker::Reciter));
    }

    #[test]
    fn picker_navigates_and_clamps() {
        let mut app = App::default();
        app.translations = vec![
            edition("en.sahih", "Saheeh International"),
            edition("id.indonesian", "Bahasa Indonesia"),
        ];
        app.picker = Some(Picker::Translation);
        app.handle_key(IpcKey::Down);
        assert_eq!(app.picker_cursor, 1);
        app.handle_key(IpcKey::Down);
        assert_eq!(app.picker_cursor, 1);
        app.handle_key(IpcKey::Up);
        assert_eq!(app.picker_cursor, 0);
        app.handle_key(IpcKey::Up);
        assert_eq!(app.picker_cursor, 0);
    }

    #[test]
    fn picker_page_keys_jump_by_page_and_clamp() {
        let mut app = App::default();
        app.area.h = 30;
        app.translations = (0..25)
            .map(|i| edition(&format!("en.e{i:02}"), &format!("Edition {i}")))
            .collect();
        app.picker = Some(Picker::Translation);
        app.handle_key(IpcKey::PageDown);
        assert_eq!(app.picker_cursor, 23);
        app.handle_key(IpcKey::PageDown);
        assert_eq!(app.picker_cursor, 24);
        app.handle_key(IpcKey::PageUp);
        assert_eq!(app.picker_cursor, 1);
        app.handle_key(IpcKey::PageUp);
        assert_eq!(app.picker_cursor, 0);
    }

    #[test]
    fn picker_enter_selects_translation_in_list_mode() {
        let mut app = App::default();
        app.translations = vec![
            edition("en.sahih", "Saheeh International"),
            edition("id.indonesian", "Bahasa Indonesia"),
        ];
        app.content_cache.insert(1, test_content());
        app.picker_cursor = 1;
        app.picker = Some(Picker::Translation);
        app.handle_key(IpcKey::Enter);
        assert_eq!(app.prefs.translation_edition, "id.indonesian");
        assert_eq!(app.picker, None);
        assert!(app.content_cache.is_empty());
    }

    #[test]
    fn picker_enter_selects_reciter() {
        let mut app = App::default();
        app.reciters = vec![
            edition("ar.alafasy", "Alafasy"),
            edition("ar.husary", "Husary"),
        ];
        app.picker_cursor = 1;
        app.picker = Some(Picker::Reciter);
        app.handle_key(IpcKey::Enter);
        assert_eq!(app.prefs.reciter, "ar.husary");
        assert_eq!(app.picker, None);
    }

    #[test]
    fn picker_blocks_other_keys() {
        let mut app = App::default();
        app.picker = Some(Picker::Translation);
        assert!(!app.handle_key(IpcKey::Char('p')));
        assert!(!app.handle_key(IpcKey::Char('/')));
        assert_eq!(app.picker, Some(Picker::Translation));
    }

    #[test]
    fn reader_e_opens_translation_picker() {
        let mut app = App::default();
        app.screen = Screen::Reader;
        app.handle_key(IpcKey::Char('e'));
        assert_eq!(app.picker, Some(Picker::Translation));
    }

    #[test]
    fn reader_picker_enter_keeps_reader_and_refetches() {
        let mut app = App::default();
        app.screen = Screen::Reader;
        app.prefs.last_surah = Some(1);
        app.content_cache.insert(1, test_content());
        app.content_cache.insert(2, test_content());
        app.translations = vec![
            edition("en.sahih", "Saheeh International"),
            edition("id.indonesian", "Bahasa Indonesia"),
        ];
        app.picker_cursor = 1;
        app.picker = Some(Picker::Translation);
        app.handle_key(IpcKey::Enter);
        assert_eq!(app.prefs.translation_edition, "id.indonesian");
        assert_eq!(app.picker, None);
        assert_eq!(app.screen, Screen::Reader);
        assert_eq!(app.content_cache.len(), 1);
        assert!(app.content_cache.contains_key(&1));
        assert!(app.fetching);
        assert!(app.rx_fetch.is_some());
    }

    #[test]
    fn picker_open_refetches_editions_when_empty() {
        let mut app = App::default();
        app.handle_key(IpcKey::Char('e'));
        assert_eq!(app.picker, Some(Picker::Translation));
        assert!(app.editions_loading);
    }

    #[test]
    fn editions_fetch_populates_lists() {
        let mut app = App::default();
        let (tx, rx) = mpsc::channel();
        app.rx_editions = Some(rx);
        app.editions_loading = true;
        let json = r#"{"data":[
            {"identifier":"en.sahih","language":"en","name":"Saheeh International","englishName":"Saheeh International","format":"text","type":"translation","direction":"ltr"},
            {"identifier":"id.indonesian","language":"id","name":"Bahasa Indonesia","englishName":"Unknown","format":"text","type":"translation","direction":"ltr"},
            {"identifier":"ar.alafasy","language":"ar","name":"مشاري العفاسي","englishName":"Alafasy","format":"audio","type":"versebyverse","direction":null},
            {"identifier":"ar.alafasy-2","language":"ar","name":"مشاري العفاسي","englishName":"Alafasy","format":"audio","type":"versebyverse","direction":null},
            {"identifier":"en.walk","language":"en","name":"Ibrahim Walk","englishName":"Ibrahim Walk","format":"audio","type":"versebyverse","direction":null}
        ]}"#;
        let list = parse_editions_value(&serde_json::from_str(json).unwrap()).unwrap();
        let _ = tx.send(FetchMsg::Editions(Ok(list)));
        app.handle_tick();
        assert!(!app.editions_loading);
        assert_eq!(app.translations.len(), 2);
        assert_eq!(app.reciters.len(), 1);
        assert_eq!(app.reciters[0].identifier, "ar.alafasy");
    }

    #[test]
    fn hints_picker_mode() {
        let h = hints(Screen::SurahList, Some(Picker::Translation), false);
        assert_eq!(
            h,
            vec![
                ("\u{2191}\u{2193}".into(), "navigate".into()),
                ("\u{21B5}".into(), "select".into()),
                ("esc".into(), "close".into()),
            ]
        );
    }

    #[test]
    fn hints_reader_includes_translation_key() {
        let h = hints(Screen::Reader, None, true);
        assert!(h.iter().any(|(k, _)| k == "e"));
    }
}
