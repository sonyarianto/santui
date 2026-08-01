use crate::protocol::IpcKey;

/// Handle a key pressed while in search mode. Returns `true` (consumed).
///
/// When Enter is pressed with a non-empty query, search mode is left active
/// on an empty query; the trimmed query is returned via the second value so
/// the caller can trigger the actual search.
pub fn handle_search_key(
    search_mode: &mut bool,
    query: &mut String,
    dirty: &mut bool,
    key: IpcKey,
) -> (bool, Option<String>) {
    match key {
        IpcKey::Esc => {
            *search_mode = false;
            query.clear();
            *dirty = true;
            (true, None)
        }
        IpcKey::Enter => {
            let q = query.trim().to_string();
            if !q.is_empty() {
                *search_mode = false;
                (true, Some(q))
            } else {
                *dirty = true;
                (true, None)
            }
        }
        IpcKey::Backspace => {
            query.pop();
            *dirty = true;
            (true, None)
        }
        IpcKey::Char(_) if !*search_mode => (false, None),
        IpcKey::Char(c) if !c.is_control() => {
            query.push(c);
            *dirty = true;
            (true, None)
        }
        _ => (true, None),
    }
}

/// Enter search mode: activate it and clear the query.
pub fn enter_search_mode(search_mode: &mut bool, query: &mut String, dirty: &mut bool) {
    *search_mode = true;
    query.clear();
    *dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_enters_search_mode() {
        let mut mode = false;
        let mut query = String::new();
        enter_search_mode(&mut mode, &mut query, &mut false);
        assert!(mode);
        assert!(query.is_empty());
    }

    #[test]
    fn esc_exits_search_mode_and_clears_query() {
        let mut mode = true;
        let mut query = "test".into();
        assert_eq!(
            handle_search_key(&mut mode, &mut query, &mut false, IpcKey::Esc),
            (true, None)
        );
        assert!(!mode);
        assert!(query.is_empty());
    }

    #[test]
    fn backspace_removes_from_query() {
        let mut mode = true;
        let mut query = "ab".into();
        assert_eq!(
            handle_search_key(&mut mode, &mut query, &mut false, IpcKey::Backspace),
            (true, None)
        );
        assert_eq!(query, "a");
        assert_eq!(
            handle_search_key(&mut mode, &mut query, &mut false, IpcKey::Backspace),
            (true, None)
        );
        assert_eq!(query, "");
    }

    #[test]
    fn char_inside_search_appends_to_query() {
        let mut mode = true;
        let mut query = String::new();
        assert_eq!(
            handle_search_key(&mut mode, &mut query, &mut false, IpcKey::Char('x')),
            (true, None)
        );
        assert_eq!(query, "x");
    }

    #[test]
    fn char_outside_search_ignored() {
        let mut mode = false;
        let mut query = String::new();
        assert_eq!(
            handle_search_key(&mut mode, &mut query, &mut false, IpcKey::Char('a')),
            (false, None)
        );
        assert_eq!(query, "");
    }

    #[test]
    fn enter_empty_does_not_submit() {
        let mut mode = true;
        let mut query = "   ".into();
        let (consumed, sub) = handle_search_key(&mut mode, &mut query, &mut false, IpcKey::Enter);
        assert!(consumed);
        assert!(mode);
        assert!(sub.is_none());
    }

    #[test]
    fn enter_triggers_submit_with_trimmed_query() {
        let mut mode = true;
        let mut query = " eminem ".into();
        let (consumed, sub) = handle_search_key(&mut mode, &mut query, &mut false, IpcKey::Enter);
        assert!(consumed);
        assert!(!mode);
        assert_eq!(sub.unwrap(), "eminem");
    }
}
