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

    /// Called every Tick. Auto-dismisses the status after ~2 seconds.
    pub fn tick_status(&mut self) {
        if !self.status.is_empty() {
            self.status_ticks += 1;
            if self.status_ticks > 120 {
                self.status.clear();
            }
        }
    }
}
