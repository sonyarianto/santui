use std::io::{BufRead, BufReader};

use santui_ipc::protocol::{
    Area, HostMsg, IpcKey, IpcKeyModifiers, RenderCmd, TextStyle, ThemeData, BORDER_ALL,
};

#[derive(Debug, Clone)]
struct Email {
    from: String,
    subject: String,
    date: String,
    body: String,
}

fn mock_emails() -> Vec<Email> {
    vec![
        Email {
            from: "alice@example.com".into(),
            subject: "Weekend plans".into(),
            date: "2026-07-01".into(),
            body: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.".into(),
        },
        Email {
            from: "bob@corp.net".into(),
            subject: "Q3 budget review".into(),
            date: "2026-07-02".into(),
            body: "Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.".into(),
        },
        Email {
            from: "carol@friends.org".into(),
            subject: "Birthday party invite".into(),
            date: "2026-07-03".into(),
            body: "Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo.".into(),
        },
        Email {
            from: "dave@devops.io".into(),
            subject: "Deployment pipeline update".into(),
            date: "2026-07-04".into(),
            body: "Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt.".into(),
        },
        Email {
            from: "eve@startup.co".into(),
            subject: "Investor meeting notes".into(),
            date: "2026-07-05".into(),
            body: "At vero eos et accusamus et iusto odio dignissimos ducimus qui blanditiis praesentium voluptatum deleniti atque corrupti quos dolores et quas molestias excepturi sint occaecati cupiditate non provident.".into(),
        },
        Email {
            from: "frank@newsletter.com".into(),
            subject: "Your weekly digest".into(),
            date: "2026-07-06".into(),
            body: "Similique sunt in culpa qui officia deserunt mollitia animi, id est laborum et dolorum fuga. Et harum quidem rerum facilis est et expedita distinctio.".into(),
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Screen {
    Inbox,
    View,
}

struct App {
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    emails: Vec<Email>,
    cursor: usize,
    screen: Screen,
    status: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            emails: mock_emails(),
            cursor: 0,
            screen: Screen::Inbox,
            status: String::new(),
        }
    }
}

impl App {
    fn handle_key(&mut self, key: IpcKey, _modifiers: IpcKeyModifiers) -> bool {
        self.dirty = true;
        match self.screen {
            Screen::Inbox => self.handle_inbox_key(key),
            Screen::View => self.handle_view_key(key),
        }
    }

    fn handle_inbox_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.emails.len().saturating_sub(1);
                self.cursor = self.cursor.min(max).saturating_add(1).min(max);
                true
            }
            IpcKey::Enter => {
                if self.cursor < self.emails.len() {
                    self.screen = Screen::View;
                    self.status = String::new();
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_view_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Esc => {
                self.screen = Screen::Inbox;
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
        title: Some(" Email Client ".into()),
        title_fg: Some(t.text),
        title_dash_fg: Some(t.border),
        border_type: None,
    });

    match app.screen {
        Screen::Inbox => render_inbox(app, &mut cmds, t, w, h),
        Screen::View => render_view(app, &mut cmds, t, w, h),
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
    cmds
}

fn hints() -> Vec<(String, String)> {
    vec![
        ("↑↓/jk".into(), "navigate".into()),
        ("enter".into(), "open".into()),
        ("esc".into(), "back".into()),
    ]
}

fn render_inbox(app: &App, cmds: &mut Vec<RenderCmd>, t: &ThemeData, w: u16, _h: u16) {
    let rows: Vec<Vec<String>> = app
        .emails
        .iter()
        .map(|e| vec![e.date.clone(), e.from.clone(), e.subject.clone()])
        .collect();

    cmds.push(RenderCmd::Table {
        x: 2,
        y: 1,
        w: w.saturating_sub(4),
        h: app.emails.len() as u16 + 2,
        header: vec!["Date".into(), "From".into(), "Subject".into()],
        header_style: TextStyle {
            fg: Some(t.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![12, 24, w.saturating_sub(44)],
        selected: Some(app.cursor),
        style: TextStyle {
            fg: Some(t.text),
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
        current_row: None,
        current_style: None,
        cell_styles: None,
    });
}

fn render_view(app: &App, cmds: &mut Vec<RenderCmd>, t: &ThemeData, w: u16, h: u16) {
    if let Some(email) = app.emails.get(app.cursor) {
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 1,
            text: format!("From: {}", email.from),
            fg: Some(t.text),
            bg: None,
            bold: true,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 2,
            text: format!("Date: {}", email.date),
            fg: Some(t.text_muted),
            bg: None,
            bold: false,
            modifiers: 0,
        });
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 3,
            text: format!("Subject: {}", email.subject),
            fg: Some(t.accent),
            bg: None,
            bold: true,
            modifiers: 0,
        });

        let body_y = 5;
        let body_h = h.saturating_sub(body_y + 3) as usize;
        cmds.push(RenderCmd::Paragraph {
            x: 2,
            y: body_y,
            w: w.saturating_sub(4),
            h: body_h as u16,
            text: email.body.clone(),
            style: TextStyle {
                fg: Some(t.text),
                bg: None,
                bold: false,
                modifiers: 0,
            },
            wrap: true,
            spans: None,
            alignment: None,
        });
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

fn palette_commands() -> Vec<(String, String)> {
    vec![("Plugins".into(), "Open email client".into())]
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
                log::error!("[email-client] parse error: {e}: {trimmed}");
                false
            }
        };
        respond(&mut app, consumed);
        line.clear();
    }
}
