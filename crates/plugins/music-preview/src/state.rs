use crate::api::ItunesTrack;

#[derive(Debug, Clone, PartialEq)]
pub enum FetchState {
    Idle,
    Fetching,
    Done,
    Error(String),
}

pub struct MusicState {
    pub query: String,
    pub results: Vec<ItunesTrack>,
    pub selected: usize,
    pub scroll: usize,
    pub fetch_state: FetchState,
    pub dirty: bool,
}

impl Default for MusicState {
    fn default() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            scroll: 0,
            fetch_state: FetchState::Idle,
            dirty: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ItunesTrack;

    fn make_track(id: u32, name: &str) -> ItunesTrack {
        ItunesTrack {
            track_id: id,
            track_name: name.into(),
            artist_name: "Artist".into(),
            collection_name: "Album".into(),
            artwork_url_100: String::new(),
            preview_url: String::new(),
            track_time_millis: Some(30000),
            primary_genre_name: "Rock".into(),
        }
    }

    #[test]
    fn default_state_empty() {
        let state = MusicState::default();
        assert!(state.query.is_empty());
        assert!(state.results.is_empty());
        assert_eq!(state.selected, 0);
        assert_eq!(state.scroll, 0);
        assert_eq!(state.fetch_state, FetchState::Idle);
        assert!(state.dirty);
    }

    #[test]
    fn select_prev_at_top_stays() {
        let mut state = MusicState::default();
        state.results = vec![make_track(1, "A"), make_track(2, "B")];
        state.selected = 1;
        state.selected = state.selected.saturating_sub(1);
        assert_eq!(state.selected, 0);
        // saturating_sub again at top should stay
        state.selected = state.selected.saturating_sub(1);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_next_at_end_stays() {
        let mut state = MusicState::default();
        state.results = vec![make_track(1, "A"), make_track(2, "B")];
        state.selected = 0;
        state.selected = state
            .selected
            .min(state.results.len().saturating_sub(1))
            .saturating_add(1)
            .min(state.results.len().saturating_sub(1));
        assert_eq!(state.selected, 1);
        // increment again at end should stay
        state.selected = state
            .selected
            .min(state.results.len().saturating_sub(1))
            .saturating_add(1)
            .min(state.results.len().saturating_sub(1));
        assert_eq!(state.selected, 1);
    }
}
