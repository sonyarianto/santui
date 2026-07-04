use crate::m3u::Channel;
use std::collections::{BTreeMap, HashSet};

pub enum Screen {
    ChannelList,
    Search,
    PlaylistUrlEditor,
    GroupFilter,
}

pub enum PlaybackState {
    Stopped,
    Buffering { channel_index: usize },
    Playing { channel_index: usize },
    Paused { channel_index: usize },
    Error(String),
}

pub struct IptvState {
    pub channels: Vec<Channel>,
    pub selected: usize,
    pub scroll: usize,
    pub filtered: Vec<usize>,
    pub play_state: PlaybackState,
    pub volume: i64,
    pub screen: Screen,
    pub query: String,
    pub url_edit: String,
    pub url_edit_cursor: usize,
    pub tick_counter: u64,
    pub scan_msg: Option<String>,
    pub scan_ticks: u64,
    pub favorites: HashSet<String>,
    pub show_favorites_only: bool,
    pub playlist_url: String,
    pub group_filter: Option<String>,
}

impl IptvState {
    pub fn new() -> Self {
        IptvState {
            channels: Vec::new(),
            selected: 0,
            scroll: 0,
            filtered: Vec::new(),
            play_state: PlaybackState::Stopped,
            volume: 100,
            screen: Screen::ChannelList,
            query: String::new(),
            url_edit: String::new(),
            url_edit_cursor: 0,
            tick_counter: 0,
            scan_msg: None,
            scan_ticks: 0,
            favorites: HashSet::new(),
            show_favorites_only: false,
            playlist_url: crate::m3u::DEFAULT_PLAYLIST_URL.to_string(),
            group_filter: None,
        }
    }

    pub fn apply_filter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = self
            .channels
            .iter()
            .enumerate()
            .filter(|(_, ch)| {
                let matches_query = q.is_empty()
                    || ch.name.to_lowercase().contains(&q)
                    || ch
                        .group_title
                        .as_ref()
                        .map(|g| g.to_lowercase().contains(&q))
                        .unwrap_or(false)
                    || ch
                        .tvg_id
                        .as_ref()
                        .map(|id| id.to_lowercase().contains(&q))
                        .unwrap_or(false);
                let matches_group = self
                    .group_filter
                    .as_ref()
                    .is_none_or(|gf| ch.group_title.as_ref().map(|g| g == gf).unwrap_or(false));
                let matches_fav = !self.show_favorites_only || self.favorites.contains(&ch.url);
                matches_query && matches_group && matches_fav
            })
            .map(|(i, _)| i)
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.apply_filter();
        self.scroll = 0;
    }

    pub fn group_titles(&self) -> Vec<String> {
        let mut groups: BTreeMap<String, usize> = BTreeMap::new();
        for ch in &self.channels {
            if let Some(ref g) = ch.group_title {
                *groups.entry(g.clone()).or_insert(0) += 1;
            }
        }
        groups.into_keys().collect()
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
            0
        } else {
            self.filtered[self.selected.min(self.filtered.len() - 1)]
        }
    }

    pub fn selected_channel(&self) -> Option<&Channel> {
        if self.filtered.is_empty() {
            return None;
        }
        let idx = self.filtered[self.selected.min(self.filtered.len() - 1)];
        self.channels.get(idx)
    }

    pub fn set_scan_msg(&mut self, msg: String) {
        self.scan_msg = Some(msg);
        self.scan_ticks = 0;
    }

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
        match &self.play_state {
            PlaybackState::Stopped => 3,
            PlaybackState::Buffering { .. } => 3,
            PlaybackState::Playing { .. } => 3,
            PlaybackState::Paused { .. } => 3,
            PlaybackState::Error(_) => 4,
        }
    }

    pub fn volume_up(&mut self) {
        self.volume = (self.volume + 2).min(100);
    }

    pub fn volume_down(&mut self) {
        self.volume = (self.volume - 2).max(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m3u::Channel;
    use std::collections::BTreeMap;

    fn make_channels(n: usize) -> Vec<Channel> {
        (0..n)
            .map(|i| Channel {
                name: format!("Channel {i}"),
                url: format!("http://example.com/{i}"),
                tvg_id: Some(format!("ch{i}")),
                tvg_name: Some(format!("Channel {i}")),
                tvg_logo: None,
                group_title: Some(if i % 3 == 0 {
                    "News".into()
                } else if i % 3 == 1 {
                    "Sports".into()
                } else {
                    "Music".into()
                }),
                attrs: BTreeMap::new(),
            })
            .collect()
    }

    #[test]
    fn new_state_is_stopped() {
        let mut state = IptvState::new();
        state.channels = make_channels(5);
        state.apply_filter();
        assert_eq!(state.filtered.len(), 5);
        assert!(matches!(state.play_state, PlaybackState::Stopped));
    }

    #[test]
    fn filter_by_name() {
        let mut state = IptvState::new();
        state.channels = make_channels(5);
        state.set_query("Channel 3".into());
        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.filtered[0], 3);
    }

    #[test]
    fn filter_by_group() {
        let mut state = IptvState::new();
        state.channels = make_channels(6);
        state.group_filter = Some("News".into());
        state.apply_filter();
        assert_eq!(state.filtered.len(), 2);
        assert_eq!(state.filtered[0], 0);
        assert_eq!(state.filtered[1], 3);
    }

    #[test]
    fn filter_combined() {
        let mut state = IptvState::new();
        state.channels = make_channels(6);
        state.query = "1".into();
        state.group_filter = Some("Sports".into());
        state.apply_filter();
        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.filtered[0], 1);
    }

    #[test]
    fn filter_favorites_only() {
        let mut state = IptvState::new();
        state.channels = make_channels(5);
        state.favorites.insert("http://example.com/0".into());
        state.favorites.insert("http://example.com/2".into());
        state.show_favorites_only = true;
        state.apply_filter();
        assert_eq!(state.filtered.len(), 2);
    }

    #[test]
    fn group_titles_returns_sorted_unique() {
        let mut state = IptvState::new();
        state.channels = make_channels(6);
        let groups = state.group_titles();
        assert_eq!(groups, vec!["Music", "News", "Sports"]);
    }

    #[test]
    fn toggle_favorite() {
        let mut state = IptvState::new();
        assert!(state.toggle_favorite("http://example.com/1"));
        assert!(state.is_favorite("http://example.com/1"));
        assert!(!state.toggle_favorite("http://example.com/1"));
        assert!(!state.is_favorite("http://example.com/1"));
    }

    #[test]
    fn select_next_wraps_at_end() {
        let mut state = IptvState::new();
        state.channels = make_channels(3);
        state.apply_filter();
        state.selected = 2;
        state.select_next();
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn select_prev_stops_at_zero() {
        let mut state = IptvState::new();
        state.channels = make_channels(3);
        state.apply_filter();
        state.select_prev();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_page_down_clamps() {
        let mut state = IptvState::new();
        state.channels = make_channels(20);
        state.apply_filter();
        state.selected = 18;
        state.select_page_down(5);
        assert_eq!(state.selected, 19);
    }

    #[test]
    fn select_page_up_stops_at_zero() {
        let mut state = IptvState::new();
        state.channels = make_channels(20);
        state.apply_filter();
        state.selected = 2;
        state.select_page_up(5);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn ensure_scroll_visible() {
        let mut state = IptvState::new();
        state.channels = make_channels(20);
        state.apply_filter();
        state.selected = 15;
        state.scroll = 0;
        state.ensure_scroll_visible(5);
        assert_eq!(state.scroll, 11);
    }

    #[test]
    fn volume_up_down_clamps() {
        let mut state = IptvState::new();
        state.volume = 100;
        state.volume_up();
        assert_eq!(state.volume, 100);
        state.volume = 0;
        state.volume_down();
        assert_eq!(state.volume, 0);
    }
}
