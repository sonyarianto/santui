use crate::itunes::TrackInfo;
use crate::lrclib;
use crate::stations::Station;
use std::collections::HashSet;
use std::time::Instant;

pub const MAX_RETRIES: u32 = 10;

pub fn retry_delay_ms(attempt: u32) -> u64 {
    match attempt {
        0 => 1000,
        1 => 2000,
        2 => 4000,
        3 => 8000,
        _ => 16000,
    }
}

pub fn wrap_text(text: &str, max_w: usize) -> Vec<String> {
    let mut result = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            result.push(String::new());
            continue;
        }
        if max_w == 0 || line.chars().count() <= max_w {
            result.push(line.to_string());
            continue;
        }
        let mut current = String::new();
        let mut current_len = 0usize;
        for word in line.split_whitespace() {
            let word_len = word.chars().count();
            if current_len == 0 {
                if word_len > max_w {
                    for chunk in word.chars().collect::<Vec<char>>().chunks(max_w) {
                        result.push(chunk.iter().collect());
                    }
                } else {
                    current = word.to_string();
                    current_len = word_len;
                }
            } else if current_len + 1 + word_len <= max_w {
                current.push(' ');
                current.push_str(word);
                current_len += 1 + word_len;
            } else {
                result.push(current);
                current = word.to_string();
                current_len = word_len;
            }
        }
        if !current.is_empty() {
            result.push(current);
        }
    }
    result
}

#[derive(Debug)]
pub enum PlayState {
    Stopped,
    Connecting(String),
    Playing(String),
    Retrying(String),
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
    pub last_metadata: String,
    pub track_info: Option<TrackInfo>,
    pub volume: i64,
    pub start_time: Instant,
    pub retry_deadline: Option<Instant>,
    pub retry_attempt: u32,
    pub retry_mode: bool,
    pub scan_msg: Option<String>,
    pub scan_ticks: u64,
    pub query: String,
    pub search_mode: bool,
    pub tick_counter: u64,
    pub show_lyrics: bool,
    pub lyrics_focused: bool,
    pub lyrics_text: String,
    pub lyrics_loading: bool,
    pub lyrics_scroll: usize,
    pub lyrics_source: String,
    pub metadata_seq: u64,
    pub favorites: HashSet<String>,
    pub show_favorites_only: bool,
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
            last_metadata: String::new(),
            track_info: None,
            volume: 100,
            start_time: Instant::now(),
            retry_deadline: None,
            retry_attempt: 0,
            retry_mode: false,
            scan_msg: None,
            scan_ticks: 0,
            query: String::new(),
            search_mode: false,
            tick_counter: 0,
            show_lyrics: false,
            lyrics_focused: false,
            lyrics_text: String::new(),
            lyrics_loading: false,
            lyrics_scroll: 0,
            lyrics_source: String::new(),
            metadata_seq: 0,
            favorites: HashSet::new(),
            show_favorites_only: false,
        }
    }

    pub fn apply_filter(&mut self) {
        let q = self.query.to_lowercase();
        if q.is_empty() && !self.show_favorites_only {
            self.filtered = (0..self.stations.len()).collect();
        } else {
            self.filtered = self
                .stations
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    let matches_query = q.is_empty()
                        || s.name.to_lowercase().contains(&q)
                        || s.country.to_lowercase().contains(&q)
                        || s.country_name().to_lowercase().contains(&q)
                        || s.genre.to_lowercase().contains(&q);
                    let matches_fav = !self.show_favorites_only || self.favorites.contains(&s.url);
                    matches_query && matches_fav
                })
                .map(|(i, _)| i)
                .collect();
        }
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.apply_filter();
        self.scroll = 0;
    }

    pub fn is_favorite(&self, url: &str) -> bool {
        self.favorites.contains(url)
    }

    pub fn toggle_favorite(&mut self, url: &str) -> bool {
        if self.favorites.contains(url) {
            self.favorites.remove(url);
            false
        } else {
            self.favorites.insert(url.to_string());
            true
        }
    }

    pub fn set_favorites(&mut self, favs: HashSet<String>) {
        self.favorites = favs;
        self.apply_filter();
    }

    pub fn favorites_count(&self) -> usize {
        self.favorites.len()
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

    pub fn set_scan_msg(&mut self, msg: String) {
        self.scan_msg = Some(msg);
        self.scan_ticks = 0;
    }

    /// Returns `true` if the message was just auto-dismissed this tick.
    pub fn tick_scan_msg(&mut self) -> bool {
        if self.scan_msg.is_some() {
            self.scan_ticks += 1;
            if self.scan_ticks > 20 {
                self.scan_msg = None;
                return true;
            }
        }
        false
    }

    pub fn info_h(&self) -> u16 {
        let content_rows = match &self.play_state {
            PlayState::Stopped => 1,
            PlayState::Connecting(_) => 2,
            PlayState::Playing(_) => {
                if self.song_title.is_empty() {
                    2
                } else {
                    match &self.track_info {
                        Some(info) if info.artist.is_some() => 3,
                        _ => 2,
                    }
                }
            }
            PlayState::Retrying(_) | PlayState::Error(_) => 2,
        };
        content_rows + 2
    }

    pub fn volume_up(&mut self) {
        self.volume = (self.volume + 2).min(100);
    }

    pub fn volume_down(&mut self) {
        self.volume = (self.volume - 2).max(0);
    }

    pub fn clear_lyrics(&mut self) {
        self.lyrics_text.clear();
        self.lyrics_loading = false;
        self.lyrics_scroll = 0;
        self.lyrics_source.clear();
    }

    pub fn clear_retry(&mut self) {
        self.retry_attempt = 0;
        self.retry_deadline = None;
    }

    /// Inner width of the lyrics panel for text wrapping purposes.
    pub fn lyrics_inner_w(&self, area_w: u16) -> u16 {
        let left_w = if self.show_lyrics {
            (area_w * 3 / 5).max(20)
        } else {
            area_w
        };
        let right_w = area_w.saturating_sub(left_w);
        right_w.saturating_sub(4)
    }

    /// Number of visible lines in the lyrics content area (inside panel borders,
    /// below header, above footer).
    pub fn lyrics_content_height(&self, area_h: u16) -> usize {
        let footer_rows = if self.show_lyrics { 2 } else { 0 };
        let hdr_rows = if !self.lyrics_text.is_empty() {
            let has_title = if let Some(ref info) = self.track_info {
                info.title.is_some() || !self.song_title.is_empty()
            } else {
                !self.song_title.is_empty()
            };
            let has_artist = if let Some(ref info) = self.track_info {
                info.artist.is_some()
            } else if !self.song_title.is_empty() {
                lrclib::split_title(&self.song_title).0.is_some()
            } else {
                false
            };
            match (has_title, has_artist) {
                (true, true) => 3,
                (true, false) => 2,
                (false, true) => 2,
                (false, false) => 0,
            }
        } else {
            0
        };
        area_h.saturating_sub(2 + footer_rows + hdr_rows) as usize
    }

    pub fn lyrics_scroll_up(&mut self) {
        self.lyrics_scroll = self.lyrics_scroll.saturating_sub(1);
    }

    pub fn lyrics_scroll_down(&mut self, panel_h: usize, inner_w: u16) {
        let total = wrap_text(&self.lyrics_text, inner_w as usize).len();
        if total > panel_h {
            let max_scroll = total.saturating_sub(panel_h);
            self.lyrics_scroll = (self.lyrics_scroll + 1).min(max_scroll);
        }
    }

    pub fn lyrics_page_up(&mut self, page: usize) {
        self.lyrics_scroll = self.lyrics_scroll.saturating_sub(page);
    }

    pub fn lyrics_page_down(&mut self, page: usize, inner_w: u16) {
        let total = wrap_text(&self.lyrics_text, inner_w as usize).len();
        if total > page {
            let max_scroll = total.saturating_sub(page);
            self.lyrics_scroll = (self.lyrics_scroll + page).min(max_scroll);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stations::Station;

    fn make_stations(n: usize) -> Vec<Station> {
        (0..n)
            .map(|i| Station {
                name: format!("Station {i}"),
                url: format!("http://example.com/{i}"),
                country: if i % 2 == 0 { "US".into() } else { "GB".into() },
                genre: "Rock".into(),
            })
            .collect()
    }

    #[test]
    fn new_initializes_all_stations() {
        let s = make_stations(5);
        let state = RadioState::new(s);
        assert_eq!(state.filtered.len(), 5);
        assert_eq!(state.selected, 0);
        assert_eq!(state.volume, 100);
    }

    #[test]
    fn apply_filter_empty_returns_all() {
        let s = make_stations(5);
        let mut state = RadioState::new(s);
        state.set_query(String::new());
        assert_eq!(state.filtered.len(), 5);
    }

    #[test]
    fn apply_filter_matches_name() {
        let s = make_stations(5);
        let mut state = RadioState::new(s);
        state.set_query("Station 3".into());
        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.filtered[0], 3);
    }

    #[test]
    fn apply_filter_matches_country() {
        let s = make_stations(5);
        let mut state = RadioState::new(s);
        state.set_query("GB".into());
        assert_eq!(state.filtered.len(), 2);
        assert!(state.filtered.iter().all(|&i| i % 2 == 1));
    }

    #[test]
    fn apply_filter_no_match() {
        let s = make_stations(5);
        let mut state = RadioState::new(s);
        state.set_query("NONEXISTENT".into());
        assert!(state.filtered.is_empty());
    }

    #[test]
    fn apply_filter_case_insensitive() {
        let s = make_stations(5);
        let mut state = RadioState::new(s);
        state.set_query("station".into());
        assert_eq!(state.filtered.len(), 5);
    }

    #[test]
    fn select_next_wraps_at_end() {
        let s = make_stations(3);
        let mut state = RadioState::new(s);
        state.selected = 2;
        state.select_next();
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn select_next_normal() {
        let s = make_stations(3);
        let mut state = RadioState::new(s);
        state.select_next();
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn select_prev_stops_at_zero() {
        let s = make_stations(3);
        let mut state = RadioState::new(s);
        state.select_prev();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_prev_normal() {
        let s = make_stations(3);
        let mut state = RadioState::new(s);
        state.selected = 2;
        state.select_prev();
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn select_page_down() {
        let s = make_stations(20);
        let mut state = RadioState::new(s);
        state.select_page_down(5);
        assert_eq!(state.selected, 5);
    }

    #[test]
    fn select_page_down_clamps() {
        let s = make_stations(20);
        let mut state = RadioState::new(s);
        state.selected = 18;
        state.select_page_down(5);
        assert_eq!(state.selected, 19);
    }

    #[test]
    fn select_page_up() {
        let s = make_stations(20);
        let mut state = RadioState::new(s);
        state.selected = 10;
        state.select_page_up(3);
        assert_eq!(state.selected, 7);
    }

    #[test]
    fn select_page_up_stops_at_zero() {
        let s = make_stations(20);
        let mut state = RadioState::new(s);
        state.selected = 2;
        state.select_page_up(5);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn ensure_scroll_visible_shifts_up() {
        let mut state = RadioState::new(make_stations(20));
        state.selected = 15;
        state.scroll = 0;
        state.ensure_scroll_visible(5);
        assert_eq!(state.scroll, 11);
    }

    #[test]
    fn ensure_scroll_visible_shifts_down() {
        let mut state = RadioState::new(make_stations(20));
        state.selected = 2;
        state.scroll = 10;
        state.ensure_scroll_visible(5);
        assert_eq!(state.scroll, 2);
    }

    #[test]
    fn selected_station_returns_correct() {
        let s = make_stations(5);
        let state = RadioState::new(s);
        let station = state.selected_station();
        assert!(station.is_some());
        assert_eq!(station.unwrap().name, "Station 0");
    }

    #[test]
    fn selected_station_empty_filtered() {
        let mut state = RadioState::new(make_stations(3));
        state.filtered = vec![];
        assert!(state.selected_station().is_none());
    }

    #[test]
    fn current_filtered_index_empty() {
        let mut state = RadioState::new(make_stations(3));
        state.filtered = vec![];
        assert_eq!(state.current_filtered_index(), 0);
    }

    #[test]
    fn volume_up_increases() {
        let mut state = RadioState::new(make_stations(1));
        state.volume = 50;
        state.volume_up();
        assert_eq!(state.volume, 52);
    }

    #[test]
    fn volume_up_clamps() {
        let mut state = RadioState::new(make_stations(1));
        state.volume = 100;
        state.volume_up();
        assert_eq!(state.volume, 100);
    }

    #[test]
    fn volume_down_decreases() {
        let mut state = RadioState::new(make_stations(1));
        state.volume = 50;
        state.volume_down();
        assert_eq!(state.volume, 48);
    }

    #[test]
    fn volume_down_clamps() {
        let mut state = RadioState::new(make_stations(1));
        state.volume = 1;
        state.volume_down();
        assert_eq!(state.volume, 0);
    }

    #[test]
    fn apply_filter_resets_selected_when_filtered_shrinks() {
        let s = make_stations(5);
        let mut state = RadioState::new(s);
        state.selected = 4;
        state.set_query("Station 0".into());
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn ensure_scroll_visible_min_visible() {
        let mut state = RadioState::new(make_stations(5));
        state.selected = 0;
        state.scroll = 0;
        state.ensure_scroll_visible(0);
        assert_eq!(state.scroll, 0);
    }
}
