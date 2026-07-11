use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::Local;
use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, PluginRequest, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};
use serde::{Deserialize, Serialize};

const DB_KEY: &str = "package-version-monitor";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
    name: String,
    path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ecosystem {
    Rust,
    Node,
    Python,
}

impl Ecosystem {
    fn label(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Node => "node",
            Self::Python => "python",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageStatus {
    Current,
    Outdated,
    Unknown,
    Failed,
    Pending,
}

impl PackageStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Outdated => "outdated",
            Self::Unknown => "unknown",
            Self::Failed => "failed",
            Self::Pending => "pending",
        }
    }
}

#[derive(Debug, Clone)]
struct Dependency {
    project: String,
    name: String,
    requirement: String,
    latest: Option<String>,
    ecosystem: Ecosystem,
    status: PackageStatus,
    source_file: String,
    package_url: String,
    error: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedState {
    projects: Vec<Project>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    List,
    AddProject { field: ProjectField },
    ConfirmRemove(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectField {
    Name,
    Path,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    projects: Vec<Project>,
    dependencies: Vec<Dependency>,
    filtered: Vec<usize>,
    cursor: usize,
    search: String,
    screen: Screen,
    project_name: String,
    project_path: String,
    status: String,
    last_refreshed: String,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            theme: default_theme(),
            area: Area { w: 112, h: 30 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet { key: DB_KEY.into() }),
            projects: Vec::new(),
            dependencies: Vec::new(),
            filtered: Vec::new(),
            cursor: 0,
            search: String::new(),
            screen: Screen::List,
            project_name: String::new(),
            project_path: String::new(),
            status: "a add project · r scan · R refresh versions · / search · c copy package · o copy URL".into(),
            last_refreshed: "never".into(),
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
            Screen::AddProject { field } => self.handle_add_key(key, field),
            Screen::ConfirmRemove(idx) => self.handle_confirm_key(key, idx),
        }
    }

    fn handle_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char('a') => {
                self.project_name.clear();
                self.project_path.clear();
                self.screen = Screen::AddProject {
                    field: ProjectField::Name,
                };
                true
            }
            IpcKey::Char('r') => {
                self.scan_projects(false);
                true
            }
            IpcKey::Char('R') => {
                self.scan_projects(true);
                true
            }
            IpcKey::Char('d') => {
                if !self.projects.is_empty() {
                    self.screen = Screen::ConfirmRemove(0);
                }
                true
            }
            IpcKey::Char('/') => {
                self.search.clear();
                self.status = "Search packages/projects".into();
                true
            }
            IpcKey::Char('c') => {
                self.copy_selected(false);
                true
            }
            IpcKey::Char('o') => {
                self.copy_selected(true);
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

    fn handle_add_key(&mut self, key: IpcKey, field: ProjectField) -> bool {
        match key {
            IpcKey::Tab | IpcKey::BackTab => {
                self.screen = Screen::AddProject {
                    field: if field == ProjectField::Name {
                        ProjectField::Path
                    } else {
                        ProjectField::Name
                    },
                };
                true
            }
            IpcKey::Enter => {
                if let Err(e) = self.save_project() {
                    self.status = e;
                }
                true
            }
            IpcKey::Backspace => {
                self.project_buf_mut(field).pop();
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::List;
                true
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.project_buf_mut(field).push(ch);
                true
            }
            _ => false,
        }
    }

    fn handle_confirm_key(&mut self, key: IpcKey, idx: usize) -> bool {
        match key {
            IpcKey::Char('y') | IpcKey::Char('Y') => {
                if idx < self.projects.len() {
                    self.projects.remove(idx);
                }
                self.schedule_save();
                self.scan_projects(false);
                self.screen = Screen::List;
                true
            }
            IpcKey::Char('n') | IpcKey::Esc => {
                self.screen = Screen::List;
                true
            }
            _ => true,
        }
    }

    fn project_buf_mut(&mut self, field: ProjectField) -> &mut String {
        match field {
            ProjectField::Name => &mut self.project_name,
            ProjectField::Path => &mut self.project_path,
        }
    }
    fn save_project(&mut self) -> Result<(), String> {
        let name = self.project_name.trim().to_string();
        let path = self.project_path.trim().to_string();
        if name.is_empty() {
            return Err("Project name is required".into());
        }
        if path.is_empty() {
            return Err("Project path is required".into());
        }
        if !Path::new(&path).is_dir() {
            return Err("Project path must be an existing directory".into());
        }
        self.projects.push(Project { name, path });
        self.screen = Screen::List;
        self.schedule_save();
        self.scan_projects(false);
        Ok(())
    }

    fn scan_projects(&mut self, check_latest: bool) {
        let mut deps = Vec::new();
        for project in &self.projects {
            match scan_project(project) {
                Ok(mut found) => deps.append(&mut found),
                Err(e) => deps.push(Dependency {
                    project: project.name.clone(),
                    name: "(scan failed)".into(),
                    requirement: String::new(),
                    latest: None,
                    ecosystem: Ecosystem::Rust,
                    status: PackageStatus::Failed,
                    source_file: project.path.clone(),
                    package_url: String::new(),
                    error: e,
                }),
            }
        }
        if check_latest {
            for dep in &mut deps {
                refresh_latest(dep);
            }
        }
        self.dependencies = deps;
        self.last_refreshed = Local::now().format("%Y-%m-%d %H:%M").to_string();
        self.status = if check_latest {
            "Refreshed manifests and public registries".into()
        } else {
            "Scanned manifests; press R to contact registries".into()
        };
        self.apply_filter();
    }

    fn apply_filter(&mut self) {
        let query = self.search.trim().to_lowercase();
        self.filtered = self
            .dependencies
            .iter()
            .enumerate()
            .filter(|(_, dep)| {
                query.is_empty()
                    || dep.name.to_lowercase().contains(&query)
                    || dep.project.to_lowercase().contains(&query)
                    || dep.ecosystem.label().contains(&query)
            })
            .map(|(idx, _)| idx)
            .collect();
        self.cursor = self.cursor.min(self.filtered.len().saturating_sub(1));
    }

    fn selected_dep(&self) -> Option<&Dependency> {
        self.filtered
            .get(self.cursor)
            .map(|idx| &self.dependencies[*idx])
    }
    fn copy_selected(&mut self, url: bool) {
        if let Some(dep) = self.selected_dep() {
            let text = if url {
                dep.package_url.as_str()
            } else {
                dep.name.as_str()
            };
            match copy_to_clipboard(text) {
                Ok(()) => self.status = "Copied".into(),
                Err(e) => self.status = format!("Clipboard error: {e}"),
            }
        }
    }
    fn serialize(&self) -> String {
        serde_json::to_string(&PersistedState {
            projects: self.projects.clone(),
        })
        .unwrap_or_default()
    }
    fn load(&mut self, json: &str) {
        if let Ok(state) = serde_json::from_str::<PersistedState>(json) {
            self.projects = state.projects;
            self.scan_projects(false);
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

fn scan_project(project: &Project) -> Result<Vec<Dependency>, String> {
    let root = Path::new(&project.path);
    let mut deps = Vec::new();
    for path in find_manifest_files(root, 3)? {
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        match file_name {
            "Cargo.toml" => deps.extend(parse_cargo_toml(&text, project, &path)),
            "package.json" => deps.extend(parse_package_json(&text, project, &path)),
            "pyproject.toml" => deps.extend(parse_pyproject_toml(&text, project, &path)),
            "requirements.txt" => deps.extend(parse_requirements(&text, project, &path)),
            _ => {}
        }
    }
    Ok(deps)
}

fn find_manifest_files(root: &Path, max_depth: usize) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(root, 0, max_depth, &mut out)?;
    Ok(out)
}
fn visit(
    path: &Path,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if depth > max_depth {
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        if p.is_dir() {
            if !matches!(name, "target" | "node_modules" | ".git" | ".venv" | "venv") {
                visit(&p, depth + 1, max_depth, out)?;
            }
        } else if matches!(
            name,
            "Cargo.toml" | "package.json" | "pyproject.toml" | "requirements.txt"
        ) {
            out.push(p);
        }
    }
    Ok(())
}

fn parse_cargo_toml(text: &str, project: &Project, path: &Path) -> Vec<Dependency> {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(tbl) = table.get(section).and_then(|v| v.as_table()) {
            for (name, val) in tbl.iter() {
                if let Some(req) = dep_req_from_toml(val) {
                    deps.push(make_dep(project, name, &req, Ecosystem::Rust, path));
                }
            }
        }
    }
    deps
}
fn dep_req_from_toml(value: &toml::Value) -> Option<String> {
    value.as_str().map(str::to_string).or_else(|| {
        value
            .as_table()
            .and_then(|t| t.get("version"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    })
}
fn parse_package_json(text: &str, project: &Project, path: &Path) -> Vec<Dependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    for table in ["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(obj) = value.get(table).and_then(|v| v.as_object()) {
            for (name, val) in obj {
                if let Some(req) = val.as_str() {
                    deps.push(make_dep(project, name, req, Ecosystem::Node, path));
                }
            }
        }
    }
    deps
}
fn parse_pyproject_toml(text: &str, project: &Project, path: &Path) -> Vec<Dependency> {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    if let Some(arr) = table
        .get("project")
        .and_then(|p| p.as_table())
        .and_then(|t| t.get("dependencies"))
        .and_then(|v| v.as_array())
    {
        for item in arr {
            if let Some(spec) = item.as_str() {
                deps.push(make_python_dep(project, spec, path));
            }
        }
    }
    if let Some(tbl) = table
        .get("tool")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.as_table())
        .and_then(|p| p.get("dependencies"))
        .and_then(|v| v.as_table())
    {
        for (name, val) in tbl.iter() {
            if name != "python" {
                deps.push(make_dep(
                    project,
                    name,
                    &dep_req_from_toml(val).unwrap_or_else(|| "*".into()),
                    Ecosystem::Python,
                    path,
                ));
            }
        }
    }
    deps
}
fn parse_requirements(text: &str, project: &Project, path: &Path) -> Vec<Dependency> {
    text.lines()
        .filter_map(parse_requirement_line)
        .map(|(name, req)| make_dep(project, &name, &req, Ecosystem::Python, path))
        .collect()
}
fn parse_requirement_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.split('#').next()?.trim();
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return None;
    }
    for sep in ["==", ">=", "<=", "~=", ">", "<"] {
        if let Some((name, version)) = trimmed.split_once(sep) {
            return Some((name.trim().to_string(), format!("{sep}{}", version.trim())));
        }
    }
    Some((trimmed.to_string(), "*".into()))
}
fn make_python_dep(project: &Project, spec: &str, path: &Path) -> Dependency {
    let (name, req) = parse_requirement_line(spec).unwrap_or_else(|| (spec.into(), "*".into()));
    make_dep(project, &name, &req, Ecosystem::Python, path)
}
fn make_dep(
    project: &Project,
    name: &str,
    req: &str,
    ecosystem: Ecosystem,
    path: &Path,
) -> Dependency {
    Dependency {
        project: project.name.clone(),
        name: name.to_string(),
        requirement: req.to_string(),
        latest: None,
        ecosystem,
        status: PackageStatus::Pending,
        source_file: path.display().to_string(),
        package_url: package_url(ecosystem, name),
        error: String::new(),
    }
}
fn package_url(ecosystem: Ecosystem, name: &str) -> String {
    match ecosystem {
        Ecosystem::Rust => format!("https://crates.io/crates/{name}"),
        Ecosystem::Node => format!("https://www.npmjs.com/package/{name}"),
        Ecosystem::Python => format!("https://pypi.org/project/{name}/"),
    }
}
fn refresh_latest(dep: &mut Dependency) {
    let result = match dep.ecosystem {
        Ecosystem::Rust => latest_crate(&dep.name),
        Ecosystem::Node => latest_npm(&dep.name),
        Ecosystem::Python => latest_pypi(&dep.name),
    };
    match result {
        Ok(latest) => {
            dep.status = compare_versions(&dep.requirement, &latest);
            dep.latest = Some(latest);
        }
        Err(e) => {
            dep.status = PackageStatus::Failed;
            dep.error = e;
        }
    }
}
fn latest_crate(name: &str) -> Result<String, String> {
    let url = format!("https://crates.io/api/v1/crates/{name}");
    let mut resp = ureq::get(&url)
        .header("User-Agent", "santui")
        .call()
        .map_err(|e| e.to_string())?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    value["crate"]["newest_version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "missing newest_version".into())
}
fn latest_npm(name: &str) -> Result<String, String> {
    let encoded = name.replace('/', "%2f");
    let url = format!("https://registry.npmjs.org/{encoded}/latest");
    let mut resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    value["version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "missing version".into())
}
fn latest_pypi(name: &str) -> Result<String, String> {
    let url = format!("https://pypi.org/pypi/{name}/json");
    let mut resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    value["info"]["version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "missing version".into())
}
fn compare_versions(requirement: &str, latest: &str) -> PackageStatus {
    let Some(current) = normalized_exact_version(requirement) else {
        return PackageStatus::Unknown;
    };
    if current == latest {
        PackageStatus::Current
    } else {
        PackageStatus::Outdated
    }
}
fn normalized_exact_version(req: &str) -> Option<String> {
    let trimmed = req
        .trim()
        .trim_start_matches('=')
        .trim_start_matches('^')
        .trim_start_matches('~');
    if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
        && !trimmed.contains(['*', '>', '<', '|'])
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let theme = app.theme.clone();
    let w = app.area.w.max(80);
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
        title: Some(" Package Version Monitor ".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    match &app.screen {
        Screen::List => render_list(app, &mut cmds, &theme, w, h),
        Screen::AddProject { field } => render_add(app, &mut cmds, &theme, h, *field),
        Screen::ConfirmRemove(_) => {
            render_list(app, &mut cmds, &theme, w, h);
            cmds.push(RenderCmd::Dim {
                x: 0,
                y: 0,
                w,
                h,
                bg: theme.background_overlay,
            });
            push_text(
                &mut cmds,
                w / 2 - 20,
                h / 2,
                "Remove first project? y confirm · n/Esc cancel",
                theme.error,
                true,
            );
        }
    }
    cmds
}
fn render_list(app: &App, cmds: &mut Vec<RenderCmd>, theme: &ThemeData, w: u16, h: u16) {
    let header = format!(
        "Projects: {} · Packages: {} · Last refresh: {} · Search: {}",
        app.projects.len(),
        app.dependencies.len(),
        app.last_refreshed,
        app.search
    );
    push_text(
        cmds,
        2,
        2,
        truncate(&header, w as usize - 4),
        theme.text,
        true,
    );
    let list_h = h.saturating_sub(8).max(4);
    let detail_x = (w * 66 / 100).max(52);
    let rows: Vec<Vec<String>> = app
        .filtered
        .iter()
        .map(|idx| dep_row(&app.dependencies[*idx]))
        .collect();
    cmds.push(RenderCmd::Table {
        x: 2,
        y: 4,
        w: detail_x.saturating_sub(4),
        h: list_h,
        header: vec![
            "Pkg".into(),
            "Req".into(),
            "Latest".into(),
            "Eco".into(),
            "Status".into(),
        ],
        header_style: TextStyle {
            fg: Some(theme.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![18, 13, 12, 7, 9],
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
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
        border_type: None,
    });
    if let Some(dep) = app.selected_dep() {
        let detail = format!("{}\nProject: {}\nEcosystem: {}\nRequirement: {}\nLatest: {}\nStatus: {}\nSource: {}\nURL: {}\n{}", dep.name, dep.project, dep.ecosystem.label(), dep.requirement, dep.latest.as_deref().unwrap_or("-"), dep.status.label(), dep.source_file, dep.package_url, dep.error);
        cmds.push(RenderCmd::Paragraph {
            x: detail_x + 1,
            y: 5,
            w: w.saturating_sub(detail_x + 4),
            h: list_h.saturating_sub(2),
            text: detail,
            style: TextStyle {
                fg: Some(theme.text),
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
        ("r".into(), "scan local".into()),
        ("R".into(), "refresh registries".into()),
        ("/".into(), "search".into()),
        ("c".into(), "copy package".into()),
        ("o".into(), "copy URL".into()),
        ("d".into(), "remove first".into()),
        ("esc".into(), "back".into()),
    ]
}
fn render_add(
    app: &App,
    cmds: &mut Vec<RenderCmd>,
    theme: &ThemeData,
    h: u16,
    field: ProjectField,
) {
    push_text(
        cmds,
        2,
        2,
        "Add project directory · Tab field · Enter save · Esc cancel",
        theme.text,
        true,
    );
    push_text(
        cmds,
        2,
        4,
        format!(
            "{} Name: {}",
            if field == ProjectField::Name {
                ">"
            } else {
                " "
            },
            app.project_name
        ),
        theme.text,
        field == ProjectField::Name,
    );
    push_text(
        cmds,
        2,
        5,
        format!(
            "{} Path: {}",
            if field == ProjectField::Path {
                ">"
            } else {
                " "
            },
            app.project_path
        ),
        theme.text,
        field == ProjectField::Path,
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
fn dep_row(dep: &Dependency) -> Vec<String> {
    vec![
        dep.name.clone(),
        dep.requirement.clone(),
        dep.latest.clone().unwrap_or_else(|| "-".into()),
        dep.ecosystem.label().into(),
        dep.status.label().into(),
    ]
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
    vec![("Developer".into(), "Open package version monitor".into())]
}
fn respond(app: &mut App, consumed: bool) {
    let Ok(commands_val) = serde_json::to_value(app.render()) else {
        return;
    };
    let request = app.pending_request.take();
    let json = serde_json::json!({ "commands": commands_val, "hints": hints(), "palette_commands": palette_commands(), "request": request, "plugin_message": null, "consumed": consumed });
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
                log::error!("[package-version-monitor] parse error: {e}: {trimmed}");
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
    fn project() -> Project {
        Project {
            name: "Test".into(),
            path: ".".into(),
        }
    }
    #[test]
    fn parses_cargo_dependencies() {
        let deps = parse_cargo_toml(
            "[dependencies]\nserde = \"1\"\nfoo = { version = \"0.1\" }",
            &project(),
            Path::new("Cargo.toml"),
        );
        assert_eq!(deps.len(), 2);
        assert!(deps.iter().any(|d| d.name == "serde"));
    }
    #[test]
    fn parses_package_json() {
        let deps = parse_package_json(
            r#"{"dependencies":{"react":"18.2.0"},"devDependencies":{"vite":"^5"}}"#,
            &project(),
            Path::new("package.json"),
        );
        assert_eq!(deps.len(), 2);
    }
    #[test]
    fn parses_requirements() {
        assert_eq!(
            parse_requirement_line("requests==2.31.0 # ok").unwrap(),
            ("requests".into(), "==2.31.0".into())
        );
    }
    #[test]
    fn parses_pyproject() {
        let deps = parse_pyproject_toml(
            "[project]\ndependencies = [\"requests>=2\"]",
            &project(),
            Path::new("pyproject.toml"),
        );
        assert_eq!(deps[0].name, "requests");
    }
    #[test]
    fn compares_exact_versions() {
        assert_eq!(compare_versions("1.0.0", "1.0.0"), PackageStatus::Current);
        assert_eq!(compare_versions("1.0.0", "1.1.0"), PackageStatus::Outdated);
        assert_eq!(compare_versions(">=1", "2"), PackageStatus::Unknown);
    }
}
