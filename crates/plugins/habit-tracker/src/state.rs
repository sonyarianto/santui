use std::collections::HashMap;

use chrono::{Datelike, Days, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Habit {
    pub id: String,
    pub name: String,
    pub description: String,
    pub color: String,
    pub created_at: String,
    pub archived: bool,
}

impl Default for Habit {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            color: "green".into(),
            created_at: Local::now().format("%Y-%m-%d").to_string(),
            archived: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HabitEntry {
    pub date: String,
    pub habit_id: String,
    pub completed: bool,
    pub note: String,
}

impl Default for HabitEntry {
    fn default() -> Self {
        Self {
            date: Local::now().format("%Y-%m-%d").to_string(),
            habit_id: String::new(),
            completed: false,
            note: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HabitData {
    #[serde(default)]
    pub habits: Vec<Habit>,
    #[serde(default)]
    pub entries: Vec<HabitEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Overview,
    Detail,
    Editor,
    DayDetail,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FocusField {
    Name,
    Description,
    Color,
}

pub const COLOR_PRESETS: &[&str] = &[
    "green", "blue", "orange", "purple", "red", "yellow", "cyan", "pink",
];

pub struct HabitState {
    pub data: HabitData,
    pub screen: Screen,
    pub cursor: usize,
    pub detail_habit_idx: usize,
    pub heatmap_row: usize,
    pub heatmap_col: usize,
    pub heatmap_scroll: usize,
    pub editor_habit: Option<Habit>,
    pub editor_focus: FocusField,
    pub editor_buffer: String,
    pub editing: bool,
    pub day_detail_date: String,
    pub day_detail_scroll: usize,
    pub sorted_habit_ids: Vec<String>,
    pub filter_query: String,
    pub filter_mode: bool,
    pub dirty: bool,
    pub note_editing: bool,
    pub note_buffer: String,
    pub note_habit_idx: usize,
}

impl Default for HabitState {
    fn default() -> Self {
        Self {
            data: HabitData::default(),
            screen: Screen::Overview,
            cursor: 0,
            detail_habit_idx: 0,
            heatmap_row: 0,
            heatmap_col: 0,
            heatmap_scroll: 0,
            editor_habit: None,
            editor_focus: FocusField::Name,
            editor_buffer: String::new(),
            editing: false,
            day_detail_date: Local::now().format("%Y-%m-%d").to_string(),
            day_detail_scroll: 0,
            sorted_habit_ids: Vec::new(),
            filter_query: String::new(),
            filter_mode: false,
            dirty: true,
            note_editing: false,
            note_buffer: String::new(),
            note_habit_idx: 0,
        }
    }
}

impl HabitState {
    #[allow(dead_code)]
    pub fn entries_for_habit<'a>(&'a self, habit_id: &str) -> HashMap<&'a str, &'a HabitEntry> {
        self.data
            .entries
            .iter()
            .filter(|e| e.habit_id == habit_id && e.completed)
            .map(|e| (e.date.as_str(), e))
            .collect()
    }

    pub fn is_completed_on(&self, habit_id: &str, date: &str) -> bool {
        self.data
            .entries
            .iter()
            .any(|e| e.habit_id == habit_id && e.date == date && e.completed)
    }

    pub fn get_entry(&self, habit_id: &str, date: &str) -> Option<&HabitEntry> {
        self.data
            .entries
            .iter()
            .find(|e| e.habit_id == habit_id && e.date == date)
    }

    pub fn toggle_entry(&mut self, habit_id: &str, date: &str) {
        if let Some(entry) = self
            .data
            .entries
            .iter_mut()
            .find(|e| e.habit_id == habit_id && e.date == date)
        {
            entry.completed = !entry.completed;
        } else {
            self.data.entries.push(HabitEntry {
                date: date.into(),
                habit_id: habit_id.into(),
                completed: true,
                note: String::new(),
            });
        }
        self.dirty = true;
    }

    pub fn streak(&self, habit_id: &str) -> u32 {
        let today = Local::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();

        if self.is_completed_on(habit_id, &today_str) {
            return self.count_consecutive(habit_id, today);
        }

        if let Some(yesterday) = today.pred_opt() {
            let yesterday_str = yesterday.format("%Y-%m-%d").to_string();
            if self.is_completed_on(habit_id, &yesterday_str) {
                return self.count_consecutive(habit_id, yesterday);
            }
        }

        0
    }

    fn count_consecutive(&self, habit_id: &str, start: NaiveDate) -> u32 {
        let mut count: u32 = 0;
        let mut current = start;
        loop {
            let date_str = current.format("%Y-%m-%d").to_string();
            if !self.is_completed_on(habit_id, &date_str) {
                break;
            }
            count += 1;
            match current.pred_opt() {
                Some(prev) => current = prev,
                None => break,
            }
        }
        count
    }

    pub fn completion_rate(&self, habit_id: &str, days: u32) -> f32 {
        if days == 0 {
            return 0.0;
        }
        let today = Local::now().date_naive();
        let start = today - Days::new((days - 1) as u64);
        let mut completed = 0u32;
        let mut current = start;
        while current <= today {
            let date_str = current.format("%Y-%m-%d").to_string();
            if self.is_completed_on(habit_id, &date_str) {
                completed += 1;
            }
            match current.succ_opt() {
                Some(next) => current = next,
                None => break,
            }
        }
        completed as f32 / days as f32
    }

    pub fn heatmap_weeks(&self, habit_id: &str) -> Vec<Vec<(String, bool)>> {
        let today = Local::now().date_naive();
        let weekday = today.weekday();
        let days_since_sunday = weekday.num_days_from_sunday();

        let week_end = today + Days::new((6 - days_since_sunday) as u64);
        let week_start = week_end - Days::new((17 * 7 - 1) as u64);

        let mut weeks: Vec<Vec<(String, bool)>> = Vec::new();
        let mut current = week_start;

        for _ in 0..17 {
            let mut week: Vec<(String, bool)> = Vec::new();
            for _ in 0..7 {
                let date_str = current.format("%Y-%m-%d").to_string();
                let completed = self.is_completed_on(habit_id, &date_str);
                week.push((date_str, completed));
                match current.succ_opt() {
                    Some(next) => current = next,
                    None => break,
                }
            }
            weeks.push(week);
        }

        weeks
    }

    pub fn mini_heatmap_dates() -> Vec<String> {
        let today = Local::now().date_naive();
        let mut dates = Vec::new();
        for i in (0..37).rev() {
            match today.checked_sub_days(Days::new(i)) {
                Some(d) => dates.push(d.format("%Y-%m-%d").to_string()),
                None => dates.push(today.format("%Y-%m-%d").to_string()),
            }
        }
        dates
    }

    pub fn day_name(date: &str) -> String {
        match NaiveDate::parse_from_str(date, "%Y-%m-%d") {
            Ok(d) => {
                let names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
                let idx = d.weekday().num_days_from_sunday() as usize;
                names[idx].into()
            }
            Err(_) => date.into(),
        }
    }

    pub fn rebuild_sorted(&mut self) {
        let mut pairs: Vec<(String, String)> = self
            .data
            .habits
            .iter()
            .filter(|h| !h.archived)
            .map(|h| (h.created_at.clone(), h.id.clone()))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        self.sorted_habit_ids = pairs.into_iter().map(|(_, id)| id).collect();
    }

    pub fn filtered_habits(&self) -> Vec<&Habit> {
        let ids: Vec<String> = if self.filter_mode && !self.filter_query.is_empty() {
            self.sorted_habit_ids
                .iter()
                .filter(|id| {
                    self.data.habits.iter().any(|h| {
                        h.id == **id
                            && h.name
                                .to_lowercase()
                                .contains(&self.filter_query.to_lowercase())
                    })
                })
                .cloned()
                .collect()
        } else {
            self.sorted_habit_ids.clone()
        };

        ids.iter()
            .filter_map(|id| self.data.habits.iter().find(|h| h.id == *id && !h.archived))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state_with_habit() -> (HabitState, String) {
        let mut state = HabitState::default();
        let habit = Habit {
            id: "exercise-2026-06".into(),
            name: "Exercise".into(),
            description: "Daily workout".into(),
            color: "green".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        };
        let habit_id = habit.id.clone();
        state.data.habits.push(habit);
        state.rebuild_sorted();
        (state, habit_id)
    }

    fn today_str() -> String {
        Local::now().format("%Y-%m-%d").to_string()
    }

    fn yesterday_str() -> String {
        Local::now()
            .date_naive()
            .pred_opt()
            .unwrap()
            .format("%Y-%m-%d")
            .to_string()
    }

    fn days_ago_str(n: u64) -> String {
        Local::now()
            .date_naive()
            .checked_sub_days(Days::new(n))
            .unwrap()
            .format("%Y-%m-%d")
            .to_string()
    }

    #[test]
    fn habit_default_fields() {
        let h = Habit::default();
        assert_eq!(h.name, "");
        assert_eq!(h.color, "green");
        assert!(!h.archived);
        assert!(!h.created_at.is_empty());
    }

    #[test]
    fn habit_entry_default_fields() {
        let e = HabitEntry::default();
        assert!(!e.completed);
        assert_eq!(e.note, "");
        assert!(!e.date.is_empty());
    }

    #[test]
    fn habit_data_default_is_empty() {
        let d = HabitData::default();
        assert!(d.habits.is_empty());
        assert!(d.entries.is_empty());
    }

    #[test]
    fn habit_state_default_is_overview() {
        let s = HabitState::default();
        assert_eq!(s.screen, Screen::Overview);
        assert_eq!(s.cursor, 0);
        assert!(!s.filter_mode);
        assert!(!s.editing);
    }

    #[test]
    fn toggle_entry_creates_if_not_exists() {
        let (mut state, habit_id) = make_state_with_habit();
        let date = today_str();
        state.toggle_entry(&habit_id, &date);
        assert!(state.is_completed_on(&habit_id, &date));
        assert_eq!(state.data.entries.len(), 1);
        assert!(state.data.entries[0].completed);
    }

    #[test]
    fn toggle_entry_toggles_existing() {
        let (mut state, habit_id) = make_state_with_habit();
        let date = today_str();
        state.toggle_entry(&habit_id, &date);
        assert!(state.is_completed_on(&habit_id, &date));
        state.toggle_entry(&habit_id, &date);
        assert!(!state.is_completed_on(&habit_id, &date));
        assert_eq!(state.data.entries.len(), 1);
    }

    #[test]
    fn streak_zero_if_no_entries() {
        let (state, habit_id) = make_state_with_habit();
        assert_eq!(state.streak(&habit_id), 0);
    }

    #[test]
    fn streak_consecutive_days() {
        let (mut state, habit_id) = make_state_with_habit();
        let yesterday = yesterday_str();
        state.toggle_entry(&habit_id, &yesterday);
        let day2 = days_ago_str(2);
        state.toggle_entry(&habit_id, &day2);
        let day3 = days_ago_str(3);
        state.toggle_entry(&habit_id, &day3);
        assert_eq!(state.streak(&habit_id), 3);
    }

    #[test]
    fn streak_broken_by_missed_day() {
        let (mut state, habit_id) = make_state_with_habit();
        let yesterday = yesterday_str();
        state.toggle_entry(&habit_id, &yesterday);
        let day3 = days_ago_str(3);
        state.toggle_entry(&habit_id, &day3);
        assert_eq!(state.streak(&habit_id), 1);
    }

    #[test]
    fn streak_includes_today() {
        let (mut state, habit_id) = make_state_with_habit();
        let today = today_str();
        state.toggle_entry(&habit_id, &today);
        let yesterday = yesterday_str();
        state.toggle_entry(&habit_id, &yesterday);
        assert_eq!(state.streak(&habit_id), 2);
    }

    #[test]
    fn completion_rate_all_completed() {
        let (mut state, habit_id) = make_state_with_habit();
        for i in 0..7 {
            let date = days_ago_str(i);
            state.toggle_entry(&habit_id, &date);
        }
        let rate = state.completion_rate(&habit_id, 7);
        assert!((rate - 1.0).abs() < 0.01);
    }

    #[test]
    fn completion_rate_half() {
        let (mut state, habit_id) = make_state_with_habit();
        for i in 0..7 {
            if i % 2 == 0 {
                let date = days_ago_str(i);
                state.toggle_entry(&habit_id, &date);
            }
        }
        let rate = state.completion_rate(&habit_id, 7);
        assert!((rate - 0.5).abs() <= 0.15);
    }

    #[test]
    fn completion_rate_zero_days() {
        let (state, habit_id) = make_state_with_habit();
        assert_eq!(state.completion_rate(&habit_id, 0), 0.0);
    }

    #[test]
    fn heatmap_weeks_count() {
        let (state, habit_id) = make_state_with_habit();
        let weeks = state.heatmap_weeks(&habit_id);
        assert_eq!(weeks.len(), 17);
        for week in &weeks {
            assert_eq!(week.len(), 7);
        }
    }

    #[test]
    fn heatmap_weeks_sunday_first() {
        let (state, habit_id) = make_state_with_habit();
        let weeks = state.heatmap_weeks(&habit_id);
        for week in &weeks {
            if week.is_empty() {
                continue;
            }
            let d = NaiveDate::parse_from_str(&week[0].0, "%Y-%m-%d").unwrap();
            assert_eq!(d.weekday(), chrono::Weekday::Sun);
        }
    }

    #[test]
    fn entries_for_habit_filters_correctly() {
        let (mut state, habit_id) = make_state_with_habit();
        let today = today_str();
        state.toggle_entry(&habit_id, &today);
        let entries = state.entries_for_habit(&habit_id);
        assert_eq!(entries.len(), 1);
        assert!(entries.contains_key(today.as_str()));
    }

    #[test]
    fn filtered_habits_returns_all_when_no_filter() {
        let (mut state, _) = make_state_with_habit();
        state.data.habits.push(Habit {
            id: "read-2026-06".into(),
            name: "Read".into(),
            description: "Read books".into(),
            color: "blue".into(),
            created_at: "2026-06-02".into(),
            archived: false,
        });
        state.rebuild_sorted();
        let habits = state.filtered_habits();
        assert_eq!(habits.len(), 2);
    }

    #[test]
    fn filtered_habits_filters_by_name() {
        let (mut state, _) = make_state_with_habit();
        state.data.habits.push(Habit {
            id: "read-2026-06".into(),
            name: "Read".into(),
            description: "Read books".into(),
            color: "blue".into(),
            created_at: "2026-06-02".into(),
            archived: false,
        });
        state.rebuild_sorted();
        state.filter_mode = true;
        state.filter_query = "exer".into();
        let habits = state.filtered_habits();
        assert_eq!(habits.len(), 1);
        assert_eq!(habits[0].name, "Exercise");
    }

    #[test]
    fn mini_heatmap_dates_returns_37_days() {
        let dates = HabitState::mini_heatmap_dates();
        assert_eq!(dates.len(), 37);
    }

    #[test]
    fn day_name_returns_correct_day() {
        let name = HabitState::day_name("2026-06-15");
        assert_eq!(name, "Mon");
    }

    #[test]
    fn rebuild_sorted_orders_by_created_at() {
        let mut state = HabitState::default();
        state.data.habits.push(Habit {
            id: "b".into(),
            name: "B".into(),
            description: "".into(),
            color: "green".into(),
            created_at: "2026-06-10".into(),
            archived: false,
        });
        state.data.habits.push(Habit {
            id: "a".into(),
            name: "A".into(),
            description: "".into(),
            color: "blue".into(),
            created_at: "2026-06-01".into(),
            archived: false,
        });
        state.rebuild_sorted();
        assert_eq!(state.sorted_habit_ids, vec!["a", "b"]);
    }

    #[test]
    fn get_entry_finds_existing() {
        let (mut state, habit_id) = make_state_with_habit();
        let today = today_str();
        state.toggle_entry(&habit_id, &today);
        let entry = state.get_entry(&habit_id, &today);
        assert!(entry.is_some());
        assert!(entry.unwrap().completed);
    }

    #[test]
    fn get_entry_returns_none_for_missing() {
        let (state, habit_id) = make_state_with_habit();
        assert!(state.get_entry(&habit_id, "2020-01-01").is_none());
    }
}
