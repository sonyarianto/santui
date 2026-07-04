use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshBookmark {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: Option<String>,
    pub category: String,
    pub description: String,
    pub last_connected_at: Option<u64>,
}

impl Default for SshBookmark {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            host: String::new(),
            port: 22,
            user: String::new(),
            key_path: None,
            category: "General".into(),
            description: String::new(),
            last_connected_at: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SshData {
    pub bookmarks: Vec<SshBookmark>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    Detail,
    Connect,
}

pub struct SshState {
    pub data: SshData,
    pub screen: Screen,
    pub cursor: usize,
    pub scroll: usize,
    pub detail_idx: Option<usize>,
    pub detail_edit_field: usize,
    pub edit_buffer: String,
    pub editing: bool,
    pub filter_text: String,
    pub filter_active: bool,
    pub filtered_indices: Vec<usize>,
    pub connect_in_progress: bool,
    pub message: Option<(String, u64)>,
}

impl Default for SshState {
    fn default() -> Self {
        let mut state = Self {
            data: SshData::default(),
            screen: Screen::List,
            cursor: 0,
            scroll: 0,
            detail_idx: None,
            detail_edit_field: 0,
            edit_buffer: String::new(),
            editing: false,
            filter_text: String::new(),
            filter_active: false,
            filtered_indices: Vec::new(),
            connect_in_progress: false,
            message: None,
        };
        state.rebuild_filtered_indices();
        state
    }
}

impl SshState {
    pub fn rebuild_filtered_indices(&mut self) {
        let ft = self.filter_text.to_lowercase();
        if ft.is_empty() {
            self.filtered_indices = (0..self.data.bookmarks.len()).collect();
        } else {
            self.filtered_indices = self
                .data
                .bookmarks
                .iter()
                .enumerate()
                .filter(|(_, bm)| {
                    bm.name.to_lowercase().contains(&ft)
                        || bm.host.to_lowercase().contains(&ft)
                        || bm.user.to_lowercase().contains(&ft)
                        || bm.category.to_lowercase().contains(&ft)
                })
                .map(|(i, _)| i)
                .collect();
        }
        if self.cursor >= self.filtered_indices.len() {
            self.cursor = self.filtered_indices.len().saturating_sub(1);
        }
    }
}

pub fn generate_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", nanos)
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_default_port_22() {
        let bm = SshBookmark::default();
        assert_eq!(bm.port, 22);
    }

    #[test]
    fn ssh_data_default_empty() {
        let data = SshData::default();
        assert!(data.bookmarks.is_empty());
    }

    fn make_bm(name: &str, host: &str, user: &str, category: &str) -> SshBookmark {
        SshBookmark {
            id: name.to_lowercase().replace(' ', "-"),
            name: name.into(),
            host: host.into(),
            port: 22,
            user: user.into(),
            key_path: None,
            category: category.into(),
            description: String::new(),
            last_connected_at: None,
        }
    }

    fn state_with(bookmarks: Vec<SshBookmark>) -> SshState {
        let mut state = SshState::default();
        state.data.bookmarks = bookmarks;
        state.rebuild_filtered_indices();
        state
    }

    #[test]
    fn filter_matches_name() {
        let mut state = state_with(vec![
            make_bm("Prod Web", "10.0.1.10", "root", "Production"),
            make_bm("Dev Box", "10.0.2.20", "dev", "Dev"),
        ]);
        state.filter_text = "Prod".into();
        state.rebuild_filtered_indices();
        assert_eq!(state.filtered_indices.len(), 1);
        assert_eq!(
            state.data.bookmarks[state.filtered_indices[0]].name,
            "Prod Web"
        );
    }

    #[test]
    fn filter_matches_host() {
        let mut state = state_with(vec![
            make_bm("Prod Web", "10.0.1.10", "root", "Production"),
            make_bm("Dev Box", "10.0.2.20", "dev", "Dev"),
        ]);
        state.filter_text = "10.0.2".into();
        state.rebuild_filtered_indices();
        assert_eq!(state.filtered_indices.len(), 1);
        assert_eq!(
            state.data.bookmarks[state.filtered_indices[0]].name,
            "Dev Box"
        );
    }

    #[test]
    fn filter_matches_user() {
        let mut state = state_with(vec![
            make_bm("Prod Web", "10.0.1.10", "root", "Production"),
            make_bm("Dev Box", "10.0.2.20", "deploy", "Dev"),
        ]);
        state.filter_text = "deploy".into();
        state.rebuild_filtered_indices();
        assert_eq!(state.filtered_indices.len(), 1);
        assert_eq!(
            state.data.bookmarks[state.filtered_indices[0]].user,
            "deploy"
        );
    }

    #[test]
    fn filter_case_insensitive() {
        let mut state = state_with(vec![
            make_bm("Prod Web", "10.0.1.10", "root", "Production"),
            make_bm("Dev Box", "10.0.2.20", "dev", "Dev"),
        ]);
        state.filter_text = "PROD".into();
        state.rebuild_filtered_indices();
        assert_eq!(state.filtered_indices.len(), 1);
    }

    #[test]
    fn filter_matches_category() {
        let mut state = state_with(vec![
            make_bm("Prod Web", "10.0.1.10", "root", "Production"),
            make_bm("Dev Box", "10.0.2.20", "dev", "Dev"),
        ]);
        state.filter_text = "Dev".into();
        state.rebuild_filtered_indices();
        assert_eq!(state.filtered_indices.len(), 1);
        assert_eq!(
            state.data.bookmarks[state.filtered_indices[0]].name,
            "Dev Box"
        );
    }

    #[test]
    fn filter_empty_shows_all() {
        let mut state = state_with(vec![
            make_bm("Prod Web", "10.0.1.10", "root", "Production"),
            make_bm("Dev Box", "10.0.2.20", "dev", "Dev"),
            make_bm("Raspi", "192.168.1.100", "pi", "Personal"),
        ]);
        assert_eq!(state.filtered_indices.len(), 3);
    }
}
