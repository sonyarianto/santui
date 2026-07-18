use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

struct Sheet {
    name: &'static str,
    items: &'static [(&'static str, &'static str)],
}

const SHEETS: &[Sheet] = &[
    Sheet {
        name: "Git",
        items: &[
            ("git init", "Initialize a new repository"),
            ("git status", "Show working tree status"),
            ("git add <file>", "Stage changes"),
            ("git commit -m", "Commit staged changes"),
            ("git push", "Push to remote"),
            ("git pull", "Fetch and merge remote"),
            ("git clone <url>", "Clone a repository"),
            ("git branch", "List branches"),
            ("git checkout -b", "Create and switch branch"),
            ("git merge <b>", "Merge branch into current"),
            ("git log --oneline", "Compact commit history"),
            ("git stash", "Temporarily stash changes"),
            ("git rebase -i", "Interactive rebase"),
            ("git reset --hard", "Discard all local changes"),
        ],
    },
    Sheet {
        name: "Vim",
        items: &[
            ("i", "Enter insert mode"),
            ("ESC", "Return to normal mode"),
            (":w", "Save file"),
            (":q", "Quit"),
            (":wq", "Save and quit"),
            ("dd", "Delete current line"),
            ("yy", "Yank (copy) line"),
            ("p", "Paste after cursor"),
            ("u", "Undo"),
            ("Ctrl-r", "Redo"),
            ("/", "Search forward"),
            ("n", "Next search match"),
            ("gg", "Go to first line"),
            ("G", "Go to last line"),
            ("dw", "Delete word"),
            ("ciw", "Change inner word"),
        ],
    },
    Sheet {
        name: "Docker",
        items: &[
            ("docker ps", "List running containers"),
            ("docker ps -a", "List all containers"),
            ("docker images", "List images"),
            ("docker build -t", "Build an image"),
            ("docker run -d", "Run container detached"),
            ("docker run -p", "Map host:container ports"),
            ("docker exec -it", "Run command in container"),
            ("docker logs", "Show container logs"),
            ("docker rm", "Remove a container"),
            ("docker rmi", "Remove an image"),
            ("docker compose up", "Start compose services"),
            ("docker stop", "Stop a container"),
        ],
    },
    Sheet {
        name: "Regex",
        items: &[
            (".", "Any single character"),
            ("\\d", "Digit [0-9]"),
            ("\\w", "Word char [A-Za-z0-9_]"),
            ("\\s", "Whitespace"),
            ("^", "Start of string"),
            ("$", "End of string"),
            ("*", "0 or more"),
            ("+", "1 or more"),
            ("?", "0 or 1"),
            ("{n,m}", "Between n and m"),
            ("[abc]", "Any of a, b, c"),
            ("[^a]", "Not a"),
            ("(a|b)", "a or b"),
            ("(...) ", "Capture group"),
            ("\\b", "Word boundary"),
        ],
    },
    Sheet {
        name: "Bash",
        items: &[
            ("ls -la", "List files with details"),
            ("cd <dir>", "Change directory"),
            ("pwd", "Print working directory"),
            ("cp -r", "Copy recursively"),
            ("mv", "Move/rename"),
            ("rm -rf", "Remove recursively (force)"),
            ("grep -r", "Recursively search text"),
            ("find . -name", "Find files by name"),
            ("chmod +x", "Make executable"),
            ("tar -czf", "Create gzip tarball"),
            ("cat", "Print file contents"),
            ("|", "Pipe output to next command"),
            ("&&", "Run only if previous succeeded"),
            ("export VAR=", "Set environment variable"),
            ("ssh user@host", "Connect over SSH"),
        ],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Sheets,
    Items,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    sheet_idx: usize,
    item_cursor: usize,
    sheet_cursor: usize,
    focus: Focus,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            sheet_idx: 0,
            item_cursor: 0,
            sheet_cursor: 0,
            focus: Focus::Sheets,
            status: "Tab focus \u{b7} \u{2191}\u{2193} navigate \u{b7} c copy line \u{b7} esc"
                .into(),
        }
    }
}

impl App {
    fn current_sheet(&self) -> &'static Sheet {
        &SHEETS[self.sheet_idx]
    }

    fn handle_key(&mut self, key: IpcKey, modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Tab => {
                self.focus = match self.focus {
                    Focus::Sheets => Focus::Items,
                    Focus::Items => Focus::Sheets,
                };
                true
            }
            IpcKey::Char('c') if !modifiers.ctrl => {
                let sheet = self.current_sheet();
                if let Some((k, v)) = sheet.items.get(self.item_cursor) {
                    let text = format!("{k}\t{v}");
                    match copy_to_clipboard(&text) {
                        Ok(()) => self.status = format!("Copied: {k}"),
                        Err(e) => self.status = format!("Clipboard error: {e}"),
                    }
                }
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                match self.focus {
                    Focus::Sheets => {
                        self.sheet_idx = self.sheet_idx.saturating_sub(1);
                        self.sheet_cursor = self.sheet_idx;
                        self.item_cursor = 0;
                    }
                    Focus::Items => {
                        self.item_cursor = self.item_cursor.saturating_sub(1);
                    }
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                match self.focus {
                    Focus::Sheets => {
                        if self.sheet_idx < SHEETS.len().saturating_sub(1) {
                            self.sheet_idx += 1;
                            self.sheet_cursor = self.sheet_idx;
                            self.item_cursor = 0;
                        }
                    }
                    Focus::Items => {
                        let max = self.current_sheet().items.len().saturating_sub(1);
                        self.item_cursor = self.item_cursor.min(max).saturating_add(1).min(max);
                    }
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(self);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn render_ui(app: &App) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let t = &app.theme;
    let w = app.area.w.max(48);
    let h = app.area.h.max(16);

    cmds.push(RenderCmd::Rect {
        x: 0,
        y: 0,
        w,
        h,
        bg: t.background,
    });
    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: t.border,
        borders: BORDER_ALL,
        bg: Some(t.background_panel),
        title: Some(" Cheat Sheet Browser ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    // Sheet tabs
    let tabs: Vec<String> = SHEETS
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == app.sheet_idx {
                format!("[{}]", s.name)
            } else {
                format!(" {} ", s.name)
            }
        })
        .collect();
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: tabs.join(" "),
        fg: Some(if app.focus == Focus::Sheets {
            t.accent
        } else {
            t.text_muted
        }),
        bg: None,
        bold: app.focus == Focus::Sheets,
        modifiers: 0,
    });

    let sheet = app.current_sheet();
    let list_y = 3;
    let list_h = h.saturating_sub(list_y + 2).max(1);
    let list_w = w.saturating_sub(4);

    let start = app.item_cursor.saturating_sub(list_h as usize / 2);
    let end = (start + list_h as usize).min(sheet.items.len());

    let items: Vec<String> = sheet.items[start..end]
        .iter()
        .map(|(k, v)| format!("{k:<16} {v}"))
        .collect();

    let vis_sel = if app.item_cursor >= start && app.item_cursor < end {
        Some(app.item_cursor - start)
    } else {
        None
    };

    cmds.push(RenderCmd::List {
        x: 2,
        y: list_y,
        w: list_w,
        h: list_h,
        items,
        selected: if app.focus == Focus::Items {
            vis_sel
        } else {
            None
        },
        style: TextStyle {
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(t.inverted_text),
            bg: Some(t.highlight),
            bold: true,
            modifiers: 0,
        },
    });

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: app.status.clone(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
    cmds
}

fn hints() -> Vec<(String, String)> {
    vec![
        ("tab".into(), "focus".into()),
        ("↑↓".into(), "navigate".into()),
        ("c".into(), "copy".into()),
        ("esc".into(), "back".into()),
    ]
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| e.to_string())
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
    vec![]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: hints(),
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
            Ok(HostMsg::Shutdown) => break,
            Ok(
                HostMsg::Tick
                | HostMsg::Focus
                | HostMsg::Blur
                | HostMsg::UserUpdate { .. }
                | HostMsg::DbValue { .. }
                | HostMsg::PluginMessage { .. }
                | HostMsg::Mouse { .. }
                | HostMsg::LogEntries { .. },
            ) => false,
            Err(e) => {
                log::error!("[cheatsheet-browser] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
