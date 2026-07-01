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
    Detail(usize),
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
            rename_buf: String::new(),
            last_second: 61,
        }
    }
}

impl WorldTimeState {
    pub fn apply_search(&mut self) {
        self.search_results = timezones::search(&self.search_query);
        self.search_cursor = 0;
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
