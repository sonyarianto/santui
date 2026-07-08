use santui_ipc::protocol::{HostMsg, IpcKey, PluginMsg, PluginRequest};
use santui_registry::{plugin_filename, PluginManifest};

use super::state::{Action, App, DownloadEvent};

use std::sync::mpsc;

impl App {
    pub fn handle(&mut self, msg: HostMsg) -> PluginMsg {
        let mut request = None;
        let mut consumed = false;

        match msg {
            HostMsg::Init {
                theme,
                area,
                data_dir,
            } => {
                self.theme = theme;
                self.area = area;
                let data = std::path::PathBuf::from(&data_dir);
                self.plugins_dir = data.join("plugins");
                let reg = santui_registry::Registry::new(data);
                self.registry = Some(reg);
                self.set_status("Fetching plugins…".to_string());
                if let Some(ref mut reg) = self.registry {
                    let dev = std::env::var("SANTUI_DEV").as_deref() == Ok("1");
                    if dev {
                        reg.set_dev_mode(true);
                        let path = std::env::var("SANTUI_DEV_MANIFEST")
                            .map(std::path::PathBuf::from)
                            .unwrap_or_else(|_| std::path::PathBuf::from("plugins.json"));
                        match reg.load_local_manifest(&path) {
                            Ok(()) => {
                                let s = reg.status.clone();
                                self.set_status(s);
                            }
                            Err(e) => self.set_status(format!("Error: {e}")),
                        }
                    } else {
                        match reg.fetch_manifest() {
                            Ok(()) => {
                                let s = reg.status.clone();
                                self.set_status(s);
                            }
                            Err(e) => self.set_status(format!("Error: {e}")),
                        }
                    }
                }
                self.apply_filter();
            }

            HostMsg::Focus => {
                self.detail_idx = None;
                self.status.clear();
                self.status_ticks = 0;
            }
            HostMsg::Blur => {
                self.detail_idx = None;
                self.status.clear();
                self.status_ticks = 0;
            }

            HostMsg::Key { key, .. } => consumed = self.handle_key(key, &mut request),

            HostMsg::Tick => {
                self.tick = self.tick.wrapping_add(1);
                self.tick_status();
                let rx = self.download_rx.take();
                if let Some(rx) = rx {
                    let mut done = false;
                    let mut error = None;
                    let mut progress = None;

                    loop {
                        match rx.try_recv() {
                            Ok(DownloadEvent::Progress(d, t)) => {
                                progress = Some((d, t));
                            }
                            Ok(DownloadEvent::Done) => {
                                done = true;
                                break;
                            }
                            Ok(DownloadEvent::Error(e)) => {
                                done = true;
                                error = Some(e);
                                break;
                            }
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                done = true;
                                error = Some("Download thread terminated unexpectedly".into());
                                break;
                            }
                        }
                    }

                    if let Some(p) = progress {
                        self.download_progress = Some(p);
                    }

                    if done {
                        self.download_rx = None;
                        self.download_progress = None;
                        if let Some(e) = error {
                            self.set_status(format!("Error: {e}"));
                            self.pending_install_id = None;
                            self.pending_install_name = None;
                            self.pending_install_version = None;
                        } else if let Some(ref mut reg) = self.registry {
                            if let (Some(id), Some(name), Some(version)) = (
                                self.pending_install_id.take(),
                                self.pending_install_name.take(),
                                self.pending_install_version.take(),
                            ) {
                                let caps = std::mem::take(&mut self.pending_install_capabilities);
                                let target_path = self.plugins_dir.join(plugin_filename(&id));
                                match reg.add_installed(&id, &name, &version, target_path, &caps) {
                                    Ok(()) => {
                                        self.set_status(format!("{name} installed and enabled"));
                                        request = Some(PluginRequest::PluginsChanged);
                                    }
                                    Err(e) => self.set_status(format!("Error: {e}")),
                                }
                            }
                        }
                    } else {
                        self.download_rx = Some(rx);
                    }
                }
            }

            HostMsg::ThemeChange { theme } => {
                self.theme = theme;
            }

            HostMsg::Resize { area } => {
                self.area = area;
            }

            HostMsg::Shutdown => {
                return PluginMsg {
                    commands: vec![],
                    hints: vec![],
                    palette_commands: vec![],
                    request: None,
                    plugin_message: None,
                    consumed: false,
                }
            }

            HostMsg::UserUpdate { .. } => {}

            HostMsg::PaletteCommand { index: _ } => {}

            HostMsg::PluginMessage { .. } => {}
            HostMsg::DbValue { .. } => {}

            HostMsg::Mouse { .. } => {}
        }

        let commands = self.render_commands();
        let hints = self.hints();
        let palette_commands = vec![];

        PluginMsg {
            commands,
            hints,
            palette_commands,
            request,
            plugin_message: None,
            consumed,
        }
    }

    fn handle_key(&mut self, key: IpcKey, request: &mut Option<PluginRequest>) -> bool {
        self.status.clear();
        if let Some(detail_idx) = self.detail_idx {
            self.handle_detail_key(key, detail_idx, request)
        } else if self.search_mode {
            self.handle_search_key(key, request)
        } else {
            self.handle_list_key(key, request)
        }
    }

    fn handle_list_key(&mut self, key: IpcKey, _request: &mut Option<PluginRequest>) -> bool {
        match key {
            IpcKey::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.ensure_scroll_visible();
                }
                true
            }
            IpcKey::Down => {
                let count = self.available_count().saturating_sub(1);
                if self.cursor < count {
                    self.cursor += 1;
                    self.ensure_scroll_visible();
                }
                true
            }
            IpcKey::PageUp => {
                let page = super::render::max_list_h(self.area.h) as usize;
                self.cursor = self.cursor.saturating_sub(page);
                self.ensure_scroll_visible();
                true
            }
            IpcKey::PageDown => {
                let page = super::render::max_list_h(self.area.h) as usize;
                let max = self.available_count().saturating_sub(1);
                self.cursor = (self.cursor + page).min(max);
                self.ensure_scroll_visible();
                true
            }
            IpcKey::Enter | IpcKey::Char('d') | IpcKey::Char('D') => {
                let count = self.available_count();
                if self.cursor < count {
                    self.detail_idx = Some(self.filtered[self.cursor]);
                    self.action_cursor = 0;
                }
                true
            }
            IpcKey::Char('/') => {
                self.search_mode = true;
                self.query.clear();
                self.apply_filter();
                true
            }
            IpcKey::Char('c') if !self.query.is_empty() => {
                self.query.clear();
                self.apply_filter();
                true
            }
            IpcKey::Esc | IpcKey::Char('q') => false,
            _ => false,
        }
    }

    fn handle_search_key(&mut self, key: IpcKey, _request: &mut Option<PluginRequest>) -> bool {
        match key {
            IpcKey::Esc => {
                self.search_mode = false;
                self.query.clear();
                self.apply_filter();
                true
            }
            IpcKey::Enter => {
                let count = self.available_count();
                if self.cursor < count {
                    self.detail_idx = Some(self.filtered[self.cursor]);
                    self.action_cursor = 0;
                }
                self.search_mode = false;
                true
            }
            IpcKey::Backspace => {
                self.query.pop();
                self.apply_filter();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.query.push(c);
                self.apply_filter();
                true
            }
            IpcKey::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.ensure_scroll_visible();
                }
                true
            }
            IpcKey::Down => {
                let count = self.available_count().saturating_sub(1);
                if self.cursor < count {
                    self.cursor += 1;
                    self.ensure_scroll_visible();
                }
                true
            }
            _ => false,
        }
    }

    fn handle_detail_key(
        &mut self,
        key: IpcKey,
        detail_idx: usize,
        request: &mut Option<PluginRequest>,
    ) -> bool {
        let actions = self.available_actions(detail_idx);
        match key {
            IpcKey::Up => {
                if self.action_cursor > 0 {
                    self.action_cursor -= 1;
                }
                true
            }
            IpcKey::Down => {
                let last = actions.len().saturating_sub(1);
                if self.action_cursor < last {
                    self.action_cursor += 1;
                }
                true
            }
            IpcKey::Enter => {
                if self.action_cursor < actions.len() {
                    self.execute_action(detail_idx, actions[self.action_cursor], request);
                }
                true
            }
            IpcKey::Esc | IpcKey::Backspace => {
                self.detail_idx = None;
                true
            }
            _ => false,
        }
    }

    pub(super) fn available_actions(&self, idx: usize) -> Vec<Action> {
        let reg = match &self.registry {
            Some(r) => r,
            None => return vec![],
        };
        let plugin = match reg.available.get(idx) {
            Some(p) => p,
            None => return vec![],
        };
        let installed_idx = reg.installed.iter().position(|p| {
            p.path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.trim_end_matches(".exe"))
                == Some(&plugin.id)
        });
        let mut actions = Vec::new();
        match installed_idx {
            Some(i) => {
                if reg.installed[i].enabled {
                    actions.push(Action::Launch);
                    actions.push(Action::Disable);
                } else {
                    actions.push(Action::Enable);
                }
                if reg.installed[i].version != plugin.version {
                    actions.push(Action::Update);
                }
                actions.push(Action::Delete);
            }
            None => {
                actions.push(Action::Install);
            }
        }
        actions
    }

    fn execute_action(&mut self, idx: usize, action: Action, request: &mut Option<PluginRequest>) {
        let reg = match self.registry.as_mut() {
            Some(r) => r,
            None => return,
        };
        let plugin = match reg.available.get(idx) {
            Some(p) => p.clone(),
            None => return,
        };
        let installed_idx = reg.installed.iter().position(|p| {
            p.path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.trim_end_matches(".exe"))
                == Some(&plugin.id)
        });

        match action {
            Action::Enable => {
                if let Some(i) = installed_idx {
                    if reg.set_enabled(i, true).is_ok() {
                        self.set_status(format!("{} enabled", plugin.name));
                        *request = Some(PluginRequest::PluginsChanged);
                    }
                }
                self.detail_idx = None;
            }
            Action::Disable => {
                if let Some(i) = installed_idx {
                    if reg.set_enabled(i, false).is_ok() {
                        self.set_status(format!("{} disabled", plugin.name));
                        *request = Some(PluginRequest::PluginsChanged);
                    }
                }
                self.detail_idx = None;
            }
            Action::Install | Action::Update => {
                self.spawn_install(&plugin);
                self.detail_idx = None;
            }
            Action::Delete => {
                if let Some(i) = installed_idx {
                    if reg.remove_installed(i).is_ok() {
                        self.set_status(format!("{} deleted", plugin.name));
                        *request = Some(PluginRequest::PluginsChanged);
                    }
                }
                self.detail_idx = None;
            }
            Action::Launch => {
                *request = Some(PluginRequest::LaunchPlugin {
                    id: plugin.id.clone(),
                    name: plugin.name.clone(),
                });
                self.detail_idx = None;
            }
        }
    }

    fn spawn_install(&mut self, plugin: &PluginManifest) {
        if self.download_rx.is_some() {
            self.set_status("Already downloading…".to_string());
            return;
        }

        let id = plugin.id.clone();
        let name = plugin.name.clone();
        let version = plugin.version.clone();
        let url = plugin.download_url.clone();
        let sha256 = plugin.sha256.clone();
        let dest = self.plugins_dir.join(plugin_filename(&id));
        let dev_mode = self.registry.as_ref().map(|r| r.dev_mode).unwrap_or(false);

        let (tx, rx) = mpsc::channel();
        self.download_rx = Some(rx);
        self.pending_install_id = Some(id.clone());
        self.pending_install_name = Some(name.clone());
        self.pending_install_version = Some(version.clone());
        self.pending_install_capabilities = plugin.capabilities.clone();
        self.download_progress = Some((0, 0));
        self.set_status(format!("Downloading {name}…"));

        std::thread::spawn(move || {
            if let Some(parent) = dest.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    log::warn!("failed to create plugin download directory: {e}");
                }
            }
            let result = if dev_mode {
                let src = std::path::Path::new(&url);
                std::fs::copy(src, &dest).map(|_| ()).map_err(|e| {
                    format!("Failed to copy plugin binary from {}: {e}", src.display())
                })
            } else {
                santui_registry::download_plugin(&url, &sha256, &dest, &|downloaded, total| {
                    let _ = tx.send(DownloadEvent::Progress(downloaded, total));
                })
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(DownloadEvent::Done);
                }
                Err(e) => {
                    let _ = tx.send(DownloadEvent::Error(e));
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, PluginRequest, ThemeData};
    use santui_registry::{InstalledPlugin, PluginManifest};
    use std::sync::mpsc;

    struct TestHarness {
        app: App,
        _dir: std::path::PathBuf,
    }

    impl TestHarness {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("santui-reg-test-{name}"));
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::create_dir_all(&dir);
            let r = santui_registry::Registry::new(dir.clone());
            let mut app = App::new();
            app.registry = Some(r);
            app.theme = ThemeData {
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
            };
            app.area = Area { w: 80, h: 24 };
            TestHarness { app, _dir: dir }
        }

        fn with_available(&mut self, count: usize) {
            if let Some(ref mut r) = self.app.registry {
                for i in 0..count {
                    r.available.push(PluginManifest {
                        id: format!("p{i}"),
                        name: format!("Plugin {i}"),
                        description: format!("Description {i}"),
                        version: "1.0".into(),
                        download_url: "http://example.com/pkg".into(),
                        sha256: "abc".into(),
                        size: 100,
                        publisher: "Publisher".into(),
                        capabilities: vec![],
                    });
                }
            }
            self.app.apply_filter();
        }

        fn key(&self, key: IpcKey) -> HostMsg {
            HostMsg::Key {
                key,
                modifiers: IpcKeyModifiers::default(),
            }
        }
    }

    fn key_msg(key: IpcKey) -> HostMsg {
        HostMsg::Key {
            key,
            modifiers: IpcKeyModifiers::default(),
        }
    }

    #[test]
    fn test_handle_focus_clears_detail_and_status() {
        let mut app = App::new();
        app.detail_idx = Some(0);
        app.status = "hello".into();
        app.status_ticks = 5;
        app.handle(HostMsg::Focus);
        assert!(app.detail_idx.is_none());
        assert!(app.status.is_empty());
        assert_eq!(app.status_ticks, 0);
    }

    #[test]
    fn test_handle_blur_clears_detail_and_status() {
        let mut app = App::new();
        app.detail_idx = Some(0);
        app.status = "hello".into();
        app.handle(HostMsg::Blur);
        assert!(app.detail_idx.is_none());
        assert!(app.status.is_empty());
    }

    #[test]
    fn test_handle_theme_change() {
        let mut app = App::new();
        let new_theme = ThemeData {
            text: [100; 3],
            text_muted: [101; 3],
            accent: [102; 3],
            highlight: [103; 3],
            logo: [104; 3],
            background: [105; 3],
            background_panel: [106; 3],
            background_overlay: [107; 3],
            border: [108; 3],
            success: [109; 3],
            error: [110; 3],
            inverted_text: [111; 3],
        };
        app.handle(HostMsg::ThemeChange {
            theme: new_theme.clone(),
        });
        assert_eq!(app.theme.text, new_theme.text);
        assert_eq!(app.theme.accent, new_theme.accent);
    }

    #[test]
    fn test_handle_resize() {
        let mut app = App::new();
        app.handle(HostMsg::Resize {
            area: Area { w: 120, h: 40 },
        });
        assert_eq!(app.area.w, 120);
        assert_eq!(app.area.h, 40);
    }

    #[test]
    fn test_handle_shutdown_returns_immediately() {
        let mut app = App::new();
        let resp = app.handle(HostMsg::Shutdown);
        assert!(!resp.consumed);
        assert!(resp.commands.is_empty());
        assert!(resp.hints.is_empty());
        assert!(resp.request.is_none());
    }

    #[test]
    fn test_handle_key_up_moves_cursor() {
        let mut h = TestHarness::new("up_moves");
        h.with_available(5);
        h.app.cursor = 3;
        let resp = h.app.handle(h.key(IpcKey::Up));
        assert_eq!(h.app.cursor, 2);
        assert!(resp.consumed);
    }

    #[test]
    fn test_handle_key_up_at_top_stays() {
        let mut h = TestHarness::new("up_top");
        h.with_available(5);
        h.app.cursor = 0;
        h.app.handle(h.key(IpcKey::Up));
        assert_eq!(h.app.cursor, 0);
    }

    #[test]
    fn test_handle_key_down_moves_cursor() {
        let mut h = TestHarness::new("down_moves");
        h.with_available(5);
        h.app.cursor = 0;
        h.app.handle(h.key(IpcKey::Down));
        assert_eq!(h.app.cursor, 1);
    }

    #[test]
    fn test_handle_key_down_at_bottom_stays() {
        let mut h = TestHarness::new("down_bottom");
        h.with_available(5);
        h.app.cursor = 4;
        h.app.handle(h.key(IpcKey::Down));
        assert_eq!(h.app.cursor, 4);
    }

    #[test]
    fn test_handle_key_enter_opens_detail() {
        let mut h = TestHarness::new("enter_detail");
        h.with_available(5);
        h.app.cursor = 2;
        h.app.handle(h.key(IpcKey::Enter));
        assert_eq!(h.app.detail_idx, Some(2));
        assert_eq!(h.app.action_cursor, 0);
    }

    #[test]
    fn test_handle_key_enter_on_empty_list_does_nothing() {
        let mut app = App::new();
        app.handle(key_msg(IpcKey::Enter));
        assert!(app.detail_idx.is_none());
    }

    #[test]
    fn test_handle_key_esc_in_list_not_consumed() {
        let mut app = App::new();
        let resp = app.handle(key_msg(IpcKey::Esc));
        assert!(!resp.consumed);
    }

    #[test]
    fn test_handle_key_q_in_list_not_consumed() {
        let mut app = App::new();
        let resp = app.handle(key_msg(IpcKey::Char('q')));
        assert!(!resp.consumed);
    }

    #[test]
    fn test_handle_key_d_opens_detail() {
        let mut h = TestHarness::new("d_detail");
        h.with_available(3);
        h.app.cursor = 1;
        h.app.handle(h.key(IpcKey::Char('d')));
        assert_eq!(h.app.detail_idx, Some(1));
    }

    #[test]
    fn test_handle_key_unrecognized_in_list_not_consumed() {
        let mut h = TestHarness::new("unrec");
        h.with_available(3);
        let resp = h.app.handle(h.key(IpcKey::Char('x')));
        assert!(!resp.consumed);
    }

    #[test]
    fn test_handle_key_esc_in_detail_goes_back() {
        let mut h = TestHarness::new("esc_detail");
        h.with_available(3);
        h.app.cursor = 1;
        h.app.handle(h.key(IpcKey::Enter));
        assert_eq!(h.app.detail_idx, Some(1));
        h.app.handle(h.key(IpcKey::Esc));
        assert!(h.app.detail_idx.is_none());
    }

    #[test]
    fn test_handle_key_backspace_in_detail_goes_back() {
        let mut h = TestHarness::new("bs_detail");
        h.with_available(3);
        h.app.cursor = 1;
        h.app.handle(h.key(IpcKey::Enter));
        assert_eq!(h.app.detail_idx, Some(1));
        h.app.handle(h.key(IpcKey::Backspace));
        assert!(h.app.detail_idx.is_none());
    }

    #[test]
    fn test_handle_key_up_in_detail_moves_action_cursor() {
        let mut h = TestHarness::new("up_action");
        h.with_available(3);
        h.app.cursor = 0;
        h.app.handle(h.key(IpcKey::Enter));
        h.app.action_cursor = 2;
        h.app.handle(h.key(IpcKey::Up));
        assert_eq!(h.app.action_cursor, 1);
    }

    #[test]
    fn test_handle_key_up_in_detail_at_top_stays() {
        let mut h = TestHarness::new("up_action_top");
        h.with_available(3);
        h.app.cursor = 0;
        h.app.handle(h.key(IpcKey::Enter));
        h.app.action_cursor = 0;
        h.app.handle(h.key(IpcKey::Up));
        assert_eq!(h.app.action_cursor, 0);
    }

    #[test]
    fn test_handle_key_down_in_detail_moves_action_cursor() {
        let mut h = TestHarness::new("down_action");
        h.with_available(1);
        if let Some(ref mut r) = h.app.registry {
            r.installed.push(InstalledPlugin {
                enabled: true,
                version: "1.0".into(),
                path: std::path::PathBuf::from("p0.exe"),
                id: "p0".into(),
                name: "Plugin 0".into(),
                capabilities: vec![],
            });
        }
        // Now available_actions returns [Launch, Disable, Delete] = 3 actions
        h.app.cursor = 0;
        h.app.handle(h.key(IpcKey::Enter));
        assert_eq!(h.app.action_cursor, 0);
        h.app.handle(h.key(IpcKey::Down));
        assert_eq!(h.app.action_cursor, 1);
    }

    #[test]
    fn test_handle_key_enter_in_detail_triggers_install() {
        let mut h = TestHarness::new("enter_install");
        h.with_available(3);
        h.app.cursor = 0;
        h.app.handle(h.key(IpcKey::Enter));
        assert_eq!(h.app.detail_idx, Some(0));
        h.app.handle(h.key(IpcKey::Enter));
        assert!(h.app.detail_idx.is_none());
        assert!(h.app.download_rx.is_some());
    }

    #[test]
    fn test_handle_tick_calls_tick_status() {
        let mut app = App::new();
        app.set_status("test".into());
        app.handle(HostMsg::Tick);
        assert_eq!(app.status_ticks, 1);
    }

    #[test]
    fn test_handle_tick_processes_download_progress() {
        let mut h = TestHarness::new("tick_progress");
        let (tx, rx) = mpsc::channel();
        tx.send(DownloadEvent::Progress(50, 100)).unwrap();
        h.app.download_rx = Some(rx);
        h.app.handle(HostMsg::Tick);
        assert_eq!(h.app.download_progress, Some((50, 100)));
    }

    #[test]
    fn test_handle_tick_processes_download_error() {
        let mut h = TestHarness::new("tick_error");
        let (tx, rx) = mpsc::channel();
        tx.send(DownloadEvent::Error("fail".into())).unwrap();
        h.app.download_rx = Some(rx);
        h.app.pending_install_id = Some("p0".into());
        h.app.handle(HostMsg::Tick);
        assert!(h.app.download_rx.is_none());
        assert!(h.app.status.contains("Error"));
    }

    #[test]
    fn test_handle_tick_disconnected_channel() {
        let mut h = TestHarness::new("tick_disc");
        let (tx, rx) = mpsc::channel::<DownloadEvent>();
        drop(tx);
        h.app.download_rx = Some(rx);
        h.app.handle(HostMsg::Tick);
        assert!(h.app.download_rx.is_none());
        assert!(h.app.status.contains("Error"));
    }

    #[test]
    fn test_handle_tick_empty_channel_preserved() {
        let mut h = TestHarness::new("tick_empty");
        let (tx, rx) = mpsc::channel::<DownloadEvent>();
        h.app.download_rx = Some(rx);
        h.app.handle(HostMsg::Tick);
        assert!(h.app.download_rx.is_some());
        drop(tx);
    }

    #[test]
    fn test_available_actions_no_registry() {
        let app = App::new();
        assert!(app.available_actions(0).is_empty());
    }

    #[test]
    fn test_available_actions_out_of_bounds() {
        let mut h = TestHarness::new("avail_oob");
        assert!(h.app.available_actions(0).is_empty());
        h.with_available(1);
        assert!(h.app.available_actions(5).is_empty());
    }

    #[test]
    fn test_available_actions_not_installed() {
        let mut h = TestHarness::new("avail_ni");
        h.with_available(1);
        assert_eq!(h.app.available_actions(0), vec![Action::Install]);
    }

    #[test]
    fn test_available_actions_installed_enabled() {
        let mut h = TestHarness::new("avail_en");
        h.with_available(1);
        if let Some(ref mut r) = h.app.registry {
            r.installed.push(InstalledPlugin {
                enabled: true,
                version: "1.0".into(),
                path: std::path::PathBuf::from("p0.exe"),
                id: "p0".into(),
                name: "Plugin 0".into(),
                capabilities: vec![],
            });
        }
        let actions = h.app.available_actions(0);
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0], Action::Launch);
        assert_eq!(actions[1], Action::Disable);
        assert_eq!(actions[2], Action::Delete);
    }

    #[test]
    fn test_available_actions_installed_disabled() {
        let mut h = TestHarness::new("avail_dis");
        h.with_available(1);
        if let Some(ref mut r) = h.app.registry {
            r.installed.push(InstalledPlugin {
                enabled: false,
                version: "1.0".into(),
                path: std::path::PathBuf::from("p0.exe"),
                id: "p0".into(),
                name: "Plugin 0".into(),
                capabilities: vec![],
            });
        }
        let actions = h.app.available_actions(0);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0], Action::Enable);
        assert_eq!(actions[1], Action::Delete);
    }

    #[test]
    fn test_available_actions_update_available() {
        let mut h = TestHarness::new("avail_upd");
        h.with_available(1);
        if let Some(ref mut r) = h.app.registry {
            r.installed.push(InstalledPlugin {
                enabled: true,
                version: "0.9".into(),
                path: std::path::PathBuf::from("p0.exe"),
                id: "p0".into(),
                name: "Plugin 0".into(),
                capabilities: vec![],
            });
        }
        let actions = h.app.available_actions(0);
        assert_eq!(actions.len(), 4);
        assert_eq!(actions[0], Action::Launch);
        assert_eq!(actions[1], Action::Disable);
        assert_eq!(actions[2], Action::Update);
        assert_eq!(actions[3], Action::Delete);
    }

    #[test]
    fn test_consumed_true_on_handled_key() {
        let mut h = TestHarness::new("cons_true");
        h.with_available(3);
        let resp = h.app.handle(h.key(IpcKey::Up));
        assert!(resp.consumed);
    }

    #[test]
    fn test_consumed_false_on_unhandled_key() {
        let mut h = TestHarness::new("cons_false");
        h.with_available(3);
        let resp = h.app.handle(h.key(IpcKey::Char('z')));
        assert!(!resp.consumed);
    }

    #[test]
    fn test_consumed_false_on_focus() {
        let mut app = App::new();
        let resp = app.handle(HostMsg::Focus);
        assert!(!resp.consumed);
    }

    #[test]
    fn test_enable_triggers_plugins_changed() {
        let mut h = TestHarness::new("enable_chg");
        h.with_available(1);
        if let Some(ref mut r) = h.app.registry {
            r.installed.push(InstalledPlugin {
                enabled: false,
                version: "1.0".into(),
                path: std::path::PathBuf::from("p0.exe"),
                id: "p0".into(),
                name: "Plugin 0".into(),
                capabilities: vec![],
            });
        }
        h.app.cursor = 0;
        h.app.handle(h.key(IpcKey::Enter));
        let resp = h.app.handle(h.key(IpcKey::Enter));
        assert!(matches!(resp.request, Some(PluginRequest::PluginsChanged)));
    }
}
