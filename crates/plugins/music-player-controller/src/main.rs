use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    host: String,
    port: u16,
    status_text: String,
    song: String,
    artist: String,
    elapsed: u64,
    duration: u64,
    state: String,
    playlist: Vec<String>,
    playlist_pos: usize,
    playlist_scroll: u16,
    view_playlist: bool,
    connected: bool,
    status_msg: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            host: String::from("127.0.0.1"),
            port: 6600,
            status_text: String::new(),
            song: String::from("Not connected"),
            artist: String::new(),
            elapsed: 0,
            duration: 0,
            state: String::from("stop"),
            playlist: Vec::new(),
            playlist_pos: 0,
            playlist_scroll: 0,
            view_playlist: false,
            connected: false,
            status_msg: String::from("Connect to MPD..."),
        }
    }
}

impl App {
    fn mpd_command(host: &str, port: u16, cmd: &str) -> Option<String> {
        let addr = format!("{}:{}", host, port);
        let stream =
            TcpStream::connect_timeout(&addr.parse().ok()?, Duration::from_secs(2)).ok()?;
        let mut buf = String::new();
        stream
            .set_read_timeout(Some(Duration::from_millis(500)))
            .ok()?;
        use std::io::Write;
        let mut reader = BufReader::new(&stream);
        reader.read_line(&mut buf).ok()?;
        if !buf.starts_with("OK") {
            return None;
        }
        writeln!(&stream, "{}", cmd).ok()?;
        let mut response = String::new();
        loop {
            buf.clear();
            if reader.read_line(&mut buf).ok()? == 0 {
                break;
            }
            if buf.starts_with("OK") || buf.starts_with("ACK") {
                break;
            }
            response.push_str(&buf);
        }
        Some(response)
    }

    fn connect(&mut self) {
        let resp = Self::mpd_command(&self.host, self.port, "status");
        if resp.is_some() {
            self.connected = true;
            self.status_msg = String::from("Connected to MPD");
            self.refresh();
        } else {
            self.connected = false;
            self.status_msg = String::from("Failed to connect to MPD");
        }
    }

    fn refresh(&mut self) {
        if !self.connected {
            self.connect();
            return;
        }
        if let Some(status_resp) = Self::mpd_command(&self.host, self.port, "status") {
            for line in status_resp.lines() {
                if let Some(val) = line.strip_prefix("state: ") {
                    self.state = val.trim().to_string();
                } else if let Some(val) = line.strip_prefix("song: ") {
                    self.playlist_pos = val.trim().parse().unwrap_or(0);
                } else if let Some(val) = line.strip_prefix("elapsed: ") {
                    self.elapsed = val
                        .trim()
                        .split('.')
                        .next()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0);
                } else if let Some(val) = line.strip_prefix("duration: ") {
                    let dur_str = val.trim().split('.').next().unwrap_or("0");
                    self.duration = dur_str.parse().unwrap_or(0);
                }
            }
        }
        if let Some(song_resp) = Self::mpd_command(&self.host, self.port, "currentsong") {
            for line in song_resp.lines() {
                if let Some(val) = line.strip_prefix("Title: ") {
                    self.song = val.trim().to_string();
                } else if let Some(val) = line.strip_prefix("Artist: ") {
                    self.artist = val.trim().to_string();
                }
            }
        }
        let mut pl = Vec::new();
        if let Some(pl_resp) = Self::mpd_command(&self.host, self.port, "playlistinfo") {
            let mut current_title = String::new();
            for line in pl_resp.lines() {
                if let Some(val) = line.strip_prefix("Title: ") {
                    current_title = val.trim().to_string();
                } else if let Some(val) = line.strip_prefix("file: ") {
                    if current_title.is_empty() {
                        let path = val.trim();
                        let basename = path.rsplit('/').next().unwrap_or(path);
                        current_title = basename.to_string();
                    }
                }
                if line.trim().is_empty() && !current_title.is_empty() {
                    pl.push(current_title.clone());
                    current_title.clear();
                }
            }
            if !current_title.is_empty() {
                pl.push(current_title);
            }
        }
        self.playlist = pl;
        self.status_text = format!("{} songs in playlist", self.playlist.len());
    }

    fn send_cmd(&mut self, cmd: &str) {
        if !self.connected {
            self.connect();
            return;
        }
        Self::mpd_command(&self.host, self.port, cmd);
        self.refresh();
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.view_playlist {
            match key {
                IpcKey::Esc => {
                    self.view_playlist = false;
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    self.playlist_scroll = self.playlist_scroll.saturating_sub(1);
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    self.playlist_scroll = self.playlist_scroll.saturating_add(1);
                    true
                }
                _ => true,
            }
        } else {
            match key {
                IpcKey::Char(' ') => {
                    if self.state == "play" {
                        self.send_cmd("pause 1");
                        self.status_msg = String::from("Paused");
                    } else {
                        self.send_cmd("pause 0");
                        self.status_msg = String::from("Playing");
                    }
                    true
                }
                IpcKey::Char('n') => {
                    self.send_cmd("next");
                    self.status_msg = String::from("Next track");
                    true
                }
                IpcKey::Char('p') => {
                    self.send_cmd("previous");
                    self.status_msg = String::from("Previous track");
                    true
                }
                IpcKey::Char('r') => {
                    self.connect();
                    true
                }
                IpcKey::Char('l') => {
                    self.view_playlist = true;
                    true
                }
                IpcKey::Esc => false,
                _ => true,
            }
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let mut cmds = Vec::new();
        let t = &self.theme;
        let w = self.area.w.max(40);
        let h = self.area.h.max(10);

        cmds.push(json!({
            "type": "Rect", "x": 0, "y": 0, "w": w, "h": h, "bg": t.background
        }));
        cmds.push(json!({
            "type": "Border", "x": 0, "y": 0, "w": w, "h": h, "fg": t.border,
            "borders": BORDER_ALL, "bg": t.background_panel,
            "title": " MPD Controller ",
            "title_fg": t.text, "title_dash_fg": t.border
        }));

        if self.view_playlist {
            let list_y = 2u16;
            let list_h = h.saturating_sub(3) as usize;
            for (i, song) in self
                .playlist
                .iter()
                .enumerate()
                .skip(self.playlist_scroll as usize)
                .take(list_h)
            {
                let y = list_y + (i as u16).saturating_sub(self.playlist_scroll);
                let is_current = i == self.playlist_pos;
                cmds.push(json!({
                    "type": "Text", "x": 2, "y": y, "text": song.clone(),
                    "fg": if is_current { t.accent } else { t.text },
                    "bg": null, "bold": is_current, "modifiers": 0
                }));
            }
        } else {
            let status_icon = match self.state.as_str() {
                "play" => "\u{25B6}",
                "pause" => "\u{23F8}",
                _ => "\u{25A0}",
            };

            cmds.push(json!({
                "type": "Text", "x": 2, "y": 2, "text": format!("{} {}", status_icon, self.song),
                "fg": t.accent, "bg": null, "bold": true, "modifiers": 0
            }));
            cmds.push(json!({
                "type": "Text", "x": 2, "y": 3, "text": self.artist.clone(),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0
            }));

            let gauge_w = w.saturating_sub(4);
            if self.duration > 0 {
                let ratio = (self.elapsed as f64) / (self.duration as f64);
                let ratio = ratio.clamp(0.0, 1.0);
                let elapsed_str = format!("{}:{:02}", self.elapsed / 60, self.elapsed % 60);
                let dur_str = format!("{}:{:02}", self.duration / 60, self.duration % 60);
                let label = format!("{} / {}", elapsed_str, dur_str);
                cmds.push(json!({
                    "type": "Gauge", "x": 2, "y": 5, "w": gauge_w, "h": 1,
                    "ratio": ratio, "label": label,
                    "style": { "fg": t.text_muted, "bg": t.background },
                    "gauge_style": { "fg": t.accent, "bg": t.background }
                }));
            }

            let list_y = 7u16;
            let list_h = h.saturating_sub(9) as usize;
            for (i, song) in self.playlist.iter().enumerate().take(list_h) {
                let y = list_y + i as u16;
                let is_current = i == self.playlist_pos;
                cmds.push(json!({
                    "type": "Text", "x": 2, "y": y, "text": song.clone(),
                    "fg": if is_current { t.accent } else { t.text_muted },
                    "bg": null, "bold": is_current, "modifiers": 0
                }));
            }
        }

        cmds.push(json!({
            "type": "Text", "x": 2, "y": h.saturating_sub(1),
            "text": self.status_msg.clone(),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0
        }));

        if !self.view_playlist {
            cmds.push(json!({
                "type": "Text", "x": 2, "y": h,
                "text": String::from("space play/pause  \u{b7} n next  \u{b7} p prev  \u{b7} r reconnect  \u{b7} l playlist  \u{b7} esc"),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0
            }));
        }

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
    }
}

fn default_theme() -> ThemeData {
    ThemeData {
        text: [220; 3],
        text_muted: [140; 3],
        accent: [180; 3],
        highlight: [220; 3],
        logo: [255; 3],
        background: [0; 3],
        background_panel: [20; 3],
        background_overlay: [10; 3],
        border: [150; 3],
        success: [127, 216, 143],
        error: [224, 108, 117],
        inverted_text: [20; 3],
    }
}

fn palette_commands() -> Value {
    json!([["Music Controller", "Control MPD music player"]])
}

fn respond(app: &mut App, consumed: bool) {
    let commands_val = app.render();
    let json = json!({
        "commands": commands_val, "hints": [], "palette_commands": palette_commands(),
        "request": null, "plugin_message": null, "consumed": consumed,
    });
    if let Ok(json_str) = serde_json::to_string(&json) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{json_str}");
        let _ = out.flush();
    }
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = App::default();
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut line = String::new();
    app.connect();
    app.dirty = true;
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        let trimmed = line.trim_end();
        let msg = serde_json::from_str::<HostMsg>(trimmed);
        let consumed = match msg {
            Ok(HostMsg::Init { theme, area, .. }) => {
                app.theme = theme;
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Resize { area }) => {
                app.area = area;
                app.dirty = true;
                false
            }
            Ok(HostMsg::ThemeChange { theme }) => {
                app.theme = theme;
                app.dirty = true;
                false
            }
            Ok(HostMsg::Key { key, modifiers }) => app.handle_key(key, modifiers),
            Ok(HostMsg::PaletteCommand { .. }) => {
                app.dirty = true;
                true
            }
            Ok(HostMsg::Tick) => {
                if app.connected {
                    app.refresh();
                    app.dirty = true;
                }
                false
            }
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[music-player-controller] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
