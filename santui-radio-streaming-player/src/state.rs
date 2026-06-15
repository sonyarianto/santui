use crate::itunes::TrackInfo;
use crate::stations::Station;
use std::time::Instant;

pub enum PlayState {
    Stopped,
    Playing(String),
    Error(String),
}

pub struct RadioState {
    pub stations: Vec<Station>,
    pub selected: usize,
    pub scroll: usize,
    pub filtered: Vec<usize>,
    pub play_state: PlayState,
    pub current_station: Option<usize>,
    pub song_title: String,
    pub track_info: Option<TrackInfo>,
    pub volume: i64,
    pub start_time: Instant,
    pub scan_msg: Option<String>,
}

impl RadioState {
    pub fn new(stations: Vec<Station>) -> Self {
        let count = stations.len();
        RadioState {
            filtered: (0..count).collect(),
            stations,
            selected: 0,
            scroll: 0,
            play_state: PlayState::Stopped,
            current_station: None,
            song_title: String::new(),
            track_info: None,
            volume: 75,
            start_time: Instant::now(),
            scan_msg: None,
        }
    }

    pub fn ensure_scroll_visible(&mut self, max_visible: usize) {
        let m = max_visible.max(1);
        if self.selected >= self.scroll + m {
            self.scroll = self.selected.saturating_sub(m.saturating_sub(1));
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
    }

    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn select_page_down(&mut self, page_size: usize) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + page_size).min(self.filtered.len() - 1);
        }
    }

    pub fn select_page_up(&mut self, page_size: usize) {
        if self.selected > 0 {
            self.selected = self.selected.saturating_sub(page_size);
        }
    }

    pub fn current_filtered_index(&self) -> usize {
        if self.filtered.is_empty() {
            return 0;
        }
        self.filtered[self.selected.min(self.filtered.len() - 1)]
    }

    pub fn selected_station(&self) -> Option<&Station> {
        if self.filtered.is_empty() {
            return None;
        }
        let idx = self.filtered[self.selected.min(self.filtered.len() - 1)];
        self.stations.get(idx)
    }

    pub fn volume_up(&mut self) {
        self.volume = (self.volume + 5).min(100);
    }

    pub fn volume_down(&mut self) {
        self.volume = (self.volume - 5).max(0);
    }
}
