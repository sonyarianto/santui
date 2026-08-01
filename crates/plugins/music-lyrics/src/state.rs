use crate::lrclib::LRCLibTrack;

#[derive(Debug, Clone, PartialEq)]
pub enum FetchState {
    Idle,
    Fetching,
    Done,
    Error(String),
}

pub struct LyricsState {
    pub query: String,
    pub results: Vec<LRCLibTrack>,
    pub selected: usize,
    pub scroll: usize,
    pub fetch_state: FetchState,
    pub dirty: bool,
    pub tick_counter: u64,
    pub search_mode: bool,
    pub show_lyrics: bool,
    pub lyrics_text: String,
    pub lyrics_source: String,
    pub lyrics_scroll: usize,
    pub lyrics_title: String,
    pub lyrics_artist: String,
    pub lyrics_loading: bool,
}

impl Default for LyricsState {
    fn default() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            scroll: 0,
            fetch_state: FetchState::Idle,
            dirty: true,
            tick_counter: 0,
            search_mode: false,
            show_lyrics: false,
            lyrics_text: String::new(),
            lyrics_source: String::new(),
            lyrics_scroll: 0,
            lyrics_title: String::new(),
            lyrics_artist: String::new(),
            lyrics_loading: false,
        }
    }
}

#[cfg(test)]
pub(crate) fn make_track(id: u64, name: &str) -> LRCLibTrack {
    LRCLibTrack {
        id,
        track_name: name.into(),
        artist_name: "Artist".into(),
        album_name: "Album".into(),
        duration: 200.0,
        instrumental: false,
        plain_lyrics: None,
        synced_lyrics: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lrclib::LRCLibTrack;

    #[test]
    fn default_state_empty() {
        let state = LyricsState::default();
        assert!(state.query.is_empty());
        assert!(state.results.is_empty());
        assert_eq!(state.selected, 0);
        assert_eq!(state.fetch_state, FetchState::Idle);
        assert!(!state.show_lyrics);
        assert!(state.lyrics_text.is_empty());
    }

    #[test]
    fn select_prev_at_top_stays() {
        let mut state = LyricsState::default();
        state.results = vec![make_track(1, "A"), make_track(2, "B")];
        state.selected = 1;
        state.selected = state.selected.saturating_sub(1);
        assert_eq!(state.selected, 0);
        state.selected = state.selected.saturating_sub(1);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_next_at_end_stays() {
        let mut state = LyricsState::default();
        state.results = vec![make_track(1, "A"), make_track(2, "B")];
        state.selected = 0;
        state.selected = state
            .selected
            .min(state.results.len().saturating_sub(1))
            .saturating_add(1)
            .min(state.results.len().saturating_sub(1));
        assert_eq!(state.selected, 1);
        state.selected = state
            .selected
            .min(state.results.len().saturating_sub(1))
            .saturating_add(1)
            .min(state.results.len().saturating_sub(1));
        assert_eq!(state.selected, 1);
    }
}
