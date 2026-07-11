use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, IpcKeyModifiers, ThemeData, BORDER_ALL};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct Node {
    id: String,
    text: String,
}

#[derive(Debug, Clone)]
struct Edge {
    from: String,
    to: String,
}

#[derive(Debug, Clone)]
struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<Value>,
    source: String,
    nodes: Vec<Node>,
    output_lines: Vec<String>,
    output_scroll: u16,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            source: String::from("A[Start] --> B[Process]\nB[Process] --> C[End]"),
            nodes: Vec::new(),
            output_lines: Vec::new(),
            output_scroll: 0,
            status: String::from("Press e to edit source, r to render"),
        }
    }
}

impl App {
    fn parse(&self) -> (Vec<Node>, Vec<Edge>) {
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();
        let mut node_map: HashMap<String, usize> = HashMap::new();

        for line in self.source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.split("-->").next() {
                let rest = rest.trim();
                if let Some(id_start) = rest.find('[') {
                    let id = rest[..id_start].trim().to_string();
                    let text_end = rest.rfind(']').unwrap_or(rest.len());
                    let text = rest[id_start + 1..text_end].trim().to_string();
                    if !node_map.contains_key(&id) {
                        node_map.insert(id.clone(), nodes.len());
                        nodes.push(Node {
                            id: id.clone(),
                            text,
                        });
                    }
                }
            }
            if let Some(parts) = line.split_once("-->") {
                let left = parts.0.trim();
                let right = parts.1.trim();
                let from_id = if let Some(bracket) = left.rfind('[') {
                    left[..bracket].trim().to_string()
                } else {
                    left.to_string()
                };
                let to_id = if let Some(bracket) = right.find('[') {
                    right[..bracket].trim().to_string()
                } else {
                    right.to_string()
                };
                if from_id.is_empty() || to_id.is_empty() {
                    continue;
                }
                if node_map.contains_key(&from_id) {
                    if let Some(bracket) = right.find('[') {
                        let text_end = right.rfind(']').unwrap_or(right.len());
                        let text = right[bracket + 1..text_end].trim().to_string();
                        if !node_map.contains_key(&to_id) {
                            node_map.insert(to_id.clone(), nodes.len());
                            nodes.push(Node {
                                id: to_id.clone(),
                                text,
                            });
                        }
                    } else if !node_map.contains_key(&to_id) {
                        node_map.insert(to_id.clone(), nodes.len());
                        nodes.push(Node {
                            id: to_id.clone(),
                            text: to_id.clone(),
                        });
                    }
                    edges.push(Edge {
                        from: from_id,
                        to: to_id,
                    });
                }
            }
        }
        (nodes, edges)
    }

    fn render_diagram(&self) -> Vec<String> {
        let (nodes, edges) = self.parse();
        if nodes.is_empty() {
            return vec![String::from("(no nodes parsed)")];
        }
        let node_order: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        let mut pos: HashMap<&str, usize> = HashMap::new();
        for (i, n) in node_order.iter().enumerate() {
            pos.insert(n, i);
        }
        let mut levels: HashMap<&str, usize> = HashMap::new();
        for n in &node_order {
            levels.insert(n, 0);
        }
        let mut changed = true;
        while changed {
            changed = false;
            for edge in &edges {
                let from_level = *levels.get(edge.from.as_str()).unwrap_or(&0);
                let to_level = levels.get(edge.to.as_str()).copied().unwrap_or(0);
                if to_level <= from_level {
                    levels.insert(edge.to.as_str(), from_level + 1);
                    changed = true;
                }
            }
        }
        let mut max_level = 0usize;
        for &lvl in levels.values() {
            if lvl > max_level {
                max_level = lvl;
            }
        }
        let mut by_level: Vec<Vec<&str>> = vec![Vec::new(); max_level + 1];
        for n in &node_order {
            let lvl = *levels.get(n).unwrap_or(&0);
            by_level[lvl].push(n);
        }

        let node_width: HashMap<&str, usize> = nodes
            .iter()
            .map(|n| {
                let w = n.text.len() + 4;
                (n.id.as_str(), w.max(8))
            })
            .collect();

        let mut buffer: Vec<String> = Vec::new();

        for (lvl, ids) in by_level.iter().enumerate() {
            let mut row = String::new();
            let mut mid_row = String::new();
            let mut bot_row = String::new();

            for (ci, id) in ids.iter().enumerate() {
                let w = *node_width.get(id).unwrap_or(&8);
                let node = nodes.iter().find(|n| n.id == *id).unwrap();

                let is_first = ci == 0;
                if !is_first {
                    row.push_str("   ");
                    mid_row.push_str("──►");
                    bot_row.push_str("   ");
                }

                let label = format!(" {} ", node.text);
                let pad = w.saturating_sub(label.len());
                let left_pad = pad / 2;
                let right_pad = pad - left_pad;

                row.push('\u{250C}');
                for _ in 0..w {
                    row.push('\u{2500}');
                }
                row.push('\u{2510}');

                mid_row.push('\u{2502}');
                for _ in 0..left_pad {
                    mid_row.push(' ');
                }
                mid_row.push_str(&label);
                for _ in 0..right_pad {
                    mid_row.push(' ');
                }
                mid_row.push('\u{2502}');

                bot_row.push('\u{2514}');
                for _ in 0..w {
                    bot_row.push('\u{2500}');
                }
                bot_row.push('\u{2518}');
            }
            buffer.push(row);
            buffer.push(mid_row);
            buffer.push(bot_row);

            if lvl < max_level {
                let next_ids = &by_level[lvl + 1];
                let mut conn_row = String::new();
                for (ci, id) in ids.iter().enumerate() {
                    let has_down = edges
                        .iter()
                        .any(|e| e.from == *id && next_ids.contains(&e.to.as_str()));
                    if ci > 0 {
                        conn_row.push_str("   ");
                    }
                    let w = *node_width.get(id).unwrap_or(&8);
                    let center = w / 2;
                    for j in 0..w {
                        let c = if j == center {
                            if has_down {
                                '\u{257C}'
                            } else {
                                ' '
                            }
                        } else if has_down {
                            '\u{2500}'
                        } else {
                            ' '
                        };
                        conn_row.push(c);
                    }
                    if has_down && ci + 1 < ids.len() {}
                }
                if !conn_row.trim().is_empty() {
                    buffer.push(conn_row);
                }
            }
        }

        buffer
    }

    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match key {
            IpcKey::Char('e') => {
                self.status =
                    String::from("Editing source. Type your diagram, then press r to render.");
                true
            }
            IpcKey::Char('r') => {
                self.output_lines = self.render_diagram();
                self.output_scroll = 0;
                self.status = format!("Rendered {} nodes", self.nodes.len());
                true
            }
            IpcKey::Char('c') => {
                self.source.clear();
                self.output_lines.clear();
                self.status = String::from("Cleared");
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.output_scroll = self.output_scroll.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.output_scroll = self.output_scroll.saturating_add(1);
                true
            }
            IpcKey::Esc => false,
            _ => true,
        }
    }

    fn render(&mut self) -> Vec<Value> {
        if !self.dirty && !self.cached_commands.is_empty() {
            return self.cached_commands.clone();
        }
        let mut cmds = Vec::new();
        let t = &self.theme;
        let w = self.area.w.max(48);
        let h = self.area.h.max(12);

        cmds.push(json!({"Rect": {
        "x": 0, "y": 0, "w": w, "h": h, "bg": t.background

        }}));
        cmds.push(json!({"Border": {
        "x": 0, "y": 0, "w": w, "h": h, "fg": t.border,
                    "borders": BORDER_ALL, "bg": t.background_panel,
                    "title": " Mermaid Renderer ",
                    "title_fg": t.text, "title_dash_fg": t.border

        }}));

        let input_h = 4u16;
        let output_y = input_h + 2;
        let output_h = h.saturating_sub(output_y + 2);

        cmds.push(json!({"Text": {
        "x": 2, "y": 1,
                    "text": String::from("Source (e to edit, r to render):"),
                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

        }}));

        for (i, line) in self.source.lines().enumerate().take(input_h as usize) {
            cmds.push(json!({"Text": {
            "x": 2, "y": 2 + i as u16,
                            "text": format!(" {}", line),
                            "fg": t.accent, "bg": null, "bold": false, "modifiers": 0

            }}));
        }

        cmds.push(json!({"Border": {
        "x": 1, "y": output_y, "w": w.saturating_sub(2),
                    "h": output_h, "fg": t.border, "borders": BORDER_ALL,
                    "bg": t.background, "title": Some(String::from(" Output ")),
                    "title_fg": Some(t.text), "title_dash_fg": Some(t.border)

        }}));

        let inner_x = 3u16;
        let inner_y = output_y + 1;
        let inner_h = output_h.saturating_sub(2);

        if self.output_lines.is_empty() {
            self.output_lines = self.render_diagram();
        }

        for (i, line) in self
            .output_lines
            .iter()
            .enumerate()
            .skip(self.output_scroll as usize)
            .take(inner_h as usize)
        {
            cmds.push(json!({"Text": {
            "x": inner_x,
                            "y": inner_y + i as u16 - self.output_scroll,
                            "text": line.clone(),
                            "fg": t.text, "bg": null, "bold": false, "modifiers": 0

            }}));
        }

        cmds.push(json!({"Text": {
        "x": 2, "y": h.saturating_sub(1),
                    "text": self.status.clone(),
                    "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

        }}));

        cmds.push(json!({"Text": {
"x": 2, "y": h,
            "text": String::from("e edit  \u{b7} r render  \u{b7} c clear  \u{b7} \u{2191}\u{2193} scroll  \u{b7} esc"),
            "fg": t.text_muted, "bg": null, "bold": false, "modifiers": 0

}}));

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
    json!([["Mermaid Renderer", "Render diagrams as Unicode art"]])
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
                log::error!("[mermaid-renderer] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
