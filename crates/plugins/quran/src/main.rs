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
    selected_surah: usize,
    selected_ayah: usize,
    scroll: usize,
    search: String,
    search_mode: bool,
    picker_cursor: usize,
    fetching: bool,
    status: String,
    rx_fetch: Option<mpsc::Receiver<FetchMsg>>,
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
            selected_surah: 0,
            selected_ayah: 0,
            scroll: 0,
            search: String::new(),
            search_mode: false,
            picker_cursor: 0,
            fetching: false,
            status: String::new(),
            rx_fetch: None,
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
        match self.screen {
            Screen::SurahList => self.handle_list_key(key),
            Screen::Reader => self.handle_reader_key(key),
            Screen::TranslationPicker => self.handle_translation_key(key),
            Screen::ReciterPicker => self.handle_reciter_key(key),
        }
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
            IpcKey::Char('p') => {
                self.status = "Playing surah...".into();
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
                self.status = String::new();
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
        hints: hints(
            app.screen,
            app.search_mode,
            !app.surahs.is_empty(),
            app.fetching,
        ),
        palette_commands: palette_commands(),
        request: app.pending_request.take(),
        plugin_message: None,
        consumed,
    };
    let mut out = std::io::stdout().lock();
    let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &msg);
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
}
