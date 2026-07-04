use serde::{Deserialize, Serialize};

pub const HN_BASE: &str = "https://hacker-news.firebaseio.com/v0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HnItemType {
    Job,
    Story,
    Comment,
    Poll,
    Pollopt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnItem {
    pub id: u32,
    #[serde(rename = "type")]
    pub item_type: HnItemType,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub time: Option<u64>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub score: Option<u32>,
    #[serde(default)]
    pub descendants: Option<u32>,
    #[serde(default)]
    pub kids: Option<Vec<u32>>,
    #[serde(default)]
    pub parent: Option<u32>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub dead: Option<bool>,
}

pub fn fetch_story_ids(category: &str) -> Result<Vec<u32>, String> {
    let endpoint = match category {
        "top" => "topstories",
        "new" => "newstories",
        "best" => "beststories",
        _ => "topstories",
    };
    let url = format!("{}/{}.json", HN_BASE, endpoint);
    let mut resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let body: String = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

pub fn fetch_item(id: u32) -> Result<HnItem, String> {
    let url = format!("{}/item/{}.json", HN_BASE, id);
    let mut resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let body: String = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

pub fn fetch_items(ids: &[u32]) -> Result<Vec<HnItem>, String> {
    ids.iter().map(|id| fetch_item(*id)).collect()
}

pub fn strip_html(text: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result
}

pub fn domain_from_url(url: &str) -> Option<String> {
    if let Some(stripped) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    {
        stripped.split('/').next().map(|d| d.to_string())
    } else {
        None
    }
}

pub fn time_ago(unix_ts: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let diff = now.saturating_sub(unix_ts);
    if diff < 60 {
        "just now".into()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_item_with_all_fields() {
        let json = r#"{
            "id": 12345,
            "type": "story",
            "by": "testuser",
            "time": 1710000000,
            "title": "Test Story Title",
            "url": "https://example.com/article",
            "score": 42,
            "descendants": 10,
            "kids": [100, 200, 300]
        }"#;
        let item: HnItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, 12345);
        assert!(matches!(item.item_type, HnItemType::Story));
        assert_eq!(item.by.as_deref(), Some("testuser"));
        assert_eq!(item.title.as_deref(), Some("Test Story Title"));
        assert_eq!(item.url.as_deref(), Some("https://example.com/article"));
        assert_eq!(item.score, Some(42));
        assert_eq!(item.descendants, Some(10));
        assert_eq!(item.kids.as_deref(), Some(&[100, 200, 300][..]));
    }

    #[test]
    fn parse_comment_item() {
        let json = r#"{
            "id": 200,
            "type": "comment",
            "by": "commenter",
            "time": 1710000100,
            "text": "This is a comment.",
            "parent": 12345
        }"#;
        let item: HnItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, 200);
        assert!(matches!(item.item_type, HnItemType::Comment));
        assert_eq!(item.by.as_deref(), Some("commenter"));
        assert_eq!(item.text.as_deref(), Some("This is a comment."));
        assert_eq!(item.parent, Some(12345));
    }

    #[test]
    fn parse_deleted_item() {
        let json = r#"{
            "id": 300,
            "type": "comment",
            "deleted": true,
            "parent": 12345
        }"#;
        let item: HnItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, 300);
        assert_eq!(item.deleted, Some(true));
        assert!(item.by.is_none());
    }

    #[test]
    fn strip_html_removes_tags() {
        assert_eq!(strip_html("<p>Hello <b>world</b></p>"), "Hello world");
        assert_eq!(strip_html("No tags"), "No tags");
        assert_eq!(strip_html("<a href=\"x\">link</a>"), "link");
    }

    #[test]
    fn domain_from_url_extracts_domain() {
        assert_eq!(
            domain_from_url("https://github.com/user/repo"),
            Some("github.com".into())
        );
        assert_eq!(
            domain_from_url("http://example.com/path"),
            Some("example.com".into())
        );
        assert_eq!(domain_from_url("not-a-url"), None);
    }

    #[test]
    fn time_ago_formats() {
        assert_eq!(time_ago(u64::MAX), "just now");
        let one_hour_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(3600);
        assert!(time_ago(one_hour_ago).contains("h ago"));
    }
}
