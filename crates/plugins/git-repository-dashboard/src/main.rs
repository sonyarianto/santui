use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginRequest, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use serde::{Deserialize, Serialize};

const DB_KEY: &str = "git-repository-dashboard";
const GIT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RepositoryConfig {
    name: String,
    path: String,
}

#[derive(Debug, Clone, Default)]
struct RepositoryStatus {
    root: String,
    branch: String,
    upstream: String,
    ahead: i32,
    behind: i32,
    staged: usize,
    unstaged: usize,
    untracked: usize,
    latest_commit: String,
    commits: Vec<String>,
    error: String,
}

impl RepositoryStatus {
    fn is_dirty(&self) -> bool {
        self.staged + self.unstaged + self.untracked > 0
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    repositories: Vec<RepositoryConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    List,
    AddRepo { field: RepoField },
    ConfirmRemove(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepoField {
    Name,
    Path,
}

struct TrackedRepo {
    config: RepositoryConfig,
    status: RepositoryStatus,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    repos: Vec<TrackedRepo>,
    filtered: Vec<usize>,
    cursor: usize,
    search: String,
    screen: Screen,
    repo_name: String,
    repo_path: String,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 112, h: 30 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet { key: DB_KEY.into() }),
            repos: Vec::new(),
            filtered: Vec::new(),
            cursor: 0,
            search: String::new(),
            screen: Screen::List,
            repo_name: String::new(),
            repo_path: String::new(),
            status:
                "a add · r refresh · R refresh all · / search · c copy path · o open · d remove"
                    .into(),
        };
        app.apply_filter();
        app
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey) -> bool {
        self.dirty = true;
        match self.screen.clone() {
            Screen::List => self.handle_list_key(key),
            Screen::AddRepo { field } => self.handle_add_key(key, field),
            Screen::ConfirmRemove(idx) => self.handle_confirm_key(key, idx),
        }
    }

    fn handle_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char('a') => {
                self.repo_name.clear();
                self.repo_path.clear();
                self.screen = Screen::AddRepo {
                    field: RepoField::Name,
                };
                true
            }
            IpcKey::Char('r') => {
                if let Some(idx) = self.selected_repo_index() {
                    self.refresh_repo(idx);
                }
                true
            }
            IpcKey::Char('R') => {
                self.refresh_all();
                true
            }
            IpcKey::Char('d') => {
                if let Some(idx) = self.selected_repo_index() {
                    self.screen = Screen::ConfirmRemove(idx);
                }
                true
            }
            IpcKey::Char('/') => {
                self.search.clear();
                self.status = "Search repositories".into();
                true
            }
            IpcKey::Char('c') => {
                self.copy_selected_path();
                true
            }
            IpcKey::Char('o') => {
                self.open_selected_path();
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.filtered.len().saturating_sub(1);
                self.cursor = self.cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Backspace => {
                if !self.search.is_empty() {
                    self.search.pop();
                    self.apply_filter();
                    true
                } else {
                    false
                }
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.search.push(ch);
                self.apply_filter();
                true
            }
            IpcKey::Esc if !self.search.is_empty() => {
                self.search.clear();
                self.apply_filter();
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_add_key(&mut self, key: IpcKey, field: RepoField) -> bool {
        match key {
            IpcKey::Tab | IpcKey::BackTab => {
                self.screen = Screen::AddRepo {
                    field: if field == RepoField::Name {
                        RepoField::Path
                    } else {
                        RepoField::Name
                    },
                };
                true
            }
            IpcKey::Enter => {
                if let Err(e) = self.save_repo() {
                    self.status = e;
                }
                true
            }
            IpcKey::Backspace => {
                self.repo_buf_mut(field).pop();
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::List;
                true
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.repo_buf_mut(field).push(ch);
                true
            }
            _ => false,
        }
    }

    fn handle_confirm_key(&mut self, key: IpcKey, idx: usize) -> bool {
        match key {
            IpcKey::Char('y') | IpcKey::Char('Y') => {
                if idx < self.repos.len() {
                    self.repos.remove(idx);
                }
                self.screen = Screen::List;
                self.status = "Repository removed".into();
                self.schedule_save();
                self.apply_filter();
                true
            }
            IpcKey::Char('n') | IpcKey::Esc => {
                self.screen = Screen::List;
                true
            }
            _ => true,
        }
    }

    fn repo_buf_mut(&mut self, field: RepoField) -> &mut String {
        match field {
            RepoField::Name => &mut self.repo_name,
            RepoField::Path => &mut self.repo_path,
        }
    }

    fn save_repo(&mut self) -> Result<(), String> {
        let path = self.repo_path.trim().to_string();
        if path.is_empty() {
            return Err("Repository path is required".into());
        }
        if !Path::new(&path).is_dir() {
            return Err("Path must be an existing directory".into());
        }
        let name = if self.repo_name.trim().is_empty() {
            Path::new(&path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Repository")
                .to_string()
        } else {
            self.repo_name.trim().to_string()
        };
        let config = RepositoryConfig { name, path };
        let status = inspect_repo(&config.path);
        self.repos.push(TrackedRepo { config, status });
        self.screen = Screen::List;
        self.status = "Repository added".into();
        self.schedule_save();
        self.apply_filter();
        Ok(())
    }

    fn refresh_repo(&mut self, idx: usize) {
        if let Some(repo) = self.repos.get_mut(idx) {
            repo.status = inspect_repo(&repo.config.path);
            self.status = format!("Refreshed {}", repo.config.name);
        }
    }

    fn refresh_all(&mut self) {
        for repo in &mut self.repos {
            repo.status = inspect_repo(&repo.config.path);
        }
        self.status = format!("Refreshed {} repos", self.repos.len());
    }

    fn selected_repo_index(&self) -> Option<usize> {
        self.filtered.get(self.cursor).copied()
    }

    fn selected_repo(&self) -> Option<&TrackedRepo> {
        self.selected_repo_index()
            .and_then(|idx| self.repos.get(idx))
    }

    fn copy_selected_path(&mut self) {
        if let Some(repo) = self.selected_repo() {
            match copy_to_clipboard(&repo.config.path) {
                Ok(()) => self.status = "Copied repository path".into(),
                Err(e) => self.status = format!("Clipboard error: {e}"),
            }
        }
    }

    fn open_selected_path(&mut self) {
        if let Some(repo) = self.selected_repo() {
            match open_path(&repo.config.path) {
                Ok(()) => self.status = "Opened repository path".into(),
                Err(e) => self.status = format!("Open failed: {e}"),
            }
        }
    }

    fn apply_filter(&mut self) {
        let query = self.search.trim().to_lowercase();
        self.filtered = self
            .repos
            .iter()
            .enumerate()
            .filter(|(_, repo)| {
                query.is_empty()
                    || repo.config.name.to_lowercase().contains(&query)
                    || repo.config.path.to_lowercase().contains(&query)
                    || repo.status.branch.to_lowercase().contains(&query)
                    || repo.status.root.to_lowercase().contains(&query)
            })
            .map(|(idx, _)| idx)
            .collect();
        self.cursor = self.cursor.min(self.filtered.len().saturating_sub(1));
    }

    fn serialize(&self) -> String {
        let repositories = self.repos.iter().map(|repo| repo.config.clone()).collect();
        serde_json::to_string(&PersistedState { repositories }).unwrap_or_default()
    }

    fn load(&mut self, json: &str) {
        if let Ok(state) = serde_json::from_str::<PersistedState>(json) {
            self.repos = state
                .repositories
                .into_iter()
                .map(|config| {
                    let status = inspect_repo(&config.path);
                    TrackedRepo { config, status }
                })
                .collect();
            self.apply_filter();
        }
    }

    fn schedule_save(&mut self) {
        self.pending_request = Some(PluginRequest::DbSet {
            key: DB_KEY.into(),
            value: self.serialize(),
        });
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn inspect_repo(path: &str) -> RepositoryStatus {
    let p = Path::new(path);
    if !p.exists() {
        return status_error("missing path");
    }
    if !p.is_dir() {
        return status_error("path is not a directory");
    }
    let root = match run_git(path, &["rev-parse", "--show-toplevel"]) {
        Ok(out) => out.trim().to_string(),
        Err(e) => return status_error(&e),
    };
    let status_output = match run_git(path, &["status", "--porcelain=v2", "--branch"]) {
        Ok(out) => out,
        Err(e) => return status_error(&e),
    };
    let mut status = parse_status(&status_output);
    status.root = root;
    match run_git(path, &["log", "-5", "--pretty=format:%h %s"]) {
        Ok(out) => {
            status.commits = out.lines().map(str::to_string).collect();
            status.latest_commit = status.commits.first().cloned().unwrap_or_default();
        }
        Err(e) => status.error = e,
    }
    status
}

fn status_error(error: &str) -> RepositoryStatus {
    RepositoryStatus {
        error: error.into(),
        ..RepositoryStatus::default()
    }
}

fn run_git(path: &str, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new("git")
        .arg("--no-pager")
        .args(args)
        .current_dir(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "git executable not found".to_string(),
            std::io::ErrorKind::PermissionDenied => "permission denied launching git".to_string(),
            _ => format!("failed to launch git: {e}"),
        })?;
    let start = Instant::now();
    loop {
        if let Some(_status) = child.try_wait().map_err(|e| e.to_string())? {
            let output = child.wait_with_output().map_err(|e| e.to_string())?;
            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).to_string());
            }
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                "git command failed".into()
            } else {
                stderr
            });
        }
        if start.elapsed() >= GIT_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err("git command timed out".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn parse_status(output: &str) -> RepositoryStatus {
    let mut status = RepositoryStatus::default();
    for line in output.lines() {
        if let Some(branch) = line.strip_prefix("# branch.head ") {
            status.branch = branch.to_string();
        } else if let Some(upstream) = line.strip_prefix("# branch.upstream ") {
            status.upstream = upstream.to_string();
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            parse_ahead_behind(ab, &mut status);
        } else if let Some(rest) = line.strip_prefix("? ") {
            if !rest.trim().is_empty() {
                status.untracked += 1;
            }
        } else if line.starts_with("1 ") || line.starts_with("2 ") || line.starts_with("u ") {
            parse_change_line(line, &mut status);
        }
    }
    if status.branch.is_empty() {
        status.branch = "(detached or unknown)".into();
    }
    status
}

fn parse_ahead_behind(text: &str, status: &mut RepositoryStatus) {
    for part in text.split_whitespace() {
        if let Some(ahead) = part.strip_prefix('+') {
            status.ahead = ahead.parse().unwrap_or(0);
        } else if let Some(behind) = part.strip_prefix('-') {
            status.behind = behind.parse().unwrap_or(0);
        }
    }
}

fn parse_change_line(line: &str, status: &mut RepositoryStatus) {
    let xy = line.split_whitespace().nth(1).unwrap_or("..");
    let mut chars = xy.chars();
    let x = chars.next().unwrap_or('.');
    let y = chars.next().unwrap_or('.');
    if x != '.' && x != ' ' {
        status.staged += 1;
    }
    if y != '.' && y != ' ' {
        status.unstaged += 1;
    }
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
}

fn open_path(path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", path]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(path);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let theme = app.theme.clone();
    let w = app.area.w.max(82);
    let h = app.area.h.max(20);
    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: theme.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        borders: BORDER_ALL,
        bg: Some(theme.background_panel),
        title: Some(" Git Repository Dashboard ".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    match &app.screen {
        Screen::List => render_list(app, &mut cmds, &theme, w, h),
        Screen::AddRepo { field } => render_add(app, &mut cmds, &theme, h, *field),
        Screen::ConfirmRemove(_) => {
            render_list(app, &mut cmds, &theme, w, h);
            cmds.push(RenderCmd::Dim {
                x: 0,
                y: 0,
                w,
                h,
                bg: theme.background_overlay,
            });
            cmds.push(RenderCmd::Border {
                x: w / 2 - 24,
                y: h / 2 - 2,
                w: 48,
                h: 5,
                fg: theme.error,
                borders: BORDER_ALL,
                bg: Some(theme.background_panel),
                title: Some(" Confirm ".into()),
                title_fg: Some(theme.border),
                title_dash_fg: Some(theme.error),
                border_type: None,
            });
            push_text(
                &mut cmds,
                w / 2 - 22,
                h / 2,
                "Remove selected repository? y confirm · n/Esc cancel",
                theme.text,
                true,
            );
        }
    }
    cmds
}

fn render_list(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let header = format!("Repos: {} · Search: {}", app.repos.len(), app.search);
    push_text(
        cmds,
        2,
        2,
        truncate(&header, w as usize - 4),
        theme.text,
        true,
    );
    let list_h = h.saturating_sub(8).max(4);
    let detail_x = (w * 64 / 100).max(54);
    let rows: Vec<Vec<String>> = app
        .filtered
        .iter()
        .map(|idx| repo_row(&app.repos[*idx]))
        .collect();
    cmds.push(RenderCmd::Table {
        x: 2,
        y: 4,
        w: detail_x.saturating_sub(4),
        h: list_h,
        header: vec![
            "Repo".into(),
            "Branch".into(),
            "Dirty".into(),
            "↑".into(),
            "↓".into(),
        ],
        header_style: TextStyle {
            fg: Some(theme.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![20, 18, 8, 4, 4],
        selected: Some(app.cursor.min(app.filtered.len().saturating_sub(1))),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: true,
            modifiers: 0,
        },
        current_row: None,
        current_style: None,
        cell_styles: None,
    });
    cmds.push(RenderCmd::Border {
        x: detail_x,
        y: 4,
        w: w.saturating_sub(detail_x + 2),
        h: list_h,
        fg: theme.border,
        borders: BORDER_ALL,
        bg: None,
        title: Some(" Detail ".into()),
        title_fg: Some(theme.border),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    if let Some(repo) = app.selected_repo() {
        let detail = repo_detail(repo);
        cmds.push(RenderCmd::Paragraph {
            x: detail_x + 1,
            y: 5,
            w: w.saturating_sub(detail_x + 4),
            h: list_h.saturating_sub(2),
            text: detail,
            style: TextStyle {
                fg: Some(if repo.status.error.is_empty() {
                    theme.text
                } else {
                    theme.error
                }),
                bg: None,
                bold: false,
                modifiers: 0,
            },
            wrap: true,
            spans: None,
            alignment: None,
        });
    }
    push_text(
        cmds,
        2,
        h.saturating_sub(2),
        &app.status,
        theme.text_muted,
        false,
    );
}

fn hints() -> Vec<(String, String)> {
    vec![
        ("a".into(), "add".into()),
        ("r".into(), "refresh".into()),
        ("R".into(), "refresh all".into()),
        ("/".into(), "search".into()),
        ("c".into(), "copy path".into()),
        ("o".into(), "open".into()),
        ("d".into(), "remove".into()),
        ("esc".into(), "back".into()),
    ]
}

fn render_add(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, h: u16, field: RepoField) {
    push_text(
        cmds,
        2,
        2,
        "Add Git repository · Tab field · Enter save · Esc cancel",
        theme.text,
        true,
    );
    push_text(
        cmds,
        2,
        4,
        format!(
            "{} Name: {}",
            if field == RepoField::Name { ">" } else { " " },
            app.repo_name
        ),
        theme.text,
        field == RepoField::Name,
    );
    push_text(
        cmds,
        2,
        5,
        format!(
            "{} Path: {}",
            if field == RepoField::Path { ">" } else { " " },
            app.repo_path
        ),
        theme.text,
        field == RepoField::Path,
    );
    push_text(
        cmds,
        2,
        h.saturating_sub(1),
        &app.status,
        theme.text_muted,
        false,
    );
}

fn repo_row(repo: &TrackedRepo) -> Vec<String> {
    let status = &repo.status;
    let dirty = if status.error.is_empty() {
        if status.is_dirty() {
            format!("{}|{}|{}", status.staged, status.unstaged, status.untracked)
        } else {
            "clean".into()
        }
    } else {
        "error".into()
    };
    vec![
        repo.config.name.clone(),
        status.branch.clone(),
        dirty,
        status.ahead.to_string(),
        status.behind.to_string(),
    ]
}

fn repo_detail(repo: &TrackedRepo) -> String {
    let status = &repo.status;
    if !status.error.is_empty() {
        return format!(
            "{}\n{}\n\nError: {}",
            repo.config.name, repo.config.path, status.error
        );
    }
    format!(
        "{}\nPath: {}\nRoot: {}\nBranch: {}\nUpstream: {}\nAhead/behind: +{} -{}\nChanges: staged {}, unstaged {}, untracked {}\nLatest: {}\n\nRecent commits:\n{}",
        repo.config.name,
        repo.config.path,
        status.root,
        status.branch,
        if status.upstream.is_empty() { "-" } else { &status.upstream },
        status.ahead,
        status.behind,
        status.staged,
        status.unstaged,
        status.untracked,
        status.latest_commit,
        status.commits.join("\n")
    )
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars.saturating_sub(1) {
            out.push('…');
            return out;
        }
        out.push(ch);
    }
    out
}

fn push_text(
    cmds: &mut Vec<RenderCmd>,
    x: u16,
    y: u16,
    text: impl Into<String>,
    fg: [u8; 3],
    bold: bool,
) {
    cmds.push(RenderCmd::Text {
        x,
        y,
        text: text.into(),
        fg: Some(fg),
        bg: None,
        bold,
        modifiers: 0,
    });
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
    vec![("Developer".into(), "Open Git repository dashboard".into())]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: hints(),
        palette_commands: palette_commands(),
        request: app.pending_request.take(),
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
            Ok(HostMsg::Key { key, .. }) => app.handle_key(key),
            Ok(HostMsg::DbValue { key, value }) => {
                if key == DB_KEY {
                    if let Some(json) = value {
                        app.load(&json);
                    }
                    app.dirty = true;
                }
                false
            }
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Tick
                | HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::PaletteCommand { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[git-repository-dashboard] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "# branch.oid abc\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +2 -1\n1 M. N... 100644 100644 100644 a b file.rs\n1 .M N... 100644 100644 100644 a b lib.rs\n? new.txt\n";

    #[test]
    fn parses_branch_counts_and_changes() {
        let status = parse_status(STATUS);
        assert_eq!(status.branch, "main");
        assert_eq!(status.upstream, "origin/main");
        assert_eq!(status.ahead, 2);
        assert_eq!(status.behind, 1);
        assert_eq!(status.staged, 1);
        assert_eq!(status.unstaged, 1);
        assert_eq!(status.untracked, 1);
        assert!(status.is_dirty());
    }

    #[test]
    fn parses_detached_unknown_branch() {
        let status = parse_status("? file\n");
        assert_eq!(status.branch, "(detached or unknown)");
        assert_eq!(status.untracked, 1);
    }

    #[test]
    fn serializes_repository_config() {
        let mut app = App::default();
        app.repos.push(TrackedRepo {
            config: RepositoryConfig {
                name: "Santui".into(),
                path: ".".into(),
            },
            status: RepositoryStatus::default(),
        });
        let json = app.serialize();
        let state: PersistedState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.repositories.len(), 1);
        assert_eq!(state.repositories[0].name, "Santui");
    }

    #[test]
    fn filters_by_branch_and_path() {
        let mut app = App::default();
        app.repos.push(TrackedRepo {
            config: RepositoryConfig {
                name: "Santui".into(),
                path: "C:/repo/santui".into(),
            },
            status: RepositoryStatus {
                branch: "feature/x".into(),
                ..RepositoryStatus::default()
            },
        });
        app.search = "feature".into();
        app.apply_filter();
        assert_eq!(app.filtered, vec![0]);
    }

    #[test]
    fn error_status_is_not_dirty() {
        let status = status_error("not a git repository");
        assert!(!status.is_dirty());
        assert_eq!(status.error, "not a git repository");
    }
}
