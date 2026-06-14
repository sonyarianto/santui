use crate::plugin::{Plugin, PluginContext};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct CmdItem {
    category: &'static str,
    label: &'static str,
}

const CMD_ITEMS: &[CmdItem] = &[
    CmdItem { category: "Modules", label: "Radio Player" },
    CmdItem { category: "System", label: "About" },
];

struct PaletteState {
    query: String,
    cursor: usize,
}

pub struct Santui {
    plugins: Vec<Box<dyn Plugin>>,
    ctx: PluginContext,
    active_plugin: Option<usize>,
    palette: Option<PaletteState>,
    show_about: bool,
    running: bool,
}

impl Santui {
    pub fn new() -> Self {
        Santui {
            plugins: Vec::new(),
            ctx: PluginContext::new(),
            active_plugin: None,
            palette: None,
            show_about: false,
            running: true,
        }
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        for p in &mut self.plugins {
            p.init(&mut self.ctx)?;
        }

        let tick_rate = Duration::from_millis(100);

        while self.running {
            for p in &mut self.plugins {
                p.tick();
            }

            terminal.draw(|f| self.render(f))?;

            if crossterm::event::poll(tick_rate)? {
                if let Event::Key(key) = crossterm::event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    self.handle_key(key);
                }
            }
        }

        disable_raw_mode()?;
        execute!(std::io::stdout(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }

    fn filtered_items(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return (0..CMD_ITEMS.len()).collect();
        }
        let q = query.to_lowercase();
        CMD_ITEMS.iter().enumerate()
            .filter(|(_, item)| item.label.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if self.palette.is_some() {
            let query = self.palette.as_ref().unwrap().query.clone();
            let filtered = self.filtered_items(&query);
            let palette = self.palette.as_mut().unwrap();

            match key.code {
                KeyCode::Char(c) if c == 'p' && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    self.palette = None;
                    return;
                }
                KeyCode::Char(_c) if !key.modifiers.is_empty() => {}
                KeyCode::Char(c) => {
                    palette.query.push(c);
                    palette.cursor = 0;
                }
                KeyCode::Backspace => {
                    palette.query.pop();
                    palette.cursor = 0;
                }
                KeyCode::Up => {
                    if !filtered.is_empty() {
                        palette.cursor = palette.cursor.saturating_sub(1);
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() {
                        palette.cursor = (palette.cursor + 1).min(filtered.len() - 1);
                    }
                }
                KeyCode::Enter => {
                    if let Some(&idx) = filtered.get(palette.cursor) {
                        match CMD_ITEMS[idx].label {
                            "Radio Player" if !self.plugins.is_empty() => {
                                self.plugins[0].on_focus();
                                self.active_plugin = Some(0);
                            }
                            "About" => self.show_about = true,
                            _ => {}
                        }
                    }
                    self.palette = None;
                }
                KeyCode::Esc => self.palette = None,
                _ => {}
            }
            return;
        }

        if matches!(key.code, KeyCode::Char('p') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)) {
            self.palette = Some(PaletteState { query: String::new(), cursor: 0 });
            return;
        }

        if self.show_about {
            if matches!(key.code, KeyCode::Esc) {
                self.show_about = false;
            }
            return;
        }

        match self.active_plugin {
            None => {
                match key.code {
                    KeyCode::Char('q') => self.running = false,
                    KeyCode::Char('?') => self.show_about = true,
                    _ => {}
                }
            }
            Some(idx) => {
                match key.code {
                    KeyCode::Esc => {
                        self.plugins[idx].on_blur();
                        self.active_plugin = None;
                    }
                    _ => {
                        self.plugins[idx].handle_key(key);
                    }
                }
            }
        }
    }

    fn render(&self, f: &mut Frame) {
        let area = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        match self.active_plugin {
            None => {
                if self.show_about {
                    self.render_about(f, chunks[0]);
                } else {
                    self.render_splash(f, chunks[0]);
                }
            }
            Some(idx) => {
                self.plugins[idx].render(f, chunks[0]);
            }
        }

        if self.palette.is_some() {
            self.render_palette(f, chunks[0]);
        }

        self.render_status_bar(f, chunks[1]);
    }

    fn render_splash(&self, f: &mut Frame, area: Rect) {
        let gold = Color::Rgb(255, 185, 0);

        let logo: Vec<Line> = [
            "███████╗ █████╗ ███╗   ██╗████████╗██╗   ██╗██╗",
            "██╔════╝██╔══██╗████╗  ██║╚══██╔══╝██║   ██║██║",
            "███████╗███████║██╔██╗ ██║   ██║   ██║   ██║██║",
            "╚════██║██╔══██║██║╚██╗██║   ██║   ██║   ██║██║",
            "███████║██║  ██║██║ ╚████║   ██║   ╚██████╔╝██║",
            "╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝",
        ].iter().map(|line| {
            Line::from(Span::styled(*line, Style::default().fg(gold)))
        }).collect::<Vec<_>>();

        let logo = logo.into_iter().chain([
            Line::from(Span::styled("modular TUI platform", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled(format!("v{VERSION}"), Style::default().fg(Color::DarkGray))),
        ]).collect::<Vec<_>>();

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(8), Constraint::Fill(1)])
            .split(area);

        let p = Paragraph::new(logo).alignment(Alignment::Center);
        f.render_widget(p, vert[1]);
    }

    fn render_about(&self, f: &mut Frame, area: Rect) {
        let gold = Color::Rgb(255, 185, 0);

        let text: Vec<Line> = [
            "███████╗ █████╗ ███╗   ██╗████████╗██╗   ██╗██╗",
            "██╔════╝██╔══██╗████╗  ██║╚══██╔══╝██║   ██║██║",
            "███████╗███████║██╔██╗ ██║   ██║   ██║   ██║██║",
            "╚════██║██╔══██║██║╚██╗██║   ██║   ██║   ██║██║",
            "███████║██║  ██║██║ ╚████║   ██║   ╚██████╔╝██║",
            "╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝",
        ].iter().map(|line| {
            Line::from(Span::styled(*line, Style::default().fg(gold)))
        }).collect();

        let text = text.into_iter().chain([
            Line::from(Span::styled("modular TUI platform", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled(format!("v{VERSION}"), Style::default().fg(Color::DarkGray))),
            Line::from(""),
            Line::from("Copyright \u{00a9} Sony AK  <sony@sony-ak.com>"),
            Line::from(""),
            Line::from(Span::styled("Press esc to go back", Style::default().fg(Color::DarkGray))),
        ]).collect::<Vec<_>>();

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(13), Constraint::Fill(1)])
            .split(area);

        let p = Paragraph::new(text).alignment(Alignment::Center);
        f.render_widget(p, vert[1]);
    }

    fn render_palette(&self, f: &mut Frame, content: Rect) {
        let query = &self.palette.as_ref().unwrap().query;
        let filtered = self.filtered_items(query);
        let cursor = self.palette.as_ref().map_or(0, |p| p.cursor);

        let dim = Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM);
        let fill: Vec<Line> = (0..content.height)
            .map(|_| Line::from(Span::styled(" ".repeat(content.width as usize), dim)))
            .collect();
        f.render_widget(Clear, content);
        f.render_widget(Paragraph::new(fill), content);

        let item_count = filtered.len().max(1) as u16;
        let pal_w = (content.width as f32 * 0.5).max(40.0) as u16;
        let pal_h = item_count + 2;
        let x = content.x + (content.width - pal_w) / 2;
        let y = content.y + (content.height - pal_h) / 3;
        let pal_area = Rect { x, y, width: pal_w, height: pal_h };

        let max_label = filtered.iter()
            .map(|&i| CMD_ITEMS[i].label.len())
            .max()
            .unwrap_or(0);

        let mut lines = vec![
            Line::from(Span::styled(format!("> {}", query), Style::default().fg(Color::White))),
            Line::from(""),
        ];

        for (i, &idx) in filtered.iter().enumerate() {
            let item = &CMD_ITEMS[idx];
            let sel = i == cursor;
            let label = format!("  {}", item.label);
            let pad = (max_label + 5).saturating_sub(item.label.len());
            let content_line = format!("{}{}{}", label, " ".repeat(pad), item.category);
            let remaining = (pal_w as usize).saturating_sub(content_line.len());
            let full = format!("{}{}", content_line, " ".repeat(remaining));
            let style = if sel { Style::default().fg(Color::Black).bg(Color::Cyan) }
                         else { Style::default().fg(Color::White) };
            lines.push(Line::from(Span::styled(full, style)));
        }

        f.render_widget(Clear, pal_area);
        f.render_widget(
            Paragraph::new(lines).style(Style::default().bg(Color::Black)),
            pal_area,
        );
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let dim = Style::default().fg(Color::DarkGray);
        let key = Style::default().fg(Color::White);

        let line: Line = if self.palette.is_some() {
            Line::from(vec![
                Span::styled("↑", key),
                Span::styled(" up • ", dim),
                Span::styled("↓", key),
                Span::styled(" down • ", dim),
                Span::styled("↵", key),
                Span::styled(" enter • ", dim),
                Span::styled("esc", key),
                Span::styled(" close", dim),
            ])
        } else if self.show_about {
            Line::from(vec![
                Span::styled("esc", key),
                Span::styled(" close", dim),
            ])
        } else if self.active_plugin.is_some() {
            Line::from(vec![
                Span::styled("ctrl+p", key),
                Span::styled(" commands • ", dim),
                Span::styled("esc", key),
                Span::styled(" back • ", dim),
                Span::styled("q", key),
                Span::styled(" quit", dim),
            ])
        } else {
            Line::from(vec![
                Span::styled("ctrl+p", key),
                Span::styled(" commands • ", dim),
                Span::styled("?", key),
                Span::styled(" about • ", dim),
                Span::styled("q", key),
                Span::styled(" quit", dim),
            ])
        };

        let p = Paragraph::new(line);
        f.render_widget(p, area);

        let version = Line::from(vec![
            Span::styled("Santui ", key),
            Span::styled(VERSION, dim),
        ]);
        let version_para = Paragraph::new(version).alignment(Alignment::Right);
        f.render_widget(version_para, area);
    }
}
