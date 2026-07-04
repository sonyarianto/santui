use serde::{Deserialize, Serialize};

use crate::client::{HttpMethod, HttpResponse};

pub const HISTORY_MAX: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistedState {
    #[serde(default)]
    pub history: Vec<RequestEntry>,
    #[serde(default)]
    pub saved_requests: Vec<RequestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEntry {
    pub method: String,
    pub url: String,
    pub headers: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FetchState {
    Idle,
    Sending,
    Done,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Editor,
    Response,
    History,
    Saved,
    MethodPicker,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum EditField {
    Method,
    Url,
    Headers,
    Body,
}

impl EditField {
    pub fn from_idx(idx: usize) -> EditField {
        match idx {
            0 => EditField::Method,
            1 => EditField::Url,
            2 => EditField::Headers,
            3 => EditField::Body,
            _ => EditField::Method,
        }
    }
}

pub struct ClientState {
    pub method: HttpMethod,
    pub url: String,
    pub headers_text: String,
    pub body_text: String,
    pub response: Option<HttpResponse>,
    pub fetch_state: FetchState,
    pub screen: Screen,
    pub edit_field: EditField,
    pub edit_cursor: usize,
    pub edit_scroll: usize,
    pub field_focus_idx: usize,
    pub history: Vec<RequestEntry>,
    pub saved_requests: Vec<RequestEntry>,
    pub history_cursor: usize,
    pub saved_cursor: usize,
    pub response_scroll: usize,
    pub picker_cursor: usize,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            method: HttpMethod::GET,
            url: String::new(),
            headers_text: String::new(),
            body_text: String::new(),
            response: None,
            fetch_state: FetchState::Idle,
            screen: Screen::Editor,
            edit_field: EditField::Method,
            edit_cursor: 0,
            edit_scroll: 0,
            field_focus_idx: 0,
            history: Vec::new(),
            saved_requests: Vec::new(),
            history_cursor: 0,
            saved_cursor: 0,
            response_scroll: 0,
            picker_cursor: 0,
        }
    }
}

impl ClientState {
    pub fn insert_char(&mut self, c: char) {
        match self.edit_field {
            EditField::Method => {}
            EditField::Url => {
                self.url.insert(self.edit_cursor, c);
                self.edit_cursor += 1;
            }
            EditField::Headers => {
                self.headers_text.insert(self.edit_cursor, c);
                self.edit_cursor += 1;
            }
            EditField::Body => {
                self.body_text.insert(self.edit_cursor, c);
                self.edit_cursor += 1;
            }
        }
    }

    pub fn backspace(&mut self) {
        if self.edit_cursor > 0 {
            match self.edit_field {
                EditField::Method => {}
                EditField::Url => {
                    self.url.remove(self.edit_cursor - 1);
                    self.edit_cursor -= 1;
                }
                EditField::Headers => {
                    self.headers_text.remove(self.edit_cursor - 1);
                    self.edit_cursor -= 1;
                }
                EditField::Body => {
                    self.body_text.remove(self.edit_cursor - 1);
                    self.edit_cursor -= 1;
                }
            }
        }
    }

    pub fn cursor_left(&mut self) {
        if self.edit_cursor > 0 {
            self.edit_cursor -= 1;
        }
    }

    pub fn cursor_right(&mut self) {
        let len = match self.edit_field {
            EditField::Method => 0,
            EditField::Url => self.url.len(),
            EditField::Headers => self.headers_text.len(),
            EditField::Body => self.body_text.len(),
        };
        if self.edit_cursor < len {
            self.edit_cursor += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_state_default_empty_history() {
        let p = PersistedState::default();
        assert!(p.history.is_empty());
        assert!(p.saved_requests.is_empty());
    }

    #[test]
    fn persisted_state_deserialize_empty() {
        let p: PersistedState = serde_json::from_str("{}").unwrap();
        assert!(p.history.is_empty());
        assert!(p.saved_requests.is_empty());
    }

    #[test]
    fn field_focus_cycles_correctly() {
        assert_eq!(EditField::from_idx(0), EditField::Method);
        assert_eq!(EditField::from_idx(1), EditField::Url);
        assert_eq!(EditField::from_idx(2), EditField::Headers);
        assert_eq!(EditField::from_idx(3), EditField::Body);
        assert_eq!(EditField::from_idx(999), EditField::Method);
    }

    #[test]
    fn history_limited_to_50() {
        assert_eq!(HISTORY_MAX, 50);
    }

    #[test]
    fn client_state_default() {
        let s = ClientState::default();
        assert_eq!(s.method, HttpMethod::GET);
        assert!(s.url.is_empty());
        assert!(s.headers_text.is_empty());
        assert!(s.body_text.is_empty());
        assert!(s.response.is_none());
        assert_eq!(s.fetch_state, FetchState::Idle);
        assert_eq!(s.screen, Screen::Editor);
        assert_eq!(s.field_focus_idx, 0);
        assert_eq!(s.edit_cursor, 0);
        assert!(s.history.is_empty());
        assert!(s.saved_requests.is_empty());
    }

    #[test]
    fn insert_char_url() {
        let mut s = ClientState::default();
        s.edit_field = EditField::Url;
        s.insert_char('h');
        s.insert_char('t');
        s.insert_char('t');
        s.insert_char('p');
        assert_eq!(s.url, "http");
        assert_eq!(s.edit_cursor, 4);
    }

    #[test]
    fn backspace_url() {
        let mut s = ClientState::default();
        s.edit_field = EditField::Url;
        s.url = "abc".into();
        s.edit_cursor = 3;
        s.backspace();
        assert_eq!(s.url, "ab");
        assert_eq!(s.edit_cursor, 2);
    }

    #[test]
    fn backspace_at_start_does_nothing() {
        let mut s = ClientState::default();
        s.edit_field = EditField::Url;
        s.url = "abc".into();
        s.edit_cursor = 0;
        s.backspace();
        assert_eq!(s.url, "abc");
        assert_eq!(s.edit_cursor, 0);
    }

    #[test]
    fn cursor_left_right() {
        let mut s = ClientState::default();
        s.edit_field = EditField::Url;
        s.url = "abc".into();
        s.edit_cursor = 3;
        s.cursor_left();
        assert_eq!(s.edit_cursor, 2);
        s.cursor_right();
        assert_eq!(s.edit_cursor, 3);
        s.cursor_right();
        assert_eq!(s.edit_cursor, 3);
    }

    #[test]
    fn cursor_left_at_start() {
        let mut s = ClientState::default();
        s.edit_field = EditField::Url;
        s.url = "abc".into();
        s.edit_cursor = 0;
        s.cursor_left();
        assert_eq!(s.edit_cursor, 0);
    }

    #[test]
    fn insert_char_headers() {
        let mut s = ClientState::default();
        s.edit_field = EditField::Headers;
        s.insert_char('X');
        assert_eq!(s.headers_text, "X");
        assert_eq!(s.edit_cursor, 1);
    }

    #[test]
    fn insert_char_body() {
        let mut s = ClientState::default();
        s.edit_field = EditField::Body;
        s.insert_char('{');
        assert_eq!(s.body_text, "{");
        assert_eq!(s.edit_cursor, 1);
    }

    #[test]
    fn backspace_body() {
        let mut s = ClientState::default();
        s.edit_field = EditField::Body;
        s.body_text = "{}".into();
        s.edit_cursor = 2;
        s.backspace();
        assert_eq!(s.body_text, "{");
        assert_eq!(s.edit_cursor, 1);
    }

    #[test]
    fn cursor_right_body_past_end() {
        let mut s = ClientState::default();
        s.edit_field = EditField::Body;
        s.body_text = "hi".into();
        s.edit_cursor = 2;
        s.cursor_right();
        assert_eq!(s.edit_cursor, 2);
    }
}
