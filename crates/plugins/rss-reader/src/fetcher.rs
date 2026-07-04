use serde::{Deserialize, Serialize};

pub enum FetchMsg {
    FeedDone { url: String, items: Vec<FeedItem> },
    FeedError { url: String, error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItem {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub url: Option<String>,
    pub published: Option<u64>,
}

pub fn fetch_feed(url: &str) -> Result<Vec<FeedItem>, String> {
    let body = ureq::get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;

    let feed = feed_rs::parser::parse(body.as_bytes()).map_err(|e| e.to_string())?;

    let items = feed
        .entries
        .iter()
        .map(|e| {
            let title = e
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_else(|| "(no title)".into());
            let summary = e
                .summary
                .as_ref()
                .map(|s| strip_html(&s.content))
                .or_else(|| {
                    e.content
                        .as_ref()
                        .map(|c| strip_html(c.body.as_deref().unwrap_or("")))
                })
                .unwrap_or_default();
            let summary = summary.chars().take(500).collect();
            let url = e.links.first().map(|l| l.href.clone());
            let published = e.published.map(|dt| dt.timestamp() as u64);
            let id = e.id.clone();
            FeedItem {
                id,
                title,
                summary,
                url,
                published,
            }
        })
        .collect();

    Ok(items)
}

fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn spawn_fetch(url: String, tx: std::sync::mpsc::Sender<FetchMsg>) {
    std::thread::spawn(move || {
        let msg = match fetch_feed(&url) {
            Ok(items) => FetchMsg::FeedDone { url, items },
            Err(error) => FetchMsg::FeedError { url, error },
        };
        let _ = tx.send(msg);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_removes_tags() {
        let result = strip_html("<p>hello</p>");
        assert_eq!(result, "hello");
    }

    #[test]
    fn strip_html_collapses_whitespace() {
        let result = strip_html("hello    world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn strip_html_empty_string() {
        let result = strip_html("");
        assert_eq!(result, "");
    }

    #[test]
    fn strip_html_no_tags_passthrough() {
        let result = strip_html("hello world");
        assert_eq!(result, "hello world");
    }
}
