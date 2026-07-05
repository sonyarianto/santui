mod m3u;
mod player;
mod state;
mod ui;

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::thread;

use player::Mpv;
use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData, UserData};
use state::{IptvState, PlaybackState, Screen};

const LIST_OVERHEAD: u16 = ui::TABLE_TOP + ui::HEADER_H + 1 + 2;

enum MpvMsg {
    FileLoaded,
    EndFile(u32),
}

enum MpvCmd {
    LoadUrl(String),
    Stop,
    SetVolume(i64),
    SetPause(bool),
    Quit,
}

struct App {
    state: IptvState,
    theme: ThemeData,
    area: Area,
    tx_cmd: Option<mpsc::Sender<MpvCmd>>,
    rx_msg: Option<mpsc::Receiver<MpvMsg>>,
    mpv_thread: Option<thread::JoinHandle<()>>,
    init_error: Option<String>,
    mpv_warnings: Vec<String>,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    user: Option<UserData>,
    pending_request: Option<PluginRequest>,
}

fn send_cmd(app: &App, cmd: MpvCmd) {
    if let Some(ref tx) = app.tx_cmd {
        let _ = tx.send(cmd);
    }
}

impl App {
    fn new() -> Self {
        App {
            state: IptvState::new(),
            theme: ThemeData {
                text: [220, 220, 220],
                text_muted: [140, 140, 140],
                accent: [157, 124, 216],
                highlight: [250, 178, 131],
                logo: [255, 185, 0],
                background: [20, 20, 20],
                background_panel: [20, 20, 20],
                background_overlay: [10, 10, 10],
                border: [250, 178, 131],
                success: [127, 216, 143],
                error: [224, 108, 117],
                inverted_text: [20, 20, 20],
            },
            area: Area { w: 80, h: 24 },
            tx_cmd: None,
            rx_msg: None,
            mpv_thread: None,
            init_error: None,
            mpv_warnings: Vec::new(),
            dirty: true,
            cached_commands: Vec::new(),
            user: None,
            pending_request: Some(PluginRequest::DbGet {
                key: "favorites".into(),
            }),
        }
    }

    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.dirty = true;

        let (mut mpv, warns) = match Mpv::new() {
            Ok(v) => v,
            Err(e) => {
                self.init_error = Some(format!("{e}"));
                return;
            }
        };

        self.mpv_warnings = warns.clone();
        for w in &warns {
            log::warn!("  {w}");
        }

        if let Err(e) = mpv.observe_property(0, "volume") {
            log::warn!("mpv observe_property(volume) failed: {e}");
        }
        if let Err(e) = mpv.set_volume(self.state.volume) {
            log::warn!("mpv set_volume failed: {e}");
        }

        let (tx_msg, rx_msg) = mpsc::channel::<MpvMsg>();
        let (tx_cmd, rx_cmd) = mpsc::channel::<MpvCmd>();

        let tx_msg_mpv = tx_msg;

        let handle = thread::spawn(move || {
            loop {
                let ev = mpv.wait_event_raw(0.1);
                if let Some(ev) = ev {
                    let id = ev.event_id;
                    if id == player::MPV_EVENT_SHUTDOWN {
                        break;
                    }
                    if id == player::MPV_EVENT_FILE_LOADED {
                        let _ = tx_msg_mpv.send(MpvMsg::FileLoaded);
                    }
                    if id == player::MPV_EVENT_END_FILE {
                        if ev.data.is_null() {
                            continue;
                        }
                        let ef: &player::MpvEventEndFile = unsafe { &*(ev.data as *const _) };
                        let _ = tx_msg_mpv.send(MpvMsg::EndFile(ef.reason));
                    }
                }

                while let Ok(cmd) = rx_cmd.try_recv() {
                    match cmd {
                        MpvCmd::LoadUrl(url) => {
                            if let Err(e) = mpv.load_url(&url) {
                                log::warn!("mpv load_url failed: {e}");
                            }
                        }
                        MpvCmd::Stop => {
                            if let Err(e) = mpv.stop() {
                                log::warn!("mpv stop failed: {e}");
                            }
                        }
                        MpvCmd::SetVolume(v) => {
                            if let Err(e) = mpv.set_volume(v) {
                                log::warn!("mpv set_volume failed: {e}");
                            }
                        }
                        MpvCmd::SetPause(pause) => {
                            if let Err(e) = mpv.set_pause(pause) {
                                log::warn!("mpv set_pause failed: {e}");
                            }
                        }
                        MpvCmd::Quit => {
                            mpv.destroy();
                            return;
                        }
                    }
                }
            }
            mpv.destroy();
        });
        self.mpv_thread = Some(handle);
        self.tx_cmd = Some(tx_cmd);
        self.rx_msg = Some(rx_msg);
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.state.screen = Screen::Search;
                self.state.query.clear();
                self.state.apply_filter();
                self.state.selected = 0;
                self.state.scroll = 0;
                self.dirty = true;
            }
            1 => {
                self.do_fetch_playlist();
                self.dirty = true;
            }
            2 => {
                self.state.screen = Screen::PlaylistUrlEditor;
                self.state.url_edit = self.state.playlist_url.clone();
                self.state.url_edit_cursor = self.state.url_edit.chars().count();
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn do_fetch_playlist(&mut self) {
        let url = self.state.playlist_url.clone();
        self.state
            .set_scan_msg(format!("Fetching playlist from {}...", url));

        match ureq::get(&url)
            .header("User-Agent", "santui-iptv-player/1.0")
            .call()
        {
            Ok(mut resp) => match resp.body_mut().read_to_string() {
                Ok(body) => {
                    let channels = m3u::parse(&body);
                    let count = channels.len();
                    self.state.channels = channels;
                    self.state.apply_filter();
                    self.state.selected = 0;
                    self.state.scroll = 0;
                    self.state.group_filter = None;
                    self.state.set_scan_msg(format!("Loaded {count} channels"));
                    let cache_json =
                        serde_json::to_string(&self.state.channels).unwrap_or_default();
                    self.pending_request = Some(PluginRequest::DbSet {
                        key: "playlist-cache".into(),
                        value: cache_json,
                    });
                }
                Err(e) => {
                    self.state
                        .set_scan_msg(format!("Failed to parse response: {e}"));
                }
            },
            Err(e) => {
                self.state
                    .set_scan_msg(format!("Failed to fetch playlist: {e}"));
            }
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        match key {
            "favorites" => {
                let favs: std::collections::HashSet<String> = match value {
                    Some(json) => serde_json::from_str(&json).unwrap_or_default(),
                    None => std::collections::HashSet::new(),
                };
                self.state.set_favorites(favs);
                self.dirty = true;
            }
            "playlist-cache" => {
                if let Some(json) = value {
                    if self.state.channels.is_empty() {
                        if let Ok(channels) =
                            serde_json::from_str::<Vec<crate::m3u::Channel>>(&json)
                        {
                            let count = channels.len();
                            self.state.channels = channels;
                            self.state.apply_filter();
                            if count > 0 {
                                self.state
                                    .set_scan_msg(format!("Loaded {count} channels from cache"));
                            }
                            self.dirty = true;
                        }
                    }
                }
            }
            "playlist-url" => {
                if let Some(url) = value {
                    if !url.is_empty() {
                        self.state.playlist_url = url;
                    }
                }
            }
            "preferences" => {
                if let Some(json) = value {
                    if let Ok(prefs) = serde_json::from_str::<serde_json::Value>(&json) {
                        if let Some(vol) = prefs.get("volume").and_then(|v| v.as_i64()) {
                            self.state.volume = vol.clamp(0, 100);
                            send_cmd(self, MpvCmd::SetVolume(self.state.volume));
                        }
                        if let Some(url) = prefs.get("playlist_url").and_then(|v| v.as_str()) {
                            if !url.is_empty() {
                                self.state.playlist_url = url.to_string();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.state.scan_msg = None;
        self.dirty = true;

        match self.state.screen {
            Screen::ChannelList | Screen::GroupFilter => self.handle_key_channel_list(key),
            Screen::Search => self.handle_key_search(key),
            Screen::PlaylistUrlEditor => self.handle_key_url_editor(key),
        }
    }

    fn handle_key_channel_list(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char('j') | IpcKey::Down => {
                self.state.select_next();
                let info_h = self.state.info_h();
                let max_visible = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                self.state.ensure_scroll_visible(max_visible.max(1));
                true
            }
            IpcKey::Char('k') | IpcKey::Up => {
                self.state.select_prev();
                let info_h = self.state.info_h();
                let max_visible = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                self.state.ensure_scroll_visible(max_visible.max(1));
                true
            }
            IpcKey::PageDown => {
                let info_h = self.state.info_h();
                let page = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                self.state.select_page_down(page.max(1));
                self.state.ensure_scroll_visible(page.max(1));
                true
            }
            IpcKey::PageUp => {
                let info_h = self.state.info_h();
                let page = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                self.state.select_page_up(page.max(1));
                self.state.ensure_scroll_visible(page.max(1));
                true
            }
            IpcKey::Enter => {
                if let Some(ch) = self.state.selected_channel().cloned() {
                    let idx = self.state.current_filtered_index();
                    self.state.play_state = PlaybackState::Buffering { channel_index: idx };
                    send_cmd(self, MpvCmd::Stop);
                    send_cmd(self, MpvCmd::LoadUrl(ch.url.clone()));
                }
                true
            }
            IpcKey::Char(' ') => {
                match &self.state.play_state {
                    PlaybackState::Playing { .. } => {
                        self.state.play_state = match self.state.play_state {
                            PlaybackState::Playing { channel_index } => {
                                PlaybackState::Paused { channel_index }
                            }
                            _ => unreachable!(),
                        };
                        send_cmd(self, MpvCmd::SetPause(true));
                        return true;
                    }
                    PlaybackState::Paused { .. } => {
                        self.state.play_state = match self.state.play_state {
                            PlaybackState::Paused { channel_index } => {
                                PlaybackState::Playing { channel_index }
                            }
                            _ => unreachable!(),
                        };
                        send_cmd(self, MpvCmd::SetPause(false));
                        return true;
                    }
                    _ => {}
                }
                let url = self.state.selected_channel().map(|c| c.url.clone());
                if let Some(url) = url {
                    let is_fav = self.state.toggle_favorite(&url);
                    let favs_json =
                        serde_json::to_string(&self.state.favorites.iter().collect::<Vec<_>>())
                            .unwrap_or_default();
                    self.pending_request = Some(PluginRequest::DbSet {
                        key: "favorites".into(),
                        value: favs_json,
                    });
                    self.state.set_scan_msg(if is_fav {
                        "\u{2665} Added to favorites".into()
                    } else {
                        "Removed from favorites".into()
                    });
                }
                true
            }
            IpcKey::Char('x') => {
                send_cmd(self, MpvCmd::Stop);
                self.state.play_state = PlaybackState::Stopped;
                true
            }
            IpcKey::Char('/') | IpcKey::Char('s') => {
                self.state.screen = Screen::Search;
                self.state.query.clear();
                self.state.apply_filter();
                self.state.selected = 0;
                self.state.scroll = 0;
                true
            }
            IpcKey::Char('g') => {
                if self.state.group_filter.is_some() {
                    self.state.group_filter = None;
                    self.state.apply_filter();
                    self.state.selected = 0;
                    self.state.scroll = 0;
                    self.state.screen = Screen::ChannelList;
                } else {
                    self.state.screen = Screen::GroupFilter;
                    // Show first group
                    let groups = self.state.group_titles();
                    if !groups.is_empty() {
                        self.state.group_filter = Some(groups[0].clone());
                        self.state.apply_filter();
                        self.state.selected = 0;
                        self.state.scroll = 0;
                    } else {
                        self.state.set_scan_msg("No groups found".into());
                    }
                }
                true
            }
            IpcKey::Char('f') => {
                self.state.show_favorites_only = !self.state.show_favorites_only;
                self.state.apply_filter();
                self.state.selected = 0;
                self.state.scroll = 0;
                if self.state.show_favorites_only {
                    self.state
                        .set_scan_msg("\u{2665} Showing favorites only".into());
                } else {
                    self.state.set_scan_msg("Showing all channels".into());
                }
                true
            }
            IpcKey::Char('u') => {
                self.state.screen = Screen::PlaylistUrlEditor;
                self.state.url_edit = self.state.playlist_url.clone();
                self.state.url_edit_cursor = self.state.url_edit.chars().count();
                true
            }
            IpcKey::Char('r') => {
                self.do_fetch_playlist();
                true
            }
            IpcKey::Char('+') | IpcKey::Char('=') => {
                self.state.volume_up();
                send_cmd(self, MpvCmd::SetVolume(self.state.volume));
                true
            }
            IpcKey::Char('-') => {
                self.state.volume_down();
                send_cmd(self, MpvCmd::SetVolume(self.state.volume));
                true
            }
            IpcKey::Esc => {
                if matches!(self.state.screen, Screen::GroupFilter) {
                    self.state.group_filter = None;
                    self.state.apply_filter();
                    self.state.selected = 0;
                    self.state.scroll = 0;
                    self.state.screen = Screen::ChannelList;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn handle_key_search(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Esc => {
                self.state.screen = Screen::ChannelList;
                self.state.set_query(String::new());
                true
            }
            IpcKey::Enter => {
                self.state.screen = Screen::ChannelList;
                if let Some(ch) = self.state.selected_channel().cloned() {
                    let idx = self.state.current_filtered_index();
                    self.state.play_state = PlaybackState::Buffering { channel_index: idx };
                    send_cmd(self, MpvCmd::Stop);
                    send_cmd(self, MpvCmd::LoadUrl(ch.url.clone()));
                }
                true
            }
            IpcKey::Backspace => {
                self.state.query.pop();
                self.state.apply_filter();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.state.query.push(c);
                self.state.apply_filter();
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.select_prev();
                let info_h = self.state.info_h();
                let max_visible = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                self.state.ensure_scroll_visible(max_visible.max(1));
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.state.select_next();
                let info_h = self.state.info_h();
                let max_visible = self.area.h.saturating_sub(info_h + LIST_OVERHEAD) as usize;
                self.state.ensure_scroll_visible(max_visible.max(1));
                true
            }
            _ => false,
        }
    }

    fn handle_key_url_editor(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Esc => {
                self.state.screen = Screen::ChannelList;
                true
            }
            IpcKey::Enter => {
                let url = self.state.url_edit.clone();
                if !url.is_empty() {
                    self.state.playlist_url = url;
                    let url_json =
                        serde_json::to_string(&self.state.playlist_url).unwrap_or_default();
                    self.pending_request = Some(PluginRequest::DbSet {
                        key: "playlist-url".into(),
                        value: url_json,
                    });
                    // Also save preferences
                    let prefs = serde_json::json!({
                        "playlist_url": self.state.playlist_url,
                        "volume": self.state.volume,
                    });
                    self.pending_request = Some(PluginRequest::DbSet {
                        key: "preferences".into(),
                        value: prefs.to_string(),
                    });
                    self.state.screen = Screen::ChannelList;
                    self.do_fetch_playlist();
                }
                true
            }
            IpcKey::Backspace => {
                if self.state.url_edit_cursor > 0 {
                    let pos = self.state.url_edit_cursor;
                    let before: String = self.state.url_edit.chars().take(pos - 1).collect();
                    let after: String = self.state.url_edit.chars().skip(pos).collect();
                    self.state.url_edit = format!("{before}{after}");
                    self.state.url_edit_cursor = pos - 1;
                }
                true
            }
            IpcKey::Delete => {
                let total = self.state.url_edit.chars().count();
                if self.state.url_edit_cursor < total {
                    let pos = self.state.url_edit_cursor;
                    let before: String = self.state.url_edit.chars().take(pos).collect();
                    let after: String = self.state.url_edit.chars().skip(pos + 1).collect();
                    self.state.url_edit = format!("{before}{after}");
                }
                true
            }
            IpcKey::Left => {
                if self.state.url_edit_cursor > 0 {
                    self.state.url_edit_cursor -= 1;
                }
                true
            }
            IpcKey::Right => {
                let total = self.state.url_edit.chars().count();
                if self.state.url_edit_cursor < total {
                    self.state.url_edit_cursor += 1;
                }
                true
            }
            IpcKey::Home => {
                self.state.url_edit_cursor = 0;
                true
            }
            IpcKey::End => {
                self.state.url_edit_cursor = self.state.url_edit.chars().count();
                true
            }
            IpcKey::Char('b') => {
                // Ctrl+B shortcut handled via Char('b') - reset to default URL
                self.state.url_edit = m3u::DEFAULT_PLAYLIST_URL.to_string();
                self.state.url_edit_cursor = self.state.url_edit.chars().count();
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                let pos = self.state.url_edit_cursor;
                let before: String = self.state.url_edit.chars().take(pos).collect();
                let after: String = self.state.url_edit.chars().skip(pos).collect();
                self.state.url_edit = format!("{before}{c}{after}");
                self.state.url_edit_cursor = pos + 1;
                true
            }
            _ => false,
        }
    }

    fn handle_tick(&mut self) {
        let mut changed = false;
        if let Some(ref rx) = self.rx_msg {
            while let Ok(msg) = rx.try_recv() {
                changed = true;
                match msg {
                    MpvMsg::FileLoaded => {
                        if let PlaybackState::Buffering { channel_index } = self.state.play_state {
                            self.state.play_state = PlaybackState::Playing { channel_index };
                        }
                    }
                    MpvMsg::EndFile(reason) => {
                        if reason == player::MPV_END_FILE_REASON_EOF {
                            let current_idx = match &self.state.play_state {
                                PlaybackState::Playing { channel_index }
                                | PlaybackState::Buffering { channel_index }
                                | PlaybackState::Paused { channel_index } => Some(*channel_index),
                                _ => None,
                            };
                            if let Some(idx) = current_idx {
                                if idx < self.state.channels.len() {
                                    let next_url = self.state.channels[idx].url.clone();
                                    send_cmd(self, MpvCmd::LoadUrl(next_url));
                                }
                            }
                        } else if reason == player::MPV_END_FILE_REASON_ERROR {
                            self.state.play_state =
                                PlaybackState::Error("Stream connection lost".into());
                        }
                    }
                }
            }
        }
        self.state.tick_counter += 1;
        if self.state.tick_scan_msg() {
            changed = true;
        }
        self.dirty = changed;
    }

    fn handle_shutdown(&mut self) {
        if let Some(ref tx) = self.tx_cmd {
            let _ = tx.send(MpvCmd::Quit);
        }
        if let Some(handle) = self.mpv_thread.take() {
            let _ = handle.join();
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match self.state.screen {
            Screen::Search => vec![
                ("esc".into(), "back".into()),
                ("enter".into(), "play".into()),
            ],
            Screen::PlaylistUrlEditor => vec![
                ("esc".into(), "cancel".into()),
                ("enter".into(), "save".into()),
            ],
            Screen::ChannelList | Screen::GroupFilter => {
                let mut hints = vec![
                    ("j/k".into(), "navigate".into()),
                    ("enter".into(), "play".into()),
                    ("space".into(), "fav".into()),
                    ("/".into(), "search".into()),
                    ("x".into(), "stop".into()),
                ];
                if !self.state.query.is_empty() {
                    hints.push(("c".into(), "clear".into()));
                }
                hints.push(("+/-".into(), "volume".into()));
                hints
            }
        }
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            if let Some(ref err) = self.init_error {
                if self.mpv_warnings.is_empty() {
                    self.cached_commands = vec![RenderCmd::Text {
                        x: 0,
                        y: 0,
                        text: format!("MPV: {err}"),
                        fg: Some(self.theme.error),
                        bg: None,
                        bold: false,
                        modifiers: 0,
                    }];
                } else {
                    let mut lines = vec![format!("{err}")];
                    for w in &self.mpv_warnings {
                        lines.push(format!("  {w}"));
                    }
                    lines.push("\u{25b6} Playlist browser is still available.".into());
                    lines.push(
                        "  Controls: j/k navigate, / search, g filter groups, u edit URL.".into(),
                    );
                    let joined = lines.join("\n");
                    self.cached_commands = vec![RenderCmd::Text {
                        x: 0,
                        y: 0,
                        text: joined,
                        fg: Some(self.theme.error),
                        bg: None,
                        bold: false,
                        modifiers: 0,
                    }];
                }
            } else {
                self.cached_commands =
                    ui::render_ui(&self.state, &self.theme, self.area.w, self.area.h);
            };
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn palette_commands() -> Vec<(String, String)> {
    vec![
        ("IPTV".into(), "Search channels".into()),
        ("IPTV".into(), "Refresh playlist".into()),
        ("IPTV".into(), "Edit playlist URL".into()),
    ]
}

fn respond(app: &mut App, consumed: bool) {
    let commands_val = match serde_json::to_value(app.render()) {
        Ok(v) => v,
        Err(e) => {
            log::error!("failed to serialize render commands: {e}");
            return;
        }
    };
    let hints = app.status_hints();
    let palette = palette_commands();
    let request = app.pending_request.take();
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": hints,
        "palette_commands": palette,
        "request": request,
        "consumed": consumed,
    });
    let Ok(json_str) = serde_json::to_string(&json) else {
        return;
    };
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json_str}");
    let _ = out.flush();
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();

    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        if let Ok(null) = std::fs::File::open("/dev/null") {
            unsafe {
                libc::dup2(null.as_raw_fd(), libc::STDERR_FILENO);
            }
        }
    }

    let mut reader = BufReader::new(std::io::stdin().lock());

    let mut app = App::new();
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: HostMsg = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("[iptv] parse error: {e}: {line}");
                        continue;
                    }
                };

                match msg {
                    HostMsg::Init {
                        theme,
                        area,
                        data_dir: _,
                    } => {
                        app.handle_init(theme, area);
                        // On first init, load cached channels
                        if app.state.channels.is_empty() {
                            let cache_key = "playlist-cache".to_string();
                            let request = Some(PluginRequest::DbGet { key: cache_key });
                            app.pending_request = request;
                        }
                        // Load preferences
                        let prefs_key = "preferences".to_string();
                        let prefs_request = Some(PluginRequest::DbGet { key: prefs_key });
                        // Queue prefs request (will be overwritten by playlist-cache request, so
                        // we just set it now and the next cycle will handle it)
                        if app.pending_request.is_none() {
                            app.pending_request = prefs_request;
                        }
                        respond(&mut app, false);
                    }
                    HostMsg::Key { key, .. } => {
                        let consumed = app.handle_key(key);
                        respond(&mut app, consumed);
                    }
                    HostMsg::Tick => {
                        app.handle_tick();
                        respond(&mut app, false);
                    }
                    HostMsg::Focus | HostMsg::Blur => {
                        respond(&mut app, false);
                    }
                    HostMsg::ThemeChange { theme } => {
                        app.theme = theme;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    HostMsg::Resize { area } => {
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    HostMsg::PaletteCommand { index } => {
                        app.handle_palette_command(index);
                        respond(&mut app, false);
                    }
                    HostMsg::PluginMessage { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::Mouse { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::UserUpdate { user } => {
                        app.user = user;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    HostMsg::DbValue { key, value } => {
                        app.handle_db_value(&key, value);
                        respond(&mut app, false);
                    }
                    HostMsg::Shutdown => {
                        app.handle_shutdown();
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Screen;

    fn default_theme() -> ThemeData {
        ThemeData {
            text: [220, 220, 220],
            text_muted: [140, 140, 140],
            accent: [157, 124, 216],
            highlight: [250, 178, 131],
            logo: [255, 185, 0],
            background: [20, 20, 20],
            background_panel: [20, 20, 20],
            background_overlay: [10, 10, 10],
            border: [250, 178, 131],
            success: [127, 216, 143],
            error: [224, 108, 117],
            inverted_text: [20, 20, 20],
        }
    }

    fn base_app() -> App {
        let mut state = IptvState::new();
        state.channels = vec![
            crate::m3u::Channel {
                name: "ABC News".into(),
                url: "http://abc.com/stream".into(),
                tvg_id: Some("abc-news".into()),
                tvg_name: Some("ABC News".into()),
                tvg_logo: None,
                group_title: Some("News".into()),
                attrs: std::collections::BTreeMap::new(),
            },
            crate::m3u::Channel {
                name: "Sports HD".into(),
                url: "http://sports.com/stream".into(),
                tvg_id: Some("sports-hd".into()),
                tvg_name: Some("Sports HD".into()),
                tvg_logo: None,
                group_title: Some("Sports".into()),
                attrs: std::collections::BTreeMap::new(),
            },
            crate::m3u::Channel {
                name: "Music TV".into(),
                url: "http://music.com/stream".into(),
                tvg_id: Some("music-tv".into()),
                tvg_name: Some("Music TV".into()),
                tvg_logo: None,
                group_title: Some("Music".into()),
                attrs: std::collections::BTreeMap::new(),
            },
        ];
        state.apply_filter();
        App {
            state,
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            tx_cmd: None,
            rx_msg: None,
            mpv_thread: None,
            init_error: None,
            mpv_warnings: Vec::new(),
            dirty: true,
            cached_commands: Vec::new(),
            user: None,
            pending_request: None,
        }
    }

    // ── channel list ──────────────────────────────────────────────

    #[test]
    fn enter_plays_selected() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Enter));
        assert!(matches!(
            app.state.play_state,
            PlaybackState::Buffering { .. }
        ));
    }

    #[test]
    fn space_toggles_favorite() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char(' ')));
        assert!(app.state.is_favorite("http://abc.com/stream"));
        assert!(app.pending_request.is_some());
    }

    #[test]
    fn x_stops_playback() {
        let mut app = base_app();
        app.state.play_state = PlaybackState::Playing { channel_index: 0 };
        assert!(app.handle_key(IpcKey::Char('x')));
        assert!(matches!(app.state.play_state, PlaybackState::Stopped));
    }

    #[test]
    fn slash_enters_search() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('/')));
        assert!(matches!(app.state.screen, Screen::Search));
        assert!(app.state.query.is_empty());
    }

    #[test]
    fn g_toggles_group_filter() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('g')));
        assert!(matches!(app.state.screen, Screen::GroupFilter));
        assert_eq!(app.state.group_filter.as_deref(), Some("Music"));
        // Toggle off
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.group_filter, None);
        assert!(matches!(app.state.screen, Screen::ChannelList));
    }

    #[test]
    fn f_toggles_favorites_only() {
        let mut app = base_app();
        app.state.favorites.insert("http://abc.com/stream".into());
        assert!(app.handle_key(IpcKey::Char('f')));
        assert!(app.state.show_favorites_only);
        assert_eq!(app.state.filtered.len(), 1);
        assert!(app.handle_key(IpcKey::Char('f')));
        assert!(!app.state.show_favorites_only);
        assert_eq!(app.state.filtered.len(), 3);
    }

    #[test]
    fn u_enters_url_editor() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('u')));
        assert!(matches!(app.state.screen, Screen::PlaylistUrlEditor));
    }

    #[test]
    fn jk_navigate() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('j')));
        assert_eq!(app.state.selected, 1);
        assert!(app.handle_key(IpcKey::Char('k')));
        assert_eq!(app.state.selected, 0);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 1);
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 0);
    }

    #[test]
    fn plus_minus_volume() {
        let mut app = base_app();
        app.state.volume = 50;
        assert!(app.handle_key(IpcKey::Char('+')));
        assert_eq!(app.state.volume, 52);
        assert!(app.handle_key(IpcKey::Char('-')));
        assert_eq!(app.state.volume, 50);
    }

    #[test]
    fn esc_while_group_filter_exits_filter() {
        let mut app = base_app();
        app.state.screen = Screen::GroupFilter;
        app.state.group_filter = Some("News".into());
        assert!(app.handle_key(IpcKey::Esc));
        assert!(matches!(app.state.screen, Screen::ChannelList));
        assert_eq!(app.state.group_filter, None);
    }

    #[test]
    fn esc_while_channel_list_not_consumed() {
        let mut app = base_app();
        app.state.screen = Screen::ChannelList;
        assert!(!app.handle_key(IpcKey::Esc));
    }

    // ── search mode ───────────────────────────────────────────────

    #[test]
    fn search_esc_exits() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        app.state.query = "abc".into();
        assert!(app.handle_key(IpcKey::Esc));
        assert!(matches!(app.state.screen, Screen::ChannelList));
        assert!(app.state.query.is_empty());
    }

    #[test]
    fn search_enter_plays_and_exits() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        assert!(app.handle_key(IpcKey::Enter));
        assert!(matches!(app.state.screen, Screen::ChannelList));
        assert!(matches!(
            app.state.play_state,
            PlaybackState::Buffering { .. }
        ));
    }

    #[test]
    fn search_backspace_removes_char() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        app.state.query = "ab".into();
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.query, "a");
    }

    #[test]
    fn search_char_adds_to_query() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        assert!(app.handle_key(IpcKey::Char('x')));
        assert_eq!(app.state.query, "x");
    }

    #[test]
    fn search_control_char_not_added() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        assert!(!app.handle_key(IpcKey::Char('\n')));
        assert!(app.state.query.is_empty());
    }

    #[test]
    fn search_unhandled_key_not_consumed() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        assert!(!app.handle_key(IpcKey::F(1)));
    }

    // ── url editor ────────────────────────────────────────────────

    #[test]
    fn url_editor_enter_saves() {
        let mut app = base_app();
        app.state.screen = Screen::PlaylistUrlEditor;
        app.state.url_edit = "https://example.com/list.m3u".into();
        app.state.url_edit_cursor = app.state.url_edit.chars().count();
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.playlist_url, "https://example.com/list.m3u");
        assert!(matches!(app.state.screen, Screen::ChannelList));
    }

    #[test]
    fn url_editor_backspace_deletes() {
        let mut app = base_app();
        app.state.screen = Screen::PlaylistUrlEditor;
        app.state.url_edit = "abc".into();
        app.state.url_edit_cursor = 3;
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.url_edit, "ab");
        assert_eq!(app.state.url_edit_cursor, 2);
    }

    #[test]
    fn url_editor_delete_removes_forward() {
        let mut app = base_app();
        app.state.screen = Screen::PlaylistUrlEditor;
        app.state.url_edit = "abc".into();
        app.state.url_edit_cursor = 0;
        assert!(app.handle_key(IpcKey::Delete));
        assert_eq!(app.state.url_edit, "bc");
    }

    #[test]
    fn url_editor_left_right() {
        let mut app = base_app();
        app.state.screen = Screen::PlaylistUrlEditor;
        app.state.url_edit = "abc".into();
        app.state.url_edit_cursor = 1;
        assert!(app.handle_key(IpcKey::Right));
        assert_eq!(app.state.url_edit_cursor, 2);
        assert!(app.handle_key(IpcKey::Left));
        assert_eq!(app.state.url_edit_cursor, 1);
    }

    #[test]
    fn url_editor_char_types() {
        let mut app = base_app();
        app.state.screen = Screen::PlaylistUrlEditor;
        app.state.url_edit = "ab".into();
        app.state.url_edit_cursor = 1;
        assert!(app.handle_key(IpcKey::Char('X')));
        assert_eq!(app.state.url_edit, "aXb");
        assert_eq!(app.state.url_edit_cursor, 2);
    }

    #[test]
    fn url_editor_unhandled_not_consumed() {
        let mut app = base_app();
        app.state.screen = Screen::PlaylistUrlEditor;
        assert!(!app.handle_key(IpcKey::F(1)));
    }

    // ── favorites persistence ─────────────────────────────────────

    #[test]
    fn handle_db_value_loads_favorites() {
        let mut app = base_app();
        let favs = r#"["http://abc.com/stream","http://sports.com/stream"]"#;
        app.handle_db_value("favorites", Some(favs.to_string()));
        assert_eq!(app.state.favorites_count(), 2);
        assert!(app.state.is_favorite("http://abc.com/stream"));
        assert!(!app.state.is_favorite("http://music.com/stream"));
    }

    #[test]
    fn handle_db_value_loads_preferences() {
        let mut app = base_app();
        let prefs = r#"{"volume": 75, "playlist_url": "https://custom/playlist.m3u"}"#;
        app.handle_db_value("preferences", Some(prefs.to_string()));
        assert_eq!(app.state.volume, 75);
        assert_eq!(app.state.playlist_url, "https://custom/playlist.m3u");
    }

    #[test]
    fn handle_db_value_loads_cached_playlist() {
        let mut app = base_app();
        app.state.channels.clear();
        app.state.filtered.clear();
        let channels = r#"[{"name":"Cached","url":"http://cached","tvg_id":null,"tvg_name":null,"tvg_logo":null,"group_title":null,"attrs":{}}]"#;
        app.handle_db_value("playlist-cache", Some(channels.to_string()));
        assert_eq!(app.state.channels.len(), 1);
        assert_eq!(app.state.channels[0].name, "Cached");
    }
}
