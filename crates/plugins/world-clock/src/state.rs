use chrono::Offset;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::timezones;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockEntry {
    pub tz: Tz,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Grid,
    Search,
    Rename(usize),
}

pub struct WorldTimeState {
    pub clocks: Vec<ClockEntry>,
    pub selected: usize,
    pub screen: Screen,
    pub search_query: String,
    pub search_results: Vec<Tz>,
    pub search_cursor: usize,
    pub search_scroll: usize,
    pub search_cursor_visible: bool,
    pub rename_buf: String,
    pub last_second: u32,
}

impl Default for WorldTimeState {
    fn default() -> Self {
        Self {
            clocks: Vec::new(),
            selected: 0,
            screen: Screen::Grid,
            search_query: String::new(),
            search_results: Vec::new(),
            search_cursor: 0,
            search_scroll: 0,
            search_cursor_visible: true,
            rename_buf: String::new(),
            last_second: 61,
        }
    }
}

impl WorldTimeState {
    pub fn apply_search(&mut self) {
        self.search_results = timezones::search(&self.search_query);
        self.search_cursor = 0;
        self.search_scroll = 0;
    }

    pub fn add_clock(&mut self, tz: Tz) {
        if self.clocks.iter().any(|c| c.tz == tz) {
            return;
        }
        let label = timezones::city_name(tz);
        self.clocks.push(ClockEntry { tz, label });
    }

    pub fn remove_selected(&mut self) {
        if self.selected < self.clocks.len() {
            self.clocks.remove(self.selected);
        }
        if !self.clocks.is_empty() {
            self.selected = self.selected.min(self.clocks.len() - 1);
        } else {
            self.selected = 0;
        }
    }

    pub fn serialize_clocks(&self) -> String {
        serde_json::to_string(&self.clocks).unwrap_or_default()
    }

    pub fn load_clocks(&mut self, json: &str) {
        if let Ok(clocks) = serde_json::from_str::<Vec<ClockEntry>>(json) {
            self.clocks = clocks;
        }
    }

    pub fn default_clocks() -> Vec<ClockEntry> {
        let mut entries = Vec::new();
        entries.push(ClockEntry {
            tz: Tz::UTC,
            label: "UTC".into(),
        });
        let local_offset = chrono::Local::now().offset().fix().local_minus_utc();
        let utc_now = chrono::Utc::now();
        for &(_name, tz) in timezones::ALL {
            let tz_dt = utc_now.with_timezone(&tz);
            let tz_offset = tz_dt.offset().fix().local_minus_utc();
            if tz_offset == local_offset {
                entries.push(ClockEntry {
                    tz,
                    label: timezones::city_name(tz),
                });
                break;
            }
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_clock_deduplicates() {
        let mut s = WorldTimeState::default();
        s.add_clock(chrono_tz::Tz::Asia__Tokyo);
        s.add_clock(chrono_tz::Tz::Asia__Tokyo);
        assert_eq!(s.clocks.len(), 1);
    }

    #[test]
    fn add_clock_allows_different_timezones() {
        let mut s = WorldTimeState::default();
        s.add_clock(chrono_tz::Tz::Asia__Tokyo);
        s.add_clock(chrono_tz::Tz::Europe__London);
        assert_eq!(s.clocks.len(), 2);
    }

    #[test]
    fn remove_selected_removes_correct_entry() {
        let mut s = WorldTimeState::default();
        s.add_clock(chrono_tz::Tz::Asia__Tokyo);
        s.add_clock(chrono_tz::Tz::Europe__London);
        s.selected = 0;
        s.remove_selected();
        assert_eq!(s.clocks.len(), 1);
        assert_eq!(s.clocks[0].tz, chrono_tz::Tz::Europe__London);
    }

    #[test]
    fn remove_selected_clamps_cursor() {
        let mut s = WorldTimeState::default();
        s.add_clock(chrono_tz::Tz::Asia__Tokyo);
        s.selected = 0;
        s.remove_selected();
        assert_eq!(s.selected, 0);
        assert!(s.clocks.is_empty());
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let mut s = WorldTimeState::default();
        s.add_clock(chrono_tz::Tz::Asia__Tokyo);
        s.add_clock(chrono_tz::Tz::America__New_York);
        let json = s.serialize_clocks();
        let mut s2 = WorldTimeState::default();
        s2.load_clocks(&json);
        assert_eq!(s2.clocks.len(), 2);
        assert_eq!(s2.clocks[0].tz, chrono_tz::Tz::Asia__Tokyo);
    }

    #[test]
    fn load_clocks_invalid_json_does_not_panic() {
        let mut s = WorldTimeState::default();
        s.load_clocks("not valid json");
        assert!(s.clocks.is_empty());
    }

    #[test]
    fn default_clocks_not_empty() {
        let clocks = WorldTimeState::default_clocks();
        assert!(!clocks.is_empty());
        assert_eq!(clocks[0].tz, chrono_tz::Tz::UTC);
    }
}
