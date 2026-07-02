use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::fetcher::FeedItem;

pub const REFRESH_TICKS: u32 = 9000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub url: String,
    pub title: String,
    pub last_fetched: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadState {
    pub read_ids: HashSet<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RssData {
    pub feeds: Vec<Feed>,
    pub read_state: ReadState,
}

#[derive(Debug, Clone)]
pub struct DisplayItem {
    pub feed_url: String,
    pub feed_title: String,
    pub item: FeedItem,
    pub is_read: bool,
}

#[derive(Default)]
pub enum FetchStatus {
    #[default]
    Idle,
    Fetching(usize),
    #[allow(dead_code)]
    Error(String),
}

#[derive(Clone, Default)]
pub enum Screen {
    #[default]
    FeedList,
    ItemList(Option<String>),
    ItemView(usize),
    AddFeed,
    ConfirmRemoveFeed(usize),
}

#[allow(dead_code)]
pub struct RssState {
    pub data: RssData,
    pub current_items: Vec<DisplayItem>,
    pub all_items: Vec<DisplayItem>,
    pub fetch_status: FetchStatus,
    pub screen: Screen,
    pub feed_cursor: usize,
    pub item_cursor: usize,
    pub item_scroll: usize,
    pub add_url_buf: String,
    pub search_query: String,
    pub ticks_since_refresh: u32,
    pub dirty: bool,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Default for RssState {
    fn default() -> Self {
        Self::new()
    }
}

impl RssState {
    pub fn new() -> Self {
        Self {
            data: RssData::default(),
            current_items: Vec::new(),
            all_items: Vec::new(),
            fetch_status: FetchStatus::Idle,
            screen: Screen::FeedList,
            feed_cursor: 0,
            item_cursor: 0,
            item_scroll: 0,
            add_url_buf: String::new(),
            search_query: String::new(),
            ticks_since_refresh: 0,
            dirty: false,
        }
    }

    pub fn add_feed(&mut self, url: String) {
        if self.data.feeds.iter().any(|f| f.url == url) {
            return;
        }
        self.data.feeds.push(Feed {
            url,
            title: "Loading...".into(),
            last_fetched: None,
        });
    }

    pub fn remove_feed(&mut self, idx: usize) {
        if idx < self.data.feeds.len() {
            let url = self.data.feeds.remove(idx).url;
            self.all_items.retain(|i| i.feed_url != url);
            self.rebuild_current_items(&self.screen.clone());
        }
    }

    pub fn mark_read(&mut self, item_id: &str) {
        self.data.read_state.read_ids.insert(item_id.to_owned());
        for item in self
            .current_items
            .iter_mut()
            .chain(self.all_items.iter_mut())
        {
            if item.item.id == item_id {
                item.is_read = true;
            }
        }
    }

    pub fn unread_count(&self, feed_url: &str) -> usize {
        self.all_items
            .iter()
            .filter(|i| i.feed_url == feed_url && !i.is_read)
            .count()
    }

    pub fn total_unread(&self) -> usize {
        self.all_items.iter().filter(|i| !i.is_read).count()
    }

    pub fn apply_feed_items(
        &mut self,
        feed_url: &str,
        items: Vec<FeedItem>,
        feed_title_from_meta: Option<String>,
    ) {
        if let Some(title) = feed_title_from_meta {
            if let Some(feed) = self.data.feeds.iter_mut().find(|f| f.url == feed_url) {
                if feed.title == "Loading..." {
                    feed.title = title;
                }
                feed.last_fetched = Some(now_secs());
            }
        }
        for item in items {
            if !self.all_items.iter().any(|d| d.item.id == item.id) {
                let is_read = self.data.read_state.read_ids.contains(&item.id);
                let feed = self.data.feeds.iter().find(|f| f.url == feed_url);
                self.all_items.push(DisplayItem {
                    feed_url: feed_url.to_owned(),
                    feed_title: feed.map(|f| f.title.clone()).unwrap_or_default(),
                    item,
                    is_read,
                });
            }
        }
        self.all_items.sort_by(|a, b| {
            b.item
                .published
                .unwrap_or(0)
                .cmp(&a.item.published.unwrap_or(0))
        });
    }

    pub fn rebuild_current_items(&mut self, screen: &Screen) {
        self.current_items = match screen {
            Screen::ItemList(Some(url)) => self
                .all_items
                .iter()
                .filter(|i| &i.feed_url == url)
                .cloned()
                .collect(),
            _ => self.all_items.clone(),
        };
    }

    pub fn serialize(&self) -> String {
        serde_json::to_string(&self.data).unwrap()
    }

    pub fn load(&mut self, json: &str) {
        if let Ok(data) = serde_json::from_str::<RssData>(json) {
            self.data = data;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(id: &str, published: u64) -> FeedItem {
        FeedItem {
            id: id.to_string(),
            title: format!("title {id}"),
            summary: "summary".into(),
            url: Some(format!("http://example.com/{id}")),
            published: Some(published),
        }
    }

    fn feed_state() -> RssState {
        let mut state = RssState::new();
        state.data.feeds.push(Feed {
            url: "http://feed1".into(),
            title: "Feed 1".into(),
            last_fetched: None,
        });
        state.data.feeds.push(Feed {
            url: "http://feed2".into(),
            title: "Feed 2".into(),
            last_fetched: None,
        });
        state
    }

    #[test]
    fn add_feed_deduplicates() {
        let mut state = RssState::new();
        state.add_feed("http://same".into());
        state.add_feed("http://same".into());
        assert_eq!(state.data.feeds.len(), 1);
    }

    #[test]
    fn remove_feed_removes_items() {
        let mut state = feed_state();
        state.apply_feed_items("http://feed1", vec![make_item("a", 100)], None);
        state.apply_feed_items("http://feed2", vec![make_item("b", 200)], None);
        assert_eq!(state.all_items.len(), 2);
        state.remove_feed(0);
        assert_eq!(state.all_items.len(), 1);
        assert_eq!(state.all_items[0].feed_url, "http://feed2");
    }

    #[test]
    fn mark_read_updates_item() {
        let mut state = feed_state();
        state.apply_feed_items("http://feed1", vec![make_item("a", 100)], None);
        state.rebuild_current_items(&Screen::ItemList(Some("http://feed1".into())));
        assert!(!state.current_items[0].is_read);
        assert!(!state.all_items[0].is_read);
        state.mark_read("a");
        assert!(state.current_items[0].is_read);
        assert!(state.all_items[0].is_read);
    }

    #[test]
    fn unread_count_correct() {
        let mut state = feed_state();
        state.apply_feed_items(
            "http://feed1",
            vec![make_item("a", 100), make_item("b", 200)],
            None,
        );
        state.mark_read("a");
        assert_eq!(state.unread_count("http://feed1"), 1);
        assert_eq!(state.unread_count("http://feed2"), 0);
    }

    #[test]
    fn total_unread_correct() {
        let mut state = feed_state();
        state.apply_feed_items("http://feed1", vec![make_item("a", 100)], None);
        state.apply_feed_items("http://feed2", vec![make_item("b", 200)], None);
        state.mark_read("a");
        assert_eq!(state.total_unread(), 1);
    }

    #[test]
    fn apply_feed_items_deduplicates_by_id() {
        let mut state = feed_state();
        state.apply_feed_items("http://feed1", vec![make_item("a", 100)], None);
        state.apply_feed_items("http://feed1", vec![make_item("a", 100)], None);
        assert_eq!(state.all_items.len(), 1);
    }

    #[test]
    fn apply_feed_items_sorts_newest_first() {
        let mut state = feed_state();
        state.apply_feed_items(
            "http://feed1",
            vec![
                make_item("a", 100),
                make_item("b", 300),
                make_item("c", 200),
            ],
            None,
        );
        assert_eq!(state.all_items[0].item.id, "b");
        assert_eq!(state.all_items[1].item.id, "c");
        assert_eq!(state.all_items[2].item.id, "a");
    }

    #[test]
    fn rebuild_current_items_all_feeds() {
        let mut state = feed_state();
        state.apply_feed_items("http://feed1", vec![make_item("a", 100)], None);
        state.apply_feed_items("http://feed2", vec![make_item("b", 200)], None);
        state.rebuild_current_items(&Screen::ItemList(None));
        assert_eq!(state.current_items.len(), 2);
    }

    #[test]
    fn rebuild_current_items_single_feed() {
        let mut state = feed_state();
        state.apply_feed_items("http://feed1", vec![make_item("a", 100)], None);
        state.apply_feed_items("http://feed2", vec![make_item("b", 200)], None);
        state.rebuild_current_items(&Screen::ItemList(Some("http://feed1".into())));
        assert_eq!(state.current_items.len(), 1);
        assert_eq!(state.current_items[0].feed_url, "http://feed1");
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let mut state = feed_state();
        state.data.read_state.read_ids.insert("a".into());
        let json = state.serialize();
        let mut loaded = RssState::new();
        loaded.load(&json);
        assert_eq!(loaded.data.feeds.len(), 2);
        assert!(loaded.data.read_state.read_ids.contains("a"));
    }

    #[test]
    fn load_invalid_json_does_not_panic() {
        let mut state = RssState::new();
        state.load("not valid json");
        assert_eq!(state.data.feeds.len(), 0);
    }

    #[test]
    fn load_does_not_persist_items() {
        let mut state = feed_state();
        state.apply_feed_items("http://feed1", vec![make_item("a", 100)], None);
        let json = state.serialize();
        let mut loaded = RssState::new();
        loaded.load(&json);
        assert!(loaded.all_items.is_empty());
    }
}
