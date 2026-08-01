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
    pub tick_counter: u64,
    pub search_mode: bool,
    pub now_playing: Option<usize>,
    pub track_elapsed: Option<u64>,
    pub show_details: bool,
    pub details_focused: bool,
    pub details_scroll: usize,
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
            tick_counter: 0,
            search_mode: false,
            now_playing: None,
            track_elapsed: None,
            show_details: false,
            details_focused: false,
            details_scroll: 0,
        }
    }
}

impl MusicState {
    pub fn selected_track(&self) -> Option<&ItunesTrack> {
        self.results.get(self.selected)
    }

    pub fn toggle_details(&mut self) {
        self.show_details = !self.show_details;
        self.details_scroll = 0;
        self.details_focused = self.show_details;
    }

    pub fn close_details(&mut self) {
        self.show_details = false;
        self.details_scroll = 0;
        self.details_focused = false;
    }

    fn details_elapsed(&self) -> Option<u64> {
        if self.now_playing == Some(self.selected) {
            self.track_elapsed
        } else {
            None
        }
    }

    pub fn details_scroll_up(&mut self) {
        self.details_scroll = self.details_scroll.saturating_sub(1);
    }

    pub fn details_scroll_down(&mut self, panel_h: usize, inner_w: u16) {
        let Some(track) = self.selected_track() else {
            return;
        };
        let elapsed = self.details_elapsed();
        let total = detail_lines(track, elapsed, inner_w as usize).len();
        if total > panel_h {
            let max_scroll = total.saturating_sub(panel_h);
            self.details_scroll = (self.details_scroll + 1).min(max_scroll);
        }
    }

    pub fn details_page_up(&mut self, page: usize) {
        self.details_scroll = self.details_scroll.saturating_sub(page);
    }

    pub fn details_page_down(&mut self, page: usize, inner_w: u16) {
        let Some(track) = self.selected_track() else {
            return;
        };
        let elapsed = self.details_elapsed();
        let total = detail_lines(track, elapsed, inner_w as usize).len();
        if total > page {
            let max_scroll = total.saturating_sub(page);
            self.details_scroll = (self.details_scroll + page).min(max_scroll);
        }
    }
}

fn currency_symbol(code: &str) -> Option<&'static str> {
    match code {
        "USD" | "AUD" | "CAD" | "NZD" | "HKD" | "SGD" => Some("$"),
        "EUR" => Some("€"),
        "GBP" => Some("£"),
        "JPY" | "CNY" => Some("¥"),
        "IDR" => Some("Rp"),
        "KRW" => Some("₩"),
        _ => None,
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Format an ISO date (e.g. `2002-10-29T08:00:00Z`) as `October 29, 2002`.
/// Falls back to the raw date part when unparseable.
fn fmt_release_date(iso: &str) -> String {
    let date = iso.split('T').next().unwrap_or(iso);
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[0].parse::<u32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        ) {
            if (1..=12).contains(&m) && (1..=31).contains(&d) {
                return format!("{} {}, {}", MONTHS[(m - 1) as usize], d, y);
            }
        }
    }
    date.to_string()
}

pub fn fmt_duration(secs: u64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}

/// Build the wrapped lines shown in the Track Details panel.
/// Single source of truth for rendering and scroll clamping.
pub fn detail_lines(track: &ItunesTrack, elapsed: Option<u64>, inner_w: usize) -> Vec<String> {
    use santui_ipc::text::word_wrap;
    let mut out: Vec<String> = Vec::new();

    if let Some(secs) = elapsed {
        let total = track
            .track_time_millis
            .map(|ms| (ms / 1000) as u64)
            .unwrap_or(secs);
        out.push(format!(
            "▶ Playing {} / {}",
            fmt_duration(secs),
            fmt_duration(total)
        ));
    }

    out.push(format!("Title: {}", track.track_name));
    if !track.artist_name.is_empty() {
        out.push(format!("Artist: {}", track.artist_name));
    }
    if !track.collection_name.is_empty() {
        out.push(format!("Album: {}", track.collection_name));
    }
    if !track.primary_genre_name.is_empty() {
        out.push(format!("Genre: {}", track.primary_genre_name));
    }
    if !track.release_date.is_empty() {
        out.push(format!(
            "Released: {}",
            fmt_release_date(&track.release_date)
        ));
    }
    if track.disc_number.is_some() || track.track_number.is_some() {
        let disc = track
            .disc_number
            .map(|d| format!("{d}"))
            .unwrap_or_else(|| "?".into());
        let num = track
            .track_number
            .map(|n| format!("{n}"))
            .unwrap_or_else(|| "?".into());
        out.push(format!("Position: Disc {disc} · Track {num}"));
    }
    if let Some(ms) = track.track_time_millis {
        out.push(format!("Duration: {}", fmt_duration((ms / 1000) as u64)));
    }
    if !track.country.is_empty() {
        out.push(format!("Country: {}", track.country));
    }
    if let Some(price) = track.track_price {
        let mut line = format!("Price: {:.2}", price);
        if let Some(sym) = currency_symbol(&track.currency) {
            line = format!("Price: {sym}{:.2}", price);
        } else if !track.currency.is_empty() {
            line.push(' ');
            line.push_str(&track.currency);
        }
        if let Some(cp) = track.collection_price {
            if let Some(sym) = currency_symbol(&track.currency) {
                line.push_str(&format!("  (album: {sym}{cp:.2})"));
            } else {
                line.push_str(&format!("  (album: {cp:.2})"));
            }
        }
        out.push(line);
    }
    if !track.track_explicitness.is_empty() {
        out.push(format!("Explicitness: {}", track.track_explicitness));
    }

    let mut wrapped: Vec<String> = Vec::new();
    for line in out {
        wrapped.extend(word_wrap(&line, inner_w.max(1)));
    }
    wrapped
}
#[cfg(test)]
pub(crate) fn make_track(id: u64, name: &str, url: &str) -> ItunesTrack {
    ItunesTrack {
        track_id: id,
        track_name: name.into(),
        artist_name: "Artist".into(),
        collection_name: "Album".into(),
        artwork_url_100: String::new(),
        preview_url: url.into(),
        track_time_millis: Some(30000),
        primary_genre_name: "Rock".into(),
        release_date: String::new(),
        track_number: None,
        disc_number: None,
        country: String::new(),
        currency: String::new(),
        track_price: None,
        collection_price: None,
        track_explicitness: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        state.results = vec![make_track(1, "A", ""), make_track(2, "B", "")];
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
        state.results = vec![make_track(1, "A", ""), make_track(2, "B", "")];
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

    fn rich_track() -> ItunesTrack {
        let mut t = make_track(1, "Lose Yourself", "");
        t.artist_name = "Eminem".into();
        t.collection_name = "8 Mile Soundtrack".into();
        t.primary_genre_name = "Hip-Hop/Rap".into();
        t.release_date = "2002-10-29T08:00:00Z".into();
        t.track_number = Some(1);
        t.disc_number = Some(1);
        t.track_time_millis = Some(326000);
        t.country = "USA".into();
        t.currency = "USD".into();
        t.track_price = Some(1.29);
        t.collection_price = Some(9.99);
        t.track_explicitness = "notExplicit".into();
        t
    }

    #[test]
    fn detail_lines_include_available_fields() {
        let lines = detail_lines(&rich_track(), None, 60);
        assert_eq!(lines[0], "Title: Lose Yourself");
        assert_eq!(lines[1], "Artist: Eminem");
        assert_eq!(lines[2], "Album: 8 Mile Soundtrack");
        assert_eq!(lines[3], "Genre: Hip-Hop/Rap");
        assert_eq!(lines[4], "Released: October 29, 2002");
        assert_eq!(lines[5], "Position: Disc 1 · Track 1");
        assert_eq!(lines[6], "Duration: 5:26");
        assert_eq!(lines[7], "Country: USA");
        assert_eq!(lines[8], "Price: $1.29 (album: $9.99)");
        assert_eq!(lines[9], "Explicitness: notExplicit");
    }

    #[test]
    fn detail_lines_playing_prefix_shows_progress() {
        let lines = detail_lines(&rich_track(), Some(65), 60);
        assert_eq!(lines[0], "▶ Playing 1:05 / 5:26");
        assert_eq!(lines[1], "Title: Lose Yourself");
    }

    #[test]
    fn detail_lines_omit_missing_fields() {
        let t = ItunesTrack {
            track_id: 1,
            track_name: "Plain".into(),
            artist_name: String::new(),
            collection_name: String::new(),
            artwork_url_100: String::new(),
            preview_url: String::new(),
            track_time_millis: None,
            primary_genre_name: String::new(),
            release_date: String::new(),
            track_number: None,
            disc_number: None,
            country: String::new(),
            currency: String::new(),
            track_price: None,
            collection_price: None,
            track_explicitness: String::new(),
        };
        let lines = detail_lines(&t, None, 60);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Title: Plain");
    }

    #[test]
    fn detail_lines_wrap_long_lines() {
        let t = rich_track();
        let lines = detail_lines(&t, None, 8);
        assert!(lines.len() > 10, "long lines should wrap");
        assert!(lines.iter().any(|l| l.contains("Price:")));
    }

    #[test]
    fn toggle_details_toggles_focus_and_resets_scroll() {
        let mut state = MusicState {
            results: vec![make_track(1, "A", "")],
            details_scroll: 5,
            ..MusicState::default()
        };
        state.toggle_details();
        assert!(state.show_details);
        assert!(state.details_focused);
        assert_eq!(state.details_scroll, 0);
        state.toggle_details();
        assert!(!state.show_details);
        assert!(!state.details_focused);
    }

    #[test]
    fn close_details_resets_state() {
        let mut state = MusicState {
            show_details: true,
            details_focused: true,
            details_scroll: 3,
            ..MusicState::default()
        };
        state.close_details();
        assert!(!state.show_details);
        assert!(!state.details_focused);
        assert_eq!(state.details_scroll, 0);
    }

    #[test]
    fn fmt_release_date_formats_iso() {
        assert_eq!(fmt_release_date("1999-09-27"), "September 27, 1999");
        assert_eq!(fmt_release_date("2002-10-29T08:00:00Z"), "October 29, 2002");
        assert_eq!(fmt_release_date("2020-01-05"), "January 5, 2020");
        assert_eq!(fmt_release_date("2020-12-31"), "December 31, 2020");
    }

    #[test]
    fn fmt_release_date_falls_back_on_garbage() {
        assert_eq!(fmt_release_date("unknown"), "unknown");
        assert_eq!(fmt_release_date("2002-13-40"), "2002-13-40");
        assert_eq!(fmt_release_date(""), "");
    }

    #[test]
    fn details_scroll_clamped_to_content() {
        let mut state = MusicState {
            results: vec![rich_track()],
            show_details: true,
            details_focused: true,
            ..MusicState::default()
        };
        let max_scroll = detail_lines(&rich_track(), None, 60).len() - 3;
        for _ in 0..max_scroll + 2 {
            state.details_scroll_down(3, 60);
        }
        assert_eq!(state.details_scroll, max_scroll);
        state.details_scroll_down(3, 60);
        assert_eq!(state.details_scroll, max_scroll);
        state.details_scroll_up();
        assert_eq!(state.details_scroll, max_scroll - 1);
    }
}
