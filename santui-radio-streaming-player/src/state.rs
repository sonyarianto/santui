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
    pub filter: String,
    pub filtered: Vec<usize>,
    pub filter_active: bool,
    pub play_state: PlayState,
    pub current_station: Option<usize>,
    pub song_title: String,
    pub track_info: Option<TrackInfo>,
    pub volume: i64,
    pub start_time: Instant,
    pub show_help: bool,
}

impl RadioState {
    pub fn new(stations: Vec<Station>) -> Self {
        let count = stations.len();
        RadioState {
            filtered: (0..count).collect(),
            stations,
            selected: 0,
            filter: String::new(),
            filter_active: false,
            play_state: PlayState::Stopped,
            current_station: None,
            song_title: String::new(),
            track_info: None,
            volume: 75,
            start_time: Instant::now(),
            show_help: false,
        }
    }

    pub fn apply_filter(&mut self) {
        if self.filter.is_empty() {
            self.filtered = (0..self.stations.len()).collect();
        } else {
            let lower = self.filter.to_lowercase();
            self.filtered = self
                .stations
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.name.to_lowercase().contains(&lower)
                })
                .map(|(i, _)| i)
                .collect();
        }
        self.selected = 0;
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
