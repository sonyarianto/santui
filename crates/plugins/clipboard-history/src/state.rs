use serde::{Deserialize, Serialize};

pub const MAX_ENTRIES: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    pub id: u64,
    pub content: String,
    pub preview: String,
}

impl ClipEntry {
    pub fn new(content: String, timestamp: u64) -> Self {
        let preview = content
            .chars()
            .take(80)
            .collect::<String>()
            .replace('\n', "↵")
            .replace('\t', "→");
        Self {
            id: timestamp,
            content,
            preview,
        }
    }
}

pub enum Screen {
    List,
    View(usize),
}

pub struct ClipState {
    pub entries: Vec<ClipEntry>,
    pub screen: Screen,
    pub search_query: String,
    pub filtered: Vec<usize>,
    pub cursor: usize,
    pub view_scroll: usize,
    pub clipboard_error: Option<String>,
    pub last_copied_id: Option<u64>,
    #[allow(dead_code)]
    pub dirty: bool,
}

impl Default for ClipState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipState {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            screen: Screen::List,
            search_query: String::new(),
            filtered: Vec::new(),
            cursor: 0,
            view_scroll: 0,
            clipboard_error: None,
            last_copied_id: None,
            dirty: false,
        }
    }

    pub fn push(&mut self, content: String, timestamp: u64) {
        if self.entries.first().map(|e| &e.content) == Some(&content) {
            return;
        }
        self.entries.insert(0, ClipEntry::new(content, timestamp));
        if self.entries.len() > MAX_ENTRIES {
            self.entries.pop();
        }
        self.apply_filter();
    }

    pub fn apply_filter(&mut self) {
        let q = self.search_query.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| q.is_empty() || e.content.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        self.cursor = self.cursor.min(self.filtered.len().saturating_sub(1));
    }

    pub fn selected_entry(&self) -> Option<&ClipEntry> {
        self.filtered
            .get(self.cursor)
            .and_then(|&i| self.entries.get(i))
    }

    pub fn delete_selected(&mut self) {
        if let Some(&idx) = self.filtered.get(self.cursor) {
            self.entries.remove(idx);
            self.apply_filter();
        }
    }

    pub fn serialize(&self) -> String {
        serde_json::to_string(&self.entries).unwrap()
    }

    pub fn load(&mut self, json: &str) {
        if let Ok(entries) = serde_json::from_str::<Vec<ClipEntry>>(json) {
            self.entries = entries;
            self.apply_filter();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_adds_to_front() {
        let mut state = ClipState::new();
        state.push("second".into(), 200);
        state.push("first".into(), 100);
        assert_eq!(state.entries.len(), 2);
        assert_eq!(state.entries[0].content, "first");
        assert_eq!(state.entries[1].content, "second");
    }

    #[test]
    fn push_deduplicates_consecutive() {
        let mut state = ClipState::new();
        state.push("hello".into(), 100);
        state.push("hello".into(), 200);
        assert_eq!(state.entries.len(), 1);
    }

    #[test]
    fn push_evicts_oldest_at_max() {
        let mut state = ClipState::new();
        for i in 0..MAX_ENTRIES + 5 {
            state.push(format!("entry-{i}"), i as u64);
        }
        assert_eq!(state.entries.len(), MAX_ENTRIES);
        assert_eq!(
            state.entries[0].content,
            format!("entry-{}", MAX_ENTRIES + 4)
        );
    }

    #[test]
    fn delete_selected_removes_correct_entry() {
        let mut state = ClipState::new();
        state.push("first".into(), 100);
        state.push("second".into(), 200);
        state.push("third".into(), 300);
        state.cursor = 1;
        state.delete_selected();
        assert_eq!(state.entries.len(), 2);
        assert_eq!(state.entries[0].content, "third");
        assert_eq!(state.entries[1].content, "first");
    }

    #[test]
    fn apply_filter_empty_returns_all() {
        let mut state = ClipState::new();
        state.push("a".into(), 1);
        state.push("b".into(), 2);
        state.push("c".into(), 3);
        assert_eq!(state.filtered.len(), 3);
    }

    #[test]
    fn apply_filter_matches_substring() {
        let mut state = ClipState::new();
        state.push("hello world".into(), 1);
        state.push("goodbye".into(), 2);
        state.search_query = "world".into();
        state.apply_filter();
        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.entries[state.filtered[0]].content, "hello world");
    }

    #[test]
    fn apply_filter_case_insensitive() {
        let mut state = ClipState::new();
        state.push("Hello World".into(), 1);
        state.search_query = "hello".into();
        state.apply_filter();
        assert_eq!(state.filtered.len(), 1);
    }

    #[test]
    fn clip_entry_preview_truncates_at_80() {
        let long = "x".repeat(100);
        let entry = ClipEntry::new(long, 0);
        assert_eq!(entry.preview.len(), 80);
    }

    #[test]
    fn clip_entry_preview_replaces_newlines() {
        let entry = ClipEntry::new("line1\nline2".into(), 0);
        assert!(!entry.preview.contains('\n'));
        assert!(entry.preview.contains("↵"));
    }

    #[test]
    fn clip_entry_preview_replaces_tabs() {
        let entry = ClipEntry::new("col1\tcol2".into(), 0);
        assert!(!entry.preview.contains('\t'));
        assert!(entry.preview.contains("→"));
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let mut state = ClipState::new();
        state.push("test".into(), 42);
        let json = state.serialize();
        let mut loaded = ClipState::new();
        loaded.load(&json);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].content, "test");
        assert_eq!(loaded.entries[0].id, 42);
    }

    #[test]
    fn load_invalid_json_does_not_panic() {
        let mut state = ClipState::new();
        state.push("original".into(), 1);
        state.load("not valid json");
        assert_eq!(state.entries.len(), 1);
    }
}
