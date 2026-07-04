use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    View(usize),
    Edit(usize),
    NewTitle,
    Rename(usize),
    ConfirmDelete(usize),
}

pub struct NotesState {
    pub notes: Vec<Note>,
    pub screen: Screen,
    pub search_query: String,
    pub list_cursor: usize,
    pub filtered_indices: Vec<usize>,
    pub edit_buf: String,
    pub title_buf: String,
    pub scroll_offset: usize,
}

impl Default for NotesState {
    fn default() -> Self {
        Self {
            notes: Vec::new(),
            screen: Screen::List,
            search_query: String::new(),
            list_cursor: 0,
            filtered_indices: Vec::new(),
            edit_buf: String::new(),
            title_buf: String::new(),
            scroll_offset: 0,
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl NotesState {
    pub fn apply_filter(&mut self) {
        let q = self.search_query.to_lowercase();
        let mut indices: Vec<(usize, u64)> = self
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                q.is_empty()
                    || n.title.to_lowercase().contains(&q)
                    || n.body.to_lowercase().contains(&q)
            })
            .map(|(i, n)| (i, n.updated_at))
            .collect();
        indices.sort_by_key(|b| std::cmp::Reverse(b.1));
        self.filtered_indices = indices.into_iter().map(|(i, _)| i).collect();
        if self.list_cursor >= self.filtered_indices.len() {
            self.list_cursor = self.filtered_indices.len().saturating_sub(1);
        }
    }

    pub fn add_note(&mut self, title: String) -> usize {
        let now = unix_now();
        let note = Note {
            id: now,
            title,
            body: String::new(),
            updated_at: now,
        };
        self.notes.push(note);
        self.apply_filter();
        self.notes.len() - 1
    }

    pub fn delete_note(&mut self, idx: usize) {
        if idx < self.notes.len() {
            self.notes.remove(idx);
            self.apply_filter();
        }
    }

    pub fn save_edit(&mut self, idx: usize) {
        if idx < self.notes.len() {
            self.notes[idx].body = self.edit_buf.clone();
            self.notes[idx].updated_at = unix_now();
        }
    }

    pub fn serialize(&self) -> String {
        serde_json::to_string(&self.notes).unwrap_or_default()
    }

    pub fn load(&mut self, json: &str) {
        if let Ok(notes) = serde_json::from_str::<Vec<Note>>(json) {
            self.notes = notes;
            self.apply_filter();
        }
    }

    pub fn selected_note_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.list_cursor).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_note_increments_count() {
        let mut state = NotesState::default();
        state.add_note("Test".into());
        assert_eq!(state.notes.len(), 1);
    }

    #[test]
    fn delete_note_removes_correct_entry() {
        let mut state = NotesState::default();
        state.add_note("A".into());
        state.add_note("B".into());
        state.add_note("C".into());
        state.delete_note(1);
        assert_eq!(state.notes.len(), 2);
        assert_eq!(state.notes[0].title, "A");
        assert_eq!(state.notes[1].title, "C");
    }

    #[test]
    fn save_edit_updates_body() {
        let mut state = NotesState::default();
        state.add_note("Test".into());
        state.edit_buf = "new body".into();
        state.save_edit(0);
        assert_eq!(state.notes[0].body, "new body");
    }

    #[test]
    fn apply_filter_empty_query_returns_all() {
        let mut state = NotesState::default();
        state.add_note("A".into());
        state.add_note("B".into());
        state.apply_filter();
        assert_eq!(state.filtered_indices.len(), 2);
    }

    #[test]
    fn apply_filter_matches_title() {
        let mut state = NotesState::default();
        state.add_note("Hello".into());
        state.add_note("World".into());
        state.search_query = "hello".into();
        state.apply_filter();
        assert_eq!(state.filtered_indices.len(), 1);
    }

    #[test]
    fn apply_filter_matches_body() {
        let mut state = NotesState::default();
        let idx = state.add_note("Note".into());
        state.notes[idx].body = "some content here".into();
        state.search_query = "content".into();
        state.apply_filter();
        assert_eq!(state.filtered_indices.len(), 1);
    }

    #[test]
    fn apply_filter_no_match_returns_empty() {
        let mut state = NotesState::default();
        state.add_note("Hello".into());
        state.search_query = "zzz".into();
        state.apply_filter();
        assert!(state.filtered_indices.is_empty());
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let mut state = NotesState::default();
        state.add_note("A".into());
        state.add_note("B".into());
        let json = state.serialize();
        let mut state2 = NotesState::default();
        state2.load(&json);
        assert_eq!(state2.notes.len(), 2);
        assert_eq!(state2.notes[0].title, "A");
        assert_eq!(state2.notes[1].title, "B");
    }

    #[test]
    fn load_invalid_json_does_not_panic() {
        let mut state = NotesState::default();
        state.add_note("A".into());
        state.load("not json");
        assert_eq!(state.notes.len(), 1);
    }
}
