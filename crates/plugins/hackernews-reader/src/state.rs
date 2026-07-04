use crate::api::HnItem;

pub enum Category {
    Top,
    New,
    Best,
}

impl Category {
    pub fn label(&self) -> &str {
        match self {
            Category::Top => "Top Stories",
            Category::New => "New Stories",
            Category::Best => "Best Stories",
        }
    }

    pub fn endpoint(&self) -> &str {
        match self {
            Category::Top => "top",
            Category::New => "new",
            Category::Best => "best",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    StoryList,
    Comments,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FetchState {
    Idle,
    FetchingIds,
    FetchingStories,
    FetchingComments,
    Done,
    Error(String),
}

pub struct HnState {
    pub category: Category,
    pub stories: Vec<HnItem>,
    pub story_ids: Vec<u32>,
    pub loaded_count: usize,
    pub selected: usize,
    pub scroll: usize,
    pub screen: Screen,
    pub comments: Vec<HnItem>,
    pub comment_story: Option<HnItem>,
    pub fetch_state: FetchState,
}

impl Default for HnState {
    fn default() -> Self {
        Self {
            category: Category::Top,
            stories: Vec::new(),
            story_ids: Vec::new(),
            loaded_count: 0,
            selected: 0,
            scroll: 0,
            screen: Screen::StoryList,
            comments: Vec::new(),
            comment_story: None,
            fetch_state: FetchState::Idle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_top_category() {
        let state = HnState::default();
        assert_eq!(state.category.endpoint(), "top");
        assert!(matches!(state.screen, Screen::StoryList));
    }

    #[test]
    fn select_prev_at_top_stays() {
        let mut state = HnState::default();
        state.selected = 0;
        state.selected = state.selected.saturating_sub(1);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_next_at_end_stays() {
        let mut state = HnState::default();
        state.stories = vec![
            HnItem {
                id: 1,
                item_type: crate::api::HnItemType::Story,
                by: None,
                time: None,
                title: Some("A".into()),
                text: None,
                url: None,
                score: None,
                descendants: None,
                kids: None,
                parent: None,
                deleted: None,
                dead: None,
            },
            HnItem {
                id: 2,
                item_type: crate::api::HnItemType::Story,
                by: None,
                time: None,
                title: Some("B".into()),
                text: None,
                url: None,
                score: None,
                descendants: None,
                kids: None,
                parent: None,
                deleted: None,
                dead: None,
            },
        ];
        state.selected = 1;
        let max = state.stories.len().saturating_sub(1);
        state.selected = state.selected.min(max).saturating_add(1).min(max);
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn category_label_returns_correct() {
        assert_eq!(Category::Top.label(), "Top Stories");
        assert_eq!(Category::New.label(), "New Stories");
        assert_eq!(Category::Best.label(), "Best Stories");
    }

    #[test]
    fn category_endpoint_returns_correct() {
        assert_eq!(Category::Top.endpoint(), "top");
        assert_eq!(Category::New.endpoint(), "new");
        assert_eq!(Category::Best.endpoint(), "best");
    }
}
