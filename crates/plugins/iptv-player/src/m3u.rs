use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_PLAYLIST_URL: &str = "https://iptv-org.github.io/iptv/index.m3u";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    pub url: String,
    pub tvg_id: Option<String>,
    pub tvg_name: Option<String>,
    pub tvg_logo: Option<String>,
    pub group_title: Option<String>,
    pub attrs: BTreeMap<String, String>,
}

pub fn parse(content: &str) -> Vec<Channel> {
    let mut channels = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() || (line.starts_with('#') && !line.starts_with("#EXTINF")) {
            i += 1;
            continue;
        }

        if line.starts_with("#EXTINF") {
            let inf_line = line;
            let name = extract_name(inf_line);
            let attrs = extract_attrs(inf_line);

            i += 1;
            while i < lines.len() {
                let url_line = lines[i].trim();
                if url_line.starts_with("#EXTINF") || url_line.starts_with("#EXTM3U") {
                    i = i.saturating_sub(1);
                    break;
                }
                if url_line.is_empty() || url_line.starts_with('#') {
                    i += 1;
                    continue;
                }
                channels.push(Channel {
                    name: name.unwrap_or_default(),
                    url: url_line.to_string(),
                    tvg_id: attrs.get("tvg-id").cloned(),
                    tvg_name: attrs.get("tvg-name").cloned(),
                    tvg_logo: attrs.get("tvg-logo").cloned(),
                    group_title: attrs.get("group-title").cloned(),
                    attrs,
                });
                break;
            }
        }

        i += 1;
    }

    channels
}

fn extract_name(line: &str) -> Option<String> {
    let prefix = line.strip_prefix("#EXTINF:")?;
    let comma_idx = find_name_comma(prefix)?;
    let name = prefix[comma_idx + 1..].trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn extract_attrs(line: &str) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();

    let prefix = match line.strip_prefix("#EXTINF:") {
        Some(p) => p,
        None => return attrs,
    };

    let end = find_name_comma(prefix).unwrap_or(prefix.len());
    let content = &prefix[..end];

    let remaining = find_after_duration(content);

    if remaining.is_empty() {
        return attrs;
    }

    parse_key_values(remaining, &mut attrs);
    attrs
}

fn find_name_comma(s: &str) -> Option<usize> {
    let mut in_quotes = false;
    for (i, ch) in s.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == ',' && !in_quotes {
            return Some(i);
        }
    }
    None
}

fn find_after_duration(content: &str) -> &str {
    let s = content.trim_start();
    if let Some(rest) = s.strip_prefix('-') {
        let rest = rest.trim_start();
        let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
        rest[end..].trim_start()
    } else {
        let end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
        s[end..].trim_start()
    }
}

fn parse_key_values(input: &str, attrs: &mut BTreeMap<String, String>) {
    let mut remaining = input;
    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        let Some(eq_idx) = remaining.find('=') else {
            break;
        };
        let key = remaining[..eq_idx].trim().to_string();
        remaining = &remaining[eq_idx + 1..];

        if remaining.starts_with('"') {
            remaining = &remaining[1..];
            let mut value = String::new();
            while let Some(ch) = remaining.chars().next() {
                remaining = &remaining[ch.len_utf8()..];
                if ch == '"' {
                    break;
                }
                value.push(ch);
            }
            attrs.insert(key, value);
        } else {
            let end = remaining
                .find(|c: char| c.is_whitespace())
                .unwrap_or(remaining.len());
            let value = remaining[..end].to_string();
            remaining = &remaining[end..];
            attrs.insert(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_m3u() {
        let input = "#EXTM3U\n#EXTINF:-1 tvg-id=\"abc.xyz\" tvg-name=\"ABC HD\" tvg-logo=\"https://logo.png\" group-title=\"News\",ABC HD\nhttps://example.com/stream.m3u8\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "ABC HD");
        assert_eq!(channels[0].url, "https://example.com/stream.m3u8");
        assert_eq!(channels[0].tvg_id.as_deref(), Some("abc.xyz"));
        assert_eq!(channels[0].tvg_name.as_deref(), Some("ABC HD"));
        assert_eq!(channels[0].tvg_logo.as_deref(), Some("https://logo.png"));
        assert_eq!(channels[0].group_title.as_deref(), Some("News"));
    }

    #[test]
    fn parse_quoted_attrs_with_spaces() {
        let input = "#EXTM3U\n#EXTINF:-1 tvg-id=\"my id\" group-title=\"News 24\",Channel Name\nhttp://example.com/stream\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].tvg_id.as_deref(), Some("my id"));
        assert_eq!(channels[0].group_title.as_deref(), Some("News 24"));
    }

    #[test]
    fn skip_malformed_entries() {
        let input = "#EXTM3U\n#EXTINF:-1,Valid\nhttp://valid\n#EXTINF:-1,NoURL\n#Comment\n#EXTINF:-1,Also Valid\nhttp://also-valid\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].name, "Valid");
        assert_eq!(channels[1].name, "Also Valid");
    }

    #[test]
    fn skip_empty_entries() {
        let input = "#EXTM3U\n\n#EXTINF:-1,\n\nhttp://stream\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "");
        assert_eq!(channels[0].url, "http://stream");
    }

    #[test]
    fn preserve_unknown_attrs() {
        let input =
            "#EXTM3U\n#EXTINF:-1 custom-key=\"custom-val\" tvg-id=\"x\",Test\nhttp://test\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 1);
        assert_eq!(
            channels[0].attrs.get("custom-key").map(|s| s.as_str()),
            Some("custom-val")
        );
    }

    #[test]
    fn duplicate_names_ok() {
        let input = "#EXTM3U\n#EXTINF:-1,Dup\nhttp://a\n#EXTINF:-1,Dup\nhttp://b\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 2);
    }

    #[test]
    fn no_extinf_lines() {
        let input = "#EXTM3U\n#Comment\nhttp://no-extinf\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 0);
    }

    #[test]
    fn name_with_comma() {
        let input = "#EXTM3U\n#EXTINF:-1,Channel, With Comma\nhttp://stream\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "Channel, With Comma");
    }

    #[test]
    fn attrs_after_name_comma() {
        let input = "#EXTM3U\n#EXTINF:-1 tvg-id=\"x\",Channel Name extra stuff\nhttp://stream\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "Channel Name extra stuff");
        assert_eq!(channels[0].tvg_id.as_deref(), Some("x"));
    }

    #[test]
    fn empty_input() {
        let channels = parse("");
        assert!(channels.is_empty());
    }

    #[test]
    fn missing_url_skipped() {
        let input = "#EXTM3U\n#EXTINF:-1,No URL\n#EXTINF:-1,Has URL\nhttp://has-url\n";
        let channels = parse(input);
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "Has URL");
    }
}
