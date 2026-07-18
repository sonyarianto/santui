use std::io::{BufRead, BufReader};
use std::process::Command;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct Package {
    name: String,
    version: String,
    description: String,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    query: String,
    results: Vec<Package>,
    selected: usize,
    scroll: u16,
    view_detail: bool,
    input_mode: bool,
    input_buffer: String,
    pm_name: String,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            scroll: 0,
            view_detail: false,
            input_mode: true,
            input_buffer: String::new(),
            pm_name: String::from("unknown"),
            status: String::from("Detecting package manager..."),
        }
    }
}

impl App {
    fn detect_pm(&mut self) {
        for pm in &["apt", "pacman", "brew", "dnf"] {
            if Command::new("which")
                .arg(pm)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                self.pm_name = pm.to_string();
                self.status = format!("Using {} - type a query and press Enter", pm);
                return;
            }
        }
        self.pm_name = String::from("dpkg-query");
        self.status = String::from("No package manager found, trying dpkg-query");
    }

    fn search_packages(&mut self, q: &str) {
        self.results.clear();
        self.selected = 0;
        self.scroll = 0;
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            return;
        }
        match self.pm_name.as_str() {
            "apt" => {
                let output = Command::new("apt-cache")
                    .arg("search")
                    .arg(&q)
                    .output()
                    .ok();
                if let Some(out) = output {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines().take(200) {
                        let parts: Vec<&str> = line.splitn(2, " - ").collect();
                        if parts.len() == 2 {
                            let name = parts[0].trim().to_string();
                            let desc = parts[1].trim().to_string();
                            self.results.push(Package {
                                name,
                                version: String::from("?"),
                                description: desc,
                            });
                        }
                    }
                }
            }
            "pacman" => {
                let output = Command::new("pacman").arg("-Qs").arg(&q).output().ok();
                if let Some(out) = output {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let mut current_name = String::new();
                    let mut current_version = String::new();
                    for line in stdout.lines() {
                        if line.starts_with("local/")
                            || line.starts_with("extra/")
                            || line.starts_with("core/")
                            || line.starts_with("community/")
                        {
                            let rest =
                                line.trim_start_matches(|c: char| c.is_alphanumeric() || c == '/');
                            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                            let nv = parts[0];
                            let nv_parts: Vec<&str> = nv.rsplitn(2, ' ').collect();
                            let name_only = nv_parts.last().unwrap_or(&nv).to_string();
                            current_name = name_only;
                            current_version = String::from("?");
                            if let Some(v) = nv.rsplit(' ').next() {
                                current_version = v.trim_matches(|c| c == '>').to_string();
                            }
                        } else if !line.trim().is_empty() && !current_name.is_empty() {
                            let desc = line.trim().to_string();
                            self.results.push(Package {
                                name: current_name.clone(),
                                version: current_version.clone(),
                                description: desc,
                            });
                            current_name.clear();
                        }
                    }
                }
            }
            "brew" => {
                let output = Command::new("brew").arg("search").arg(&q).output().ok();
                if let Some(out) = output {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines().take(200) {
                        let name = line.trim().to_string();
                        if !name.is_empty() && !name.starts_with("==") {
                            self.results.push(Package {
                                name,
                                version: String::from("?"),
                                description: String::new(),
                            });
                        }
                    }
                }
            }
            "dnf" => {
                let output = Command::new("dnf").arg("search").arg(&q).output().ok();
                if let Some(out) = output {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines().skip(1).take(200) {
                        let parts: Vec<&str> = line.splitn(2, " : ").collect();
                        if parts.len() == 2 {
                            let name = parts[0].trim().to_string();
                            let desc = parts[1].trim().to_string();
                            self.results.push(Package {
                                name,
                                version: String::from("?"),
                                description: desc,
                            });
                        }
                    }
                }
            }
            _ => {
                let output = Command::new("dpkg-query").arg("-W").output().ok();
                if let Some(out) = output {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for line in stdout.lines() {
                        let parts: Vec<&str> = line.splitn(2, '\t').collect();
                        if parts.len() == 2 {
                            let name = parts[0].to_lowercase();
                            if name.contains(&q) {
                                self.results.push(Package {
                                    name: parts[0].to_string(),
                                    version: parts[1].to_string(),
                                    description: String::new(),
                                });
                            }
                        }
                    }
                }
            }
        }
        self.status = format!("Found {} packages", self.results.len());
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        if self.input_mode {
            match key {
                IpcKey::Esc => {
                    self.input_mode = false;
                    self.input_buffer.clear();
                    self.status = String::from("Ready");
                    true
                }
                IpcKey::Enter => {
                    let q = self.input_buffer.trim().to_string();
                    if !q.is_empty() {
                        self.query = q.clone();
                        self.search_packages(&q);
                    }
                    self.input_mode = false;
                    self.input_buffer.clear();
                    true
                }
                IpcKey::Backspace => {
                    self.input_buffer.pop();
                    true
                }
                IpcKey::Char(c) => {
                    self.input_buffer.push(c);
                    true
                }
                _ => true,
            }
        } else if self.view_detail {
            match key {
                IpcKey::Esc => {
                    self.view_detail = false;
                    true
                }
                _ => true,
            }
        } else {
            match key {
                IpcKey::Char('/') => {
                    self.input_mode = true;
                    self.input_buffer.clear();
                    self.status = String::from("Search packages:");
                    true
                }
                IpcKey::Enter => {
                    if self.selected < self.results.len() {
                        self.view_detail = true;
                    }
                    true
                }
                IpcKey::Up | IpcKey::Char('k') => {
                    if self.selected > 0 {
                        self.selected -= 1;
                        self.update_scroll();
                    }
                    true
                }
                IpcKey::Down | IpcKey::Char('j') => {
                    if self.selected + 1 < self.results.len() {
                        self.selected += 1;
                        self.update_scroll();
                    }
                    true
                }
                IpcKey::Esc => false,
                _ => true,
            }
        }
    }

    fn update_scroll(&mut self) {
        let list_h = self.area.h.saturating_sub(5) as usize;
        if self.selected < self.scroll as usize {
            self.scroll = self.selected as u16;
        }
        if self.selected >= self.scroll as usize + list_h {
            self.scroll = (self.selected.saturating_sub(list_h).saturating_add(1)) as u16;
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

        cmds.push(json!({"Rect": {
        "x": 0, "y": 0, "w": w, "h": h, "bg": t.background

        }}));
        cmds.push(json!({"Border": {
        "x": 0, "y": 0, "w": w, "h": h, "fg": t.border,
                    "borders": BORDER_ALL, "bg": t.background_panel,
                    "title": " Package Manager ",
                    "title_fg": t.text, "title_dash_fg": t.border

        }}));

        cmds.push(json!({"Text": {
        "x": 2, "y": 1, "text": format!("PM: {}", self.pm_name),
                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

        }}));

        if self.input_mode {
            cmds.push(json!({"Text": {
            "x": 2, "y": 2, "text": String::from("Search: "),
                            "fg": t.text, "bg": null, "bold": false, "modifiers": 0

            }}));
            let cursor_text = format!("{}_", self.input_buffer);
            cmds.push(json!({"Text": {
            "x": 10, "y": 2, "text": cursor_text,
                            "fg": t.accent, "bg": null, "bold": false, "modifiers": 0

            }}));
        } else if self.view_detail {
            if let Some(pkg) = self.results.get(self.selected) {
                cmds.push(json!({"Text": {
                "x": 2, "y": 2, "text": pkg.name.clone(),
                                    "fg": t.accent, "bg": null, "bold": true, "modifiers": 0

                }}));
                cmds.push(json!({"Text": {
                "x": 2, "y": 3,
                                    "text": format!("Version: {}", pkg.version),
                                    "fg": t.text, "bg": null, "bold": false, "modifiers": 0

                }}));
                let desc_lines = word_wrap(&pkg.description, (w.saturating_sub(4)) as usize);
                for (i, line) in desc_lines.iter().enumerate() {
                    let y = 5 + i as u16;
                    if y >= h.saturating_sub(2) {
                        break;
                    }
                    cmds.push(json!({"Text": {
                    "x": 2, "y": y, "text": line.clone(),
                                            "fg": t.text, "bg": null, "bold": false, "modifiers": 0

                    }}));
                }
            }
        } else {
            let list_y = 3u16;
            let list_h = h.saturating_sub(5) as usize;
            for (i, pkg) in self
                .results
                .iter()
                .enumerate()
                .skip(self.scroll as usize)
                .take(list_h)
            {
                let y = list_y + (i as u16).saturating_sub(self.scroll);
                let is_sel = i == self.selected;
                cmds.push(json!({"Text": {
                "x": 2, "y": y, "text": pkg.name.clone(),
                                    "fg": if is_sel { t.highlight } else { t.text },
                                    "bg": if is_sel { Some(t.background_overlay) } else { None },
                                    "bold": is_sel, "modifiers": 0

                }}));
            }
        }

        cmds.push(json!({"Text": {
        "x": 2, "y": h.saturating_sub(1),
                    "text": self.status.clone(),
                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

        }}));

        if !self.input_mode && !self.view_detail {
            cmds.push(json!({"Text": {
"x": 2, "y": h,
                "text": String::from("/ search  \u{b7} enter detail  \u{b7} \u{2191}\u{2193} nav  \u{b7} esc"),
                "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

}}));
        }

        self.cached_commands = cmds.clone();
        self.dirty = false;
        cmds
    }
}

fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.len() + word.len() + 1 > max_width && !line.is_empty() {
            lines.push(line.clone());
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
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

fn palette_commands() -> Vec<(String, String)> {
    vec![("Package Manager".into(), "Search system packages".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app
            .render()
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect(),
        hints: vec![],
        palette_commands: palette_commands(),
        request: None,
        plugin_message: None,
        consumed,
    };
    let mut out = std::io::stdout().lock();
    let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &msg);
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut app = App::default();
    app.detect_pm();
    let mut reader = BufReader::new(std::io::stdin().lock());
    let mut line = String::new();
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
            Ok(HostMsg::Tick) => false,
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
                log::error!("[package-manager] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
