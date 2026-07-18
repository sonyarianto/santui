use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct Column {
    name: String,
    col_type: String,
    is_pk: bool,
}

#[derive(Debug, Clone)]
struct Table {
    name: String,
    columns: Vec<Column>,
}

#[derive(Debug, Clone)]
struct Relationship {
    from_table: usize,
    to_table: usize,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    Diagram,
    AddTable,
    AddColumn { table_idx: usize },
    AddRelation,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    tables: Vec<Table>,
    relationships: Vec<Relationship>,
    cursor: usize,
    screen: Screen,
    input: String,
    input2: String,
    input3: String,
    #[allow(dead_code)]
    input4: String,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        let tables = vec![
            Table {
                name: "users".into(),
                columns: vec![
                    Column {
                        name: "id".into(),
                        col_type: "INTEGER".into(),
                        is_pk: true,
                    },
                    Column {
                        name: "name".into(),
                        col_type: "TEXT".into(),
                        is_pk: false,
                    },
                    Column {
                        name: "email".into(),
                        col_type: "TEXT".into(),
                        is_pk: false,
                    },
                ],
            },
            Table {
                name: "orders".into(),
                columns: vec![
                    Column {
                        name: "id".into(),
                        col_type: "INTEGER".into(),
                        is_pk: true,
                    },
                    Column {
                        name: "user_id".into(),
                        col_type: "INTEGER".into(),
                        is_pk: false,
                    },
                    Column {
                        name: "total".into(),
                        col_type: "REAL".into(),
                        is_pk: false,
                    },
                ],
            },
            Table {
                name: "products".into(),
                columns: vec![
                    Column {
                        name: "id".into(),
                        col_type: "INTEGER".into(),
                        is_pk: true,
                    },
                    Column {
                        name: "name".into(),
                        col_type: "TEXT".into(),
                        is_pk: false,
                    },
                    Column {
                        name: "price".into(),
                        col_type: "REAL".into(),
                        is_pk: false,
                    },
                ],
            },
        ];
        let relationships = vec![
            Relationship {
                from_table: 1,
                to_table: 0,
                label: "belongs_to".into(),
            },
            Relationship {
                from_table: 0,
                to_table: 2,
                label: "has_many".into(),
            },
        ];
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            tables,
            relationships,
            cursor: 0,
            screen: Screen::Diagram,
            input: String::new(),
            input2: String::new(),
            input3: String::new(),
            input4: String::new(),
            status: "a add table \u{b7} / add column \u{b7} r add relation \u{b7} d remove \u{b7} \u{2191}\u{2193}".into(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match self.screen.clone() {
            Screen::Diagram => self.handle_diagram_key(key),
            Screen::AddTable => self.handle_add_table_key(key),
            Screen::AddColumn { table_idx } => self.handle_add_column_key(key, table_idx),
            Screen::AddRelation => self.handle_add_relation_key(key),
        }
    }

    fn handle_diagram_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.tables.len().saturating_sub(1);
                self.cursor = self.cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Char('a') => {
                self.screen = Screen::AddTable;
                self.input.clear();
                self.status = "Enter table name:".into();
                true
            }
            IpcKey::Char('/') => {
                if !self.tables.is_empty() {
                    self.screen = Screen::AddColumn {
                        table_idx: self.cursor,
                    };
                    self.input.clear();
                    self.input2.clear();
                    self.status = format!(
                        "Add column to '{}': name,type,pk(y/n)",
                        self.tables[self.cursor].name
                    );
                }
                true
            }
            IpcKey::Char('r') => {
                if self.tables.len() >= 2 {
                    self.screen = Screen::AddRelation;
                    self.input.clear();
                    self.input2.clear();
                    self.input3.clear();
                    self.status = "Enter from_table_idx,to_table_idx,label:".into();
                }
                true
            }
            IpcKey::Char('d') => {
                if self.cursor < self.tables.len() {
                    let name = self.tables[self.cursor].name.clone();
                    self.tables.remove(self.cursor);
                    self.relationships
                        .retain(|r| r.from_table != self.cursor && r.to_table != self.cursor);
                    self.cursor = self.cursor.min(self.tables.len().saturating_sub(1));
                    self.status = format!("Removed table '{}'", name);
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_add_table_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Enter => {
                let name = self.input.trim().to_string();
                if !name.is_empty() {
                    self.tables.push(Table {
                        name,
                        columns: Vec::new(),
                    });
                    self.status = "Table added. Use / to add columns.".into();
                }
                self.screen = Screen::Diagram;
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::Diagram;
                true
            }
            IpcKey::Backspace => {
                self.input.pop();
                true
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.input.push(ch);
                true
            }
            _ => true,
        }
    }

    fn handle_add_column_key(&mut self, key: IpcKey, table_idx: usize) -> bool {
        match key {
            IpcKey::Enter => {
                if let Some(table) = self.tables.get_mut(table_idx) {
                    let parts: Vec<&str> = self.input.splitn(3, ',').collect();
                    if parts.len() == 3 {
                        let name = parts[0].trim().to_string();
                        let col_type = parts[1].trim().to_string();
                        let is_pk = parts[2].trim().eq_ignore_ascii_case("y");
                        if !name.is_empty() {
                            table.columns.push(Column {
                                name,
                                col_type,
                                is_pk,
                            });
                            self.status = format!("Column added to '{}'", table.name);
                        }
                    }
                }
                self.screen = Screen::Diagram;
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::Diagram;
                true
            }
            IpcKey::Backspace => {
                self.input.pop();
                true
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.input.push(ch);
                true
            }
            _ => true,
        }
    }

    fn handle_add_relation_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Enter => {
                let parts: Vec<&str> = self.input.splitn(3, ',').collect();
                if parts.len() == 3 {
                    let from = parts[0].trim().parse::<usize>().unwrap_or(0);
                    let to = parts[1].trim().parse::<usize>().unwrap_or(0);
                    let label = parts[2].trim().to_string();
                    if from < self.tables.len() && to < self.tables.len() {
                        self.relationships.push(Relationship {
                            from_table: from,
                            to_table: to,
                            label,
                        });
                        self.status = format!(
                            "Relation added: {} -> {}",
                            self.tables[from].name, self.tables[to].name
                        );
                    }
                }
                self.screen = Screen::Diagram;
                true
            }
            IpcKey::Esc => {
                self.screen = Screen::Diagram;
                true
            }
            IpcKey::Backspace => {
                self.input.pop();
                true
            }
            IpcKey::Char(ch) if !ch.is_control() => {
                self.input.push(ch);
                true
            }
            _ => true,
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
    let w = app.area.w.max(60);
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
        title: Some(" ERD Sketch ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    match app.screen {
        Screen::Diagram => render_diagram(app, &mut cmds, t, w, h),
        Screen::AddTable => render_input(app, &mut cmds, t, w, h, "Add Table", &app.input),
        Screen::AddColumn { .. } => render_input(
            app,
            &mut cmds,
            t,
            w,
            h,
            "Add Column (name,type,pk(y/n))",
            &app.input,
        ),
        Screen::AddRelation => render_input(
            app,
            &mut cmds,
            t,
            w,
            h,
            "Add Relation (from_idx,to_idx,label)",
            &app.input,
        ),
    }

    cmds
}

fn render_diagram(app: &App, cmds: &mut Vec<RenderCmd>, t: &ThemeData, _w: u16, h: u16) {
    let mut y = 1;
    for (i, table) in app.tables.iter().enumerate() {
        let selected = i == app.cursor && app.screen == Screen::Diagram;
        let fg = if selected { t.highlight } else { t.text };
        let prefix = if selected { ">" } else { " " };
        let pk_cols: Vec<&str> = table
            .columns
            .iter()
            .filter(|c| c.is_pk)
            .map(|c| c.name.as_str())
            .collect();
        let pk_str = if pk_cols.is_empty() {
            String::new()
        } else {
            format!(" (PK: {})", pk_cols.join(", "))
        };

        cmds.push(RenderCmd::Text {
            x: 2,
            y,
            text: format!("{}[{}]{} {}", prefix, i, table.name, pk_str),
            fg: Some(fg),
            bg: None,
            bold: true,
            modifiers: 0,
        });
        y += 1;

        for col in &table.columns {
            let pk_mark = if col.is_pk { "\u{1f511} " } else { "  " };
            cmds.push(RenderCmd::Text {
                x: 6,
                y,
                text: format!("{}{}: {}", pk_mark, col.name, col.col_type),
                fg: Some(t.text_muted),
                bg: None,
                bold: false,
                modifiers: 0,
            });
            y += 1;
        }
        y += 1;
    }

    if !app.relationships.is_empty() {
        cmds.push(RenderCmd::Text {
            x: 2,
            y,
            text: "Relationships:".into(),
            fg: Some(t.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        });
        y += 1;
        for rel in &app.relationships {
            if rel.from_table < app.tables.len() && rel.to_table < app.tables.len() {
                cmds.push(RenderCmd::Text {
                    x: 4,
                    y,
                    text: format!(
                        "{} --({})--> {}",
                        app.tables[rel.from_table].name, rel.label, app.tables[rel.to_table].name
                    ),
                    fg: Some(t.text_muted),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                });
                y += 1;
            }
        }
    }

    cmds.push(RenderCmd::Text {
        x: 2,
        y: h.saturating_sub(2),
        text: app.status.clone(),
        fg: Some(t.text_muted),
        bg: None,
        bold: false,
        modifiers: 0,
    });
}

fn hints() -> Vec<(String, String)> {
    vec![
        ("a".into(), "table".into()),
        ("/".into(), "column".into()),
        ("r".into(), "relation".into()),
        ("d".into(), "remove".into()),
        ("up/down".into(), "navigate".into()),
        ("esc".into(), "back".into()),
    ]
}

fn render_input(
    _app: &App,
    cmds: &mut Vec<RenderCmd>,
    t: &ThemeData,
    _w: u16,
    _h: u16,
    title: &str,
    input: &str,
) {
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 1,
        text: title.into(),
        fg: Some(t.accent),
        bg: None,
        bold: true,
        modifiers: 0,
    });
    cmds.push(RenderCmd::Text {
        x: 2,
        y: 3,
        text: format!("> {}", input),
        fg: Some(t.text),
        bg: None,
        bold: false,
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
    vec![("Developer".into(), "Open ERD diagram sketcher".into())]
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
                log::error!("[erd-sketch] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
