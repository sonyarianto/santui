use santui_ipc::protocol::{Area, ThemeData};
use santui_registry::Registry;
use std::path::PathBuf;
use std::sync::mpsc;

pub(super) enum DownloadEvent {
    Progress(u64, u64),
    Done,
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Action {
    Enable,
    Disable,
    Install,
    Update,
    Delete,
    Launch,
}

pub struct App {
    pub(super) registry: Option<Registry>,
    pub(super) cursor: usize,
    pub(super) scroll: u16,
    pub(super) status: String,
    pub(super) status_ticks: u64,
    pub(super) detail_idx: Option<usize>,
    pub(super) action_cursor: usize,
    pub(super) theme: ThemeData,
    pub(super) area: Area,
    pub(super) plugins_dir: PathBuf,
    pub(super) download_rx: Option<mpsc::Receiver<DownloadEvent>>,
    pub(super) download_progress: Option<(u64, u64)>,
    pub(super) pending_install_id: Option<String>,
    pub(super) pending_install_name: Option<String>,
    pub(super) pending_install_version: Option<String>,
    pub(super) pending_install_capabilities: Vec<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        App {
            registry: None,
            cursor: 0,
            scroll: 0,
            status: String::new(),
            status_ticks: 0,
            detail_idx: None,
            action_cursor: 0,
            theme: ThemeData {
                text: [220; 3],
                text_muted: [140; 3],
                accent: [150; 3],
                highlight: [250; 3],
                logo: [255; 3],
                background: [20; 3],
                background_panel: [20; 3],
                background_overlay: [10; 3],
                border: [250; 3],
                success: [120; 3],
                error: [220; 3],
                inverted_text: [20; 3],
            },
            area: Area { w: 80, h: 24 },
            plugins_dir: PathBuf::new(),
            download_rx: None,
            download_progress: None,
            pending_install_id: None,
            pending_install_name: None,
            pending_install_version: None,
            pending_install_capabilities: Vec::new(),
        }
    }

    pub fn available_count(&self) -> usize {
        self.registry
            .as_ref()
            .map(|r| r.available.len())
            .unwrap_or(0)
    }

    pub fn ensure_scroll_visible(&mut self) {
        let list_h = super::render::max_list_h(self.area.h)
            .min(self.available_count() as u16)
            .max(1);
        let cursor = self.cursor.min(self.available_count().saturating_sub(1)) as u16;
        if cursor < self.scroll {
            self.scroll = cursor;
        } else if cursor >= self.scroll + list_h {
            self.scroll = cursor.saturating_sub(list_h.saturating_sub(1));
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status = msg;
        self.status_ticks = 0;
    }

    /// Called every Tick (~100 ms). Auto‑dismisses the status after ~2 s.
    pub fn tick_status(&mut self) {
        if !self.status.is_empty() {
            self.status_ticks += 1;
            if self.status_ticks > 20 {
                self.status.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new_defaults() {
        let app = App::new();
        assert!(app.registry.is_none());
        assert_eq!(app.cursor, 0);
        assert_eq!(app.scroll, 0);
        assert!(app.status.is_empty());
        assert_eq!(app.status_ticks, 0);
        assert!(app.detail_idx.is_none());
        assert_eq!(app.action_cursor, 0);
        assert_eq!(app.area.w, 80);
        assert_eq!(app.area.h, 24);
        assert!(app.plugins_dir.as_os_str().is_empty());
        assert!(app.download_rx.is_none());
        assert!(app.download_progress.is_none());
        assert!(app.pending_install_id.is_none());
        assert!(app.pending_install_name.is_none());
        assert!(app.pending_install_version.is_none());
        assert!(app.pending_install_capabilities.is_empty());
    }

    #[test]
    fn test_available_count_no_registry() {
        let app = App::new();
        assert_eq!(app.available_count(), 0);
    }

    #[test]
    fn test_set_status() {
        let mut app = App::new();
        assert!(app.status.is_empty());
        app.set_status("hello".into());
        assert_eq!(app.status, "hello");
        assert_eq!(app.status_ticks, 0);
    }

    #[test]
    fn test_set_status_overwrites_previous() {
        let mut app = App::new();
        app.set_status("first".into());
        app.status_ticks = 10;
        app.set_status("second".into());
        assert_eq!(app.status, "second");
        assert_eq!(app.status_ticks, 0);
    }

    #[test]
    fn test_tick_status_auto_dismiss() {
        let mut app = App::new();
        app.set_status("test".into());
        for _ in 0..20 {
            app.tick_status();
        }
        assert_eq!(app.status, "test");
        app.tick_status();
        assert!(app.status.is_empty());
    }

    #[test]
    fn test_tick_status_no_status() {
        let mut app = App::new();
        app.tick_status();
        assert!(app.status.is_empty());
        assert_eq!(app.status_ticks, 0);
    }

    #[test]
    fn test_tick_status_increments_ticks() {
        let mut app = App::new();
        app.set_status("test".into());
        app.tick_status();
        assert_eq!(app.status_ticks, 1);
        app.tick_status();
        assert_eq!(app.status_ticks, 2);
    }

    #[test]
    fn test_ensure_scroll_visible_no_registry() {
        let mut app = App::new();
        app.cursor = 5;
        app.scroll = 10;
        // With no registry, available_count is 0 so cursor stays unchanged
        // but scroll is adjusted if cursor is above/below visible range
        app.ensure_scroll_visible();
        assert_eq!(app.cursor, 5);
    }
}
