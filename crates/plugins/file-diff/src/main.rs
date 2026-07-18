use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffLine {
    Same,
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone)]
struct DiffEntry {
    left: String,
    right: String,
    kind: DiffLine,
    #[allow(dead_code)]
    line_no: usize,
}

fn mock_left() -> Vec<&'static str> {
    vec![
        "import React from 'react';",
        "import { useState } from 'react';",
        "",
        "function App() {",
        "  const [count, setCount] = useState(0);",
        "",
        "  return (",
        "    <div className=\"app\">",
        "      <h1>Hello World</h1>",
        "      <p>Count: {count}</p>",
        "      <button onClick={() => setCount(c => c + 1)}>",
        "        Increment",
        "      </button>",
        "    </div>",
        "  );",
        "}",
        "",
        "export default App;",
    ]
}

fn mock_right() -> Vec<&'static str> {
    vec![
        "import React, { useState, useEffect } from 'react';",
        "import { useCallback } from 'react';",
        "",
        "function App() {",
        "  const [count, setCount] = useState(0);",
        "  const [name, setName] = useState('');",
        "",
        "  const increment = useCallback(() => {",
        "    setCount(c => c + 1);",
        "  }, []);",
        "",
        "  return (",
        "    <div className=\"app\">",
        "      <h1>Hello World</h1>",
        "      <input value={name} onChange={e => setName(e.target.value)} />",
        "      <p>Count: {count}</p>",
        "      <button onClick={increment}>",
        "        Increment",
        "      </button>",
        "    </div>",
        "  );",
        "}",
        "",
        "export default App;",
    ]
}

fn compute_diff(left: &[&str], right: &[&str]) -> Vec<DiffEntry> {
    let max_lines = left.len().max(right.len());
    let mut entries = Vec::new();
    let mut i = 0;
    while i < max_lines {
        let l = if i < left.len() { left[i] } else { "" };
        let r = if i < right.len() { right[i] } else { "" };
        let kind = if i >= left.len() {
            DiffLine::Added
        } else if i >= right.len() {
            DiffLine::Removed
        } else if l == r {
            DiffLine::Same
        } else {
            DiffLine::Changed
        };
        entries.push(DiffEntry {
            left: l.into(),
            right: r.into(),
            kind,
            line_no: i + 1,
        });
        i += 1;
    }
    entries
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    diff: Vec<DiffEntry>,
    scroll: usize,
    #[allow(dead_code)]
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let left = mock_left();
        let right = mock_right();
        let diff = compute_diff(&left, &right);
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            diff,
            scroll: 0,
            status: String::new(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.scroll = self.scroll.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max_scroll = self.diff.len().saturating_sub(1);
                self.scroll = self
                    .scroll
                    .min(max_scroll)
                    .saturating_add(1)
                    .min(max_scroll);
                true
            }
            IpcKey::PageUp => {
                let vis = self.visible_lines();
                self.scroll = self.scroll.saturating_sub(vis);
                true
            }
            IpcKey::PageDown => {
                let vis = self.visible_lines();
                let max_scroll = self.diff.len().saturating_sub(1);
                self.scroll = (self.scroll + vis).min(max_scroll);
                true
            }
            IpcKey::Home => {
                self.scroll = 0;
                true
            }
            IpcKey::End => {
                self.scroll = self.diff.len().saturating_sub(1);
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn visible_lines(&self) -> usize {
        (self.area.h.max(12) as usize).saturating_sub(5)
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
    let w = app.area.w.max(72);
    let h = app.area.h.max(14);

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
        title: Some(" File Diff Viewer ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    // Header
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: format!(
            "{} lines, showing {}-{}",
            app.diff.len(),
            app.scroll + 1,
            (app.scroll + app.visible_lines()).min(app.diff.len())
        ),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let mid = w / 2;
    let col_w = (mid.saturating_sub(3)).max(20);

    // Column headers
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 2,
        text: " Left (original)".into(),
        fg: Some(t.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: mid + 1,
        y: 2,
        text: " Right (modified)".into(),
        fg: Some(t.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });

    // Divider
    cmds.push(RenderCmd::Text {
        x: mid,
        y: 2,
        text: "\u{2502}".into(),
        fg: Some(t.border),
        bg: None,
        bold: false,
        modifiers: 0,
    });

    let vis = app.visible_lines();
    for i in 0..vis {
        let idx = app.scroll + i;
        if idx >= app.diff.len() {
            break;
        }
        let entry = &app.diff[idx];
        let y = 3 + i as u16;

        // Determine color based on diff kind
        let (left_fg, right_fg) = match entry.kind {
            DiffLine::Same => (t.text, t.text),
            DiffLine::Added => (t.text_muted, t.success),
            DiffLine::Removed => (t.error, t.text_muted),
            DiffLine::Changed => (t.highlight, t.highlight),
        };

        let left_bg = match entry.kind {
            DiffLine::Removed => Some(t.background_overlay),
            _ => None,
        };
        let right_bg = match entry.kind {
            DiffLine::Added => Some(t.background_overlay),
            _ => None,
        };

        let left_text = if entry.left.len() > col_w as usize {
            format!("{}...", &entry.left[..col_w as usize - 3])
        } else {
            format!("{:width$}", entry.left, width = col_w as usize)
        };
        let right_text = if entry.right.len() > col_w as usize {
            format!("{}...", &entry.right[..col_w as usize - 3])
        } else {
            format!("{:width$}", entry.right, width = col_w as usize)
        };

        // Line number marker
        let marker = match entry.kind {
            DiffLine::Same => " ",
            DiffLine::Added => "+",
            DiffLine::Removed => "-",
            DiffLine::Changed => "~",
        };

        cmds.push(RenderCmd::Text {
            x: 1,
            y,
            text: marker.into(),
            fg: Some(match entry.kind {
                DiffLine::Added => t.success,
                DiffLine::Removed => t.error,
                DiffLine::Changed => t.highlight,
                DiffLine::Same => t.text_muted,
            }),
            bg: None,
            bold: true,
            modifiers: 0,
        });

        cmds.push(RenderCmd::Text {
            x: 2,
            y,
            text: left_text,
            fg: Some(left_fg),
            bg: left_bg,
            bold: false,
            modifiers: 0,
        });

        cmds.push(RenderCmd::Text {
            x: mid,
            y,
            text: "\u{2502}".into(),
            fg: Some(t.border),
            bg: None,
            bold: false,
            modifiers: 0,
        });

        cmds.push(RenderCmd::Text {
            x: mid + 1,
            y,
            text: right_text,
            fg: Some(right_fg),
            bg: right_bg,
            bold: false,
            modifiers: 0,
        });
    }

    cmds.push(RenderCmd::Text {
        x: 2, y: h.saturating_sub(2),
        text: "- removed \u{b7} + added \u{b7} ~ changed  \u{b7}  \u{2191}\u{2193}/jk scroll \u{b7} pgup/pgdn page".into(),
        fg: Some(t.text_muted), bg: None, bold: false, modifiers: 0,
    });
    cmds
}

fn hints() -> Vec<(String, String)> {
    vec![
        ("home/end".into(), "jump".into()),
        ("esc".into(), "back".into()),
    ]
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
    vec![("Developer".into(), "Open file diff viewer".into())]
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
            Ok(HostMsg::Tick) => false,
            Ok(HostMsg::PaletteCommand { .. }) => {
                app.dirty = true;
                true
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
                log::error!("[file-diff] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
