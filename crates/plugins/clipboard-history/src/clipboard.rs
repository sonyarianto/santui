use std::sync::mpsc;

pub enum ClipMsg {
    NewContent(String),
    Error(String),
}

pub fn spawn_clipboard_watcher(tx: mpsc::Sender<ClipMsg>) {
    std::thread::spawn(move || {
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(ClipMsg::Error(e.to_string()));
                return;
            }
        };
        let mut last = String::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            match clipboard.get_text() {
                Ok(text) if !text.is_empty() && text != last => {
                    last = text.clone();
                    if tx.send(ClipMsg::NewContent(text)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    log::warn!("clipboard error: {e}");
                }
                _ => {}
            }
        }
    });
}

pub fn set_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn set_clipboard_roundtrip() {
        let text = "santui-clipboard-test";
        set_clipboard(text).unwrap();
        let mut clipboard = arboard::Clipboard::new().unwrap();
        let result = clipboard.get_text().unwrap();
        assert_eq!(result, text);
    }
}
