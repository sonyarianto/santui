use crate::plugin::{Plugin, PluginContext};
use crate::theme::Theme;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
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
    CmdItem {
        category: "Modules",
        label: "Radio Streaming Player",
    },
    CmdItem {
        category: "System",
        label: "Switch theme",
    },
    CmdItem {
        category: "System",
        label: "About",
    },
];

const PAL_W: u16 = 60;
const PAD_L: u16 = 2;
const PAD_T: u16 = 1;
const PAD_B: u16 = 1;
const HEADER_H: u16 = 4;

struct PaletteState {
    query: String,
    cursor: usize,
    scroll: u16,
}

pub struct Santui {
    plugins: Vec<Box<dyn Plugin>>,
    ctx: PluginContext,
    theme: Theme,
    themes: Vec<(&'static str, Theme)>,
    theme_idx: usize,
    active_plugin: Option<usize>,
    palette: Option<PaletteState>,
    show_about: bool,
    show_theme_picker: bool,
    theme_picker_query: String,
    theme_picker_cursor: usize,
    theme_picker_scroll: u16,
    theme_picker_orig_idx: usize,
    running: bool,
    tick: u64,
}

impl Default for Santui {
    fn default() -> Self {
        Self::new()
    }
}

impl Santui {
    pub fn new() -> Self {
        let themes = Theme::all();
        let theme = themes[1].1.clone();
        Santui {
            plugins: Vec::new(),
            ctx: PluginContext::new(),
            theme,
            themes,
            theme_idx: 1,
            active_plugin: None,
            palette: None,
            show_about: false,
            show_theme_picker: false,
            theme_picker_query: String::new(),
            theme_picker_cursor: 0,
            theme_picker_scroll: 0,
            theme_picker_orig_idx: 0,
            running: true,
            tick: 0,
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

        self.ctx.theme = self.theme.clone();
        for p in &mut self.plugins {
            p.init(&mut self.ctx)?;
        }

        let tick_rate = Duration::from_millis(100);

        while self.running {
            for p in &mut self.plugins {
                p.tick();
            }

            self.tick = self.tick.wrapping_add(1);

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

    fn palette_max_h(&self, area_h: u16) -> u16 {
        let content_h = area_h.saturating_sub(1);
        (content_h / 2).saturating_sub(6).max(4)
    }

    fn ensure_cursor_visible(&mut self, area_h: u16) {
        let query = self.palette.as_ref().unwrap().query.clone();
        let filtered = self.filtered_items(&query);
        let cursor = self.palette.as_ref().unwrap().cursor;
        let no_results = !query.is_empty() && filtered.is_empty();
        let mut line: u16 = 0;
        if no_results {
            line += 1;
        }
        let mut cat = String::new();
        let mut first_cat = true;
        for (flat, &idx) in filtered.iter().enumerate() {
            let c = CMD_ITEMS[idx].category;
            if c != cat {
                cat = c.to_string();
                if !first_cat {
                    line += 1; // blank before subsequent category
                }
                first_cat = false;
                line += 1; // category header
            }
            if flat == cursor {
                break;
            }
            line += 1; // item
        }
        let max_h = self.palette_max_h(area_h);
        let list_h = max_h.saturating_sub(6).max(1); // 6 = PAD_T(1) + HEADER_H(4) + pad_b(1)
        let pal = self.palette.as_mut().unwrap();
        if line < pal.scroll {
            pal.scroll = line;
        } else if line >= pal.scroll + list_h {
            pal.scroll = line.saturating_sub(list_h.saturating_sub(1));
        }
    }

    fn ensure_theme_cursor_visible(&mut self, area_h: u16) {
        let list_h = self.theme_picker_max_list_h(area_h);
        let cursor = self.theme_picker_cursor as u16;
        if cursor < self.theme_picker_scroll {
            self.theme_picker_scroll = cursor;
        } else if cursor >= self.theme_picker_scroll + list_h {
            self.theme_picker_scroll = cursor.saturating_sub(list_h.saturating_sub(1));
        }
    }

    fn theme_picker_max_list_h(&self, area_h: u16) -> u16 {
        let max_pal_h = area_h.saturating_sub(2).saturating_sub(1);
        max_pal_h.saturating_sub(PAD_T + HEADER_H + PAD_B).max(1)
    }

    fn filtered_items(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return (0..CMD_ITEMS.len()).collect();
        }
        let q = query.to_lowercase();
        CMD_ITEMS
            .iter()
            .enumerate()
            .filter(|(_, item)| item.label.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if self.palette.is_some() {
            let query = &self.palette.as_ref().unwrap().query;
            let filtered = self.filtered_items(query);
            let palette = self.palette.as_mut().unwrap();

            match key.code {
                KeyCode::Char(c)
                    if c == 'p'
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.palette = None;
                    return;
                }
                KeyCode::Char(_c) if !key.modifiers.is_empty() => {}
                KeyCode::Char(c) => {
                    palette.query.push(c);
                    palette.cursor = 0;
                    palette.scroll = 0;
                }
                KeyCode::Backspace => {
                    palette.query.pop();
                    palette.cursor = 0;
                    palette.scroll = 0;
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
                            "Radio Streaming Player" if !self.plugins.is_empty() => {
                                self.plugins[0].on_focus();
                                self.active_plugin = Some(0);
                            }
                            "Switch theme" => {
                                self.show_theme_picker = true;
                                self.theme_picker_query.clear();
                                self.theme_picker_cursor = self.theme_idx;
                                self.theme_picker_scroll = 0;
                                self.theme_picker_orig_idx = self.theme_idx;
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

            if self.palette.is_some() {
                let (_, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
                self.ensure_cursor_visible(term_h);
            }
            return;
        }

        if matches!(key.code, KeyCode::Char('p') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
        {
            self.palette = Some(PaletteState {
                query: String::new(),
                cursor: 0,
                scroll: 0,
            });
            return;
        }

        if self.show_theme_picker {
            let filtered = self.filtered_themes();
            match key.code {
                KeyCode::Char(c)
                    if c == 'p'
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.select_theme(self.theme_picker_orig_idx);
                    self.show_theme_picker = false;
                }
                KeyCode::Char(_) if !key.modifiers.is_empty() => {}
                KeyCode::Char(c) => {
                    self.theme_picker_query.push(c);
                    self.theme_picker_cursor = 0;
                    if let Some(&idx) = filtered.first() {
                        self.preview_theme(idx);
                    }
                }
                KeyCode::Backspace => {
                    self.theme_picker_query.pop();
                    self.theme_picker_cursor = 0;
                    let filtered = self.filtered_themes();
                    if let Some(&idx) = filtered.first() {
                        self.preview_theme(idx);
                    }
                }
                KeyCode::Up => {
                    if !filtered.is_empty() {
                        self.theme_picker_cursor = self.theme_picker_cursor.saturating_sub(1);
                        if let Some(&idx) = filtered.get(self.theme_picker_cursor) {
                            self.preview_theme(idx);
                        }
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() {
                        self.theme_picker_cursor =
                            (self.theme_picker_cursor + 1).min(filtered.len() - 1);
                        if let Some(&idx) = filtered.get(self.theme_picker_cursor) {
                            self.preview_theme(idx);
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(&idx) = filtered.get(self.theme_picker_cursor) {
                        self.select_theme(idx);
                    }
                    self.show_theme_picker = false;
                }
                KeyCode::Esc => {
                    self.select_theme(self.theme_picker_orig_idx);
                    self.show_theme_picker = false;
                }
                _ => {}
            }
            if self.show_theme_picker {
                let (_, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
                self.ensure_theme_cursor_visible(term_h);
            }
            return;
        }

        if self.show_about {
            if matches!(key.code, KeyCode::Esc) {
                self.show_about = false;
            }
            return;
        }

        match self.active_plugin {
            None => match key.code {
                KeyCode::Char('q') => self.running = false,
                KeyCode::Char('?') => self.show_about = true,
                _ => {}
            },
            Some(idx) => match key.code {
                KeyCode::Esc => {
                    self.plugins[idx].on_blur();
                    self.active_plugin = None;
                }
                _ => {
                    self.plugins[idx].handle_key(key);
                }
            },
        }
    }

    fn filtered_themes(&self) -> Vec<usize> {
        if self.theme_picker_query.is_empty() {
            return (0..self.themes.len()).collect();
        }
        let q = self.theme_picker_query.to_lowercase();
        self.themes
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    fn select_theme(&mut self, idx: usize) {
        self.theme_idx = idx;
        self.theme = self.themes[idx].1.clone();
        self.ctx.theme = self.theme.clone();
        for p in &mut self.plugins {
            p.on_theme_change(&self.theme);
        }
    }

    fn preview_theme(&mut self, idx: usize) {
        self.theme_idx = idx;
        self.theme = self.themes[idx].1.clone();
        self.ctx.theme = self.theme.clone();
    }

    fn render_dim_overlay(&self, f: &mut Frame, content: Rect) {
        let dim = Style::default()
            .fg(self.theme.text_muted)
            .add_modifier(Modifier::DIM);
        let fill: Vec<Line> = (0..content.height)
            .map(|_| Line::from(Span::styled(" ".repeat(content.width as usize), dim)))
            .collect();
        f.render_widget(Clear, content);
        f.render_widget(Paragraph::new(fill), content);
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

        if self.show_theme_picker {
            self.render_theme_picker(f, chunks[0]);
        }

        self.render_status_bar(f, chunks[1]);
    }

    fn render_splash(&self, f: &mut Frame, area: Rect) {
        let t = &self.theme;

        let logo: Vec<Line> = [
            "███████╗ █████╗ ███╗   ██╗████████╗██╗   ██╗██╗",
            "██╔════╝██╔══██╗████╗  ██║╚══██╔══╝██║   ██║██║",
            "███████╗███████║██╔██╗ ██║   ██║   ██║   ██║██║",
            "╚════██║██╔══██║██║╚██╗██║   ██║   ██║   ██║██║",
            "███████║██║  ██║██║ ╚████║   ██║   ╚██████╔╝██║",
            "╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝",
        ]
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(t.logo))))
        .collect::<Vec<_>>();

        let logo = logo
            .into_iter()
            .chain([
                Line::from(Span::styled(
                    "modular TUI platform",
                    Style::default().fg(t.text_muted),
                )),
                Line::from(Span::styled(
                    format!("v{VERSION}"),
                    Style::default().fg(t.text_muted),
                )),
            ])
            .collect::<Vec<_>>();

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(8),
                Constraint::Fill(1),
            ])
            .split(area);

        let p = Paragraph::new(logo).alignment(Alignment::Center);
        f.render_widget(p, vert[1]);
    }

    fn render_about(&self, f: &mut Frame, area: Rect) {
        let t = &self.theme;

        let text: Vec<Line> = [
            "███████╗ █████╗ ███╗   ██╗████████╗██╗   ██╗██╗",
            "██╔════╝██╔══██╗████╗  ██║╚══██╔══╝██║   ██║██║",
            "███████╗███████║██╔██╗ ██║   ██║   ██║   ██║██║",
            "╚════██║██╔══██║██║╚██╗██║   ██║   ██║   ██║██║",
            "███████║██║  ██║██║ ╚████║   ██║   ╚██████╔╝██║",
            "╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝",
        ]
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(t.logo))))
        .collect();

        let text = text
            .into_iter()
            .chain([
                Line::from(Span::styled(
                    "modular TUI platform",
                    Style::default().fg(t.text_muted),
                )),
                Line::from(Span::styled(
                    format!("v{VERSION}"),
                    Style::default().fg(t.text_muted),
                )),
                Line::from(""),
                Line::from("Copyright \u{00a9} Sony AK  <sony@sony-ak.com>"),
                Line::from(""),
                Line::from(Span::styled(
                    "https://santui.vercel.app",
                    Style::default().fg(t.text_muted),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press esc to go back",
                    Style::default().fg(t.text_muted),
                )),
            ])
            .collect::<Vec<_>>();

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(16),
                Constraint::Fill(1),
            ])
            .split(area);

        let p = Paragraph::new(text).alignment(Alignment::Center);
        f.render_widget(p, vert[1]);
    }

    fn render_theme_picker(&self, f: &mut Frame, content: Rect) {
        let t = &self.theme;
        let query = &self.theme_picker_query;
        let filtered = self.filtered_themes();
        let cursor = self.theme_picker_cursor;

        self.render_dim_overlay(f, content);

        let inner_w = PAL_W.saturating_sub(PAD_L * 2);

        let no_results = !query.is_empty() && filtered.is_empty();
        let list_items = if no_results { 1 } else { filtered.len() };

        let max_pal_h = content.height.saturating_sub(2);
        let pal_h = (PAD_T + HEADER_H + list_items as u16 + PAD_B).min(max_pal_h);
        let list_h = pal_h.saturating_sub(PAD_T + HEADER_H + PAD_B);

        let y = content.y + (content.height.saturating_sub(pal_h)) / 2;
        let x = (content.width.saturating_sub(PAL_W)) / 2;
        let pal_area = Rect {
            x,
            y,
            width: PAL_W,
            height: pal_h,
        };

        f.render_widget(Clear, pal_area);
        f.render_widget(
            Paragraph::new(vec![]).style(Style::default().bg(t.background_panel)),
            pal_area,
        );

        // Header: "Themes" + "esc"
        let pad_w = inner_w.saturating_sub(9); // 6 "Themes" + 3 "esc"
        let mut title_spans = vec![Span::styled(
            "Themes",
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        )];
        if pad_w > 0 {
            title_spans.push(Span::styled(" ".repeat(pad_w as usize), Style::default()));
        }
        title_spans.push(Span::styled("esc", Style::default().fg(t.text_muted)));

        let cursor_on = (self.tick / 5).is_multiple_of(2);

        let input_line = if query.is_empty() {
            let first_style = if cursor_on {
                Style::default().fg(Color::Black).bg(t.highlight)
            } else {
                Style::default().fg(t.text_muted)
            };
            Line::from(vec![
                Span::styled("S", first_style),
                Span::styled("earch", Style::default().fg(t.text_muted)),
            ])
        } else {
            let cursor_style = if cursor_on {
                Style::default().fg(Color::Black).bg(t.highlight)
            } else {
                Style::default()
                    .fg(t.background_panel)
                    .bg(t.background_panel)
            };
            Line::from(vec![
                Span::styled(query.clone(), Style::default().fg(t.text)),
                Span::styled(" ", cursor_style),
            ])
        };

        let header_lines = vec![
            Line::from(title_spans),
            Line::from(""),
            input_line,
            Line::from(""),
        ];

        let header_area = Rect {
            x: pal_area.x + PAD_L,
            y: pal_area.y + PAD_T,
            width: inner_w,
            height: HEADER_H,
        };
        f.render_widget(Paragraph::new(header_lines), header_area);

        // Theme list
        let mut list_lines = Vec::new();

        if no_results {
            list_lines.push(Line::from(Span::styled(
                "No results found",
                Style::default().fg(t.text_muted),
            )));
        }

        for (flat, &i) in filtered.iter().enumerate() {
            let (name, _) = &self.themes[i];
            let current = i == self.theme_idx;
            let hovered = flat == cursor;
            let prefix = if current { " ● " } else { "   " };
            let text_fg = if hovered {
                Color::Black
            } else if current {
                t.accent
            } else {
                t.text
            };
            let mut style = Style::default().fg(text_fg);
            if hovered {
                style = style.bg(t.highlight).add_modifier(Modifier::BOLD);
            } else if current {
                style = style.add_modifier(Modifier::BOLD);
            }
            let display = format!("{prefix}{name}");
            list_lines.push(Line::from(Span::styled(
                format!("{:<width$}", display, width = inner_w as usize),
                style,
            )));
        }

        let list_top = pal_area.y + PAD_T + HEADER_H;
        let list_area = Rect {
            x: pal_area.x + PAD_L,
            y: list_top,
            width: inner_w,
            height: list_h,
        };
        f.render_widget(
            Paragraph::new(list_lines).scroll((self.theme_picker_scroll, 0)),
            list_area,
        );
    }

    fn render_palette(&self, f: &mut Frame, content: Rect) {
        let t = &self.theme;
        let query = &self.palette.as_ref().unwrap().query;
        let filtered = self.filtered_items(query);
        let cursor = self.palette.as_ref().map_or(0, |p| p.cursor);
        let scroll = self.palette.as_ref().map_or(0, |p| p.scroll);

        self.render_dim_overlay(f, content);

        // Build category groups
        let mut current_cat = "";
        let mut cat_items: Vec<usize> = Vec::new();
        let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
        for &idx in &filtered {
            let cat = CMD_ITEMS[idx].category;
            if cat != current_cat && !cat_items.is_empty() {
                groups.push((current_cat, std::mem::take(&mut cat_items)));
            }
            current_cat = cat;
            cat_items.push(idx);
        }
        if !cat_items.is_empty() {
            groups.push((current_cat, cat_items));
        }

        let no_results = !query.is_empty() && filtered.is_empty();
        let inner_w = PAL_W.saturating_sub(PAD_L * 2);

        // Build list content
        let mut list_lines = Vec::new();

        if no_results {
            list_lines.push(Line::from(Span::styled(
                "No results found",
                Style::default().fg(t.text_muted),
            )));
        }

        let mut flat_idx = 0;
        for (i, (cat, items)) in groups.iter().enumerate() {
            if i > 0 {
                list_lines.push(Line::from(Span::styled("", Style::default())));
            }
            list_lines.push(Line::from(Span::styled(
                format!("{:<width$}", cat, width = inner_w as usize),
                Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
            )));
            for &idx in items {
                let sel = flat_idx == cursor;
                let item = &CMD_ITEMS[idx];
                let style = if sel {
                    Style::default().fg(Color::Black).bg(t.highlight)
                } else {
                    Style::default().fg(t.text)
                };
                list_lines.push(Line::from(Span::styled(
                    format!("{:<width$}", item.label, width = inner_w as usize),
                    style,
                )));
                flat_idx += 1;
            }
        }

        // Compute heights
        let max_h = (content.height / 2).saturating_sub(6).max(4);
        let natural_list_h = list_lines.len() as u16;
        let list_h = natural_list_h.min(max_h.saturating_sub(PAD_T + HEADER_H + PAD_B));
        let pal_h = PAD_T + HEADER_H + list_h + PAD_B;

        let x = (content.width.saturating_sub(PAL_W)) / 2;
        let y = content.y + (content.height.saturating_sub(pal_h)) / 2;
        let pal_area = Rect {
            x,
            y,
            width: PAL_W,
            height: pal_h,
        };

        // Dialog background
        f.render_widget(Clear, pal_area);
        f.render_widget(
            Paragraph::new(vec![]).style(Style::default().bg(t.background_panel)),
            pal_area,
        );

        // ---- Fixed header ----
        let mut header_lines = Vec::new();

        let pad_w = inner_w.saturating_sub(11); // 8 "Commands" + 3 "esc"
        let mut title_spans = vec![Span::styled(
            "Commands",
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        )];
        if pad_w > 0 {
            title_spans.push(Span::styled(" ".repeat(pad_w as usize), Style::default()));
        }
        title_spans.push(Span::styled("esc", Style::default().fg(t.text_muted)));
        header_lines.push(Line::from(title_spans));
        header_lines.push(Line::from(Span::styled("", Style::default())));

        let cursor_on = (self.tick / 5).is_multiple_of(2);

        if query.is_empty() {
            // Cursor sits ON the first character of placeholder (transparent overlay)
            let first_style = if cursor_on {
                Style::default().fg(Color::Black).bg(t.highlight)
            } else {
                Style::default().fg(t.text_muted)
            };
            header_lines.push(Line::from(vec![
                Span::styled("S", first_style),
                Span::styled("earch", Style::default().fg(t.text_muted)),
            ]));
        } else {
            // Cursor at end of query text
            let cursor_style = if cursor_on {
                Style::default().fg(Color::Black).bg(t.highlight)
            } else {
                Style::default()
                    .fg(t.background_panel)
                    .bg(t.background_panel)
            };
            header_lines.push(Line::from(vec![
                Span::styled(query.clone(), Style::default().fg(t.text)),
                Span::styled(" ", cursor_style),
            ]));
        }
        header_lines.push(Line::from(Span::styled("", Style::default())));

        let header_area = Rect {
            x: pal_area.x + PAD_L,
            y: pal_area.y + PAD_T,
            width: inner_w,
            height: HEADER_H,
        };
        f.render_widget(Paragraph::new(header_lines), header_area);

        // ---- Scrollable list ----
        let list_top = pal_area.y + PAD_T + HEADER_H;
        let list_area = Rect {
            x: pal_area.x + PAD_L,
            y: list_top,
            width: inner_w,
            height: list_h,
        };
        f.render_widget(Paragraph::new(list_lines).scroll((scroll, 0)), list_area);
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let t = &self.theme;
        let dim = Style::default().fg(t.text_muted);
        let key = Style::default().fg(t.text);

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
        } else if self.show_theme_picker {
            Line::from(vec![
                Span::styled("↑", key),
                Span::styled(" up • ", dim),
                Span::styled("↓", key),
                Span::styled(" down • ", dim),
                Span::styled("↵", key),
                Span::styled(" select • ", dim),
                Span::styled("esc", key),
                Span::styled(" back", dim),
            ])
        } else if self.show_about {
            Line::from(vec![Span::styled("esc", key), Span::styled(" close", dim)])
        } else if let Some(idx) = self.active_plugin {
            let plugin_hints = self.plugins[idx].status_hints();
            let mut spans: Vec<Span> = Vec::new();
            for (i, (hint_key, hint_desc)) in plugin_hints.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(" • ", dim));
                }
                spans.push(Span::styled(*hint_key, key));
                spans.push(Span::styled(format!(" {hint_desc}"), dim));
            }
            if !plugin_hints.is_empty() {
                spans.push(Span::styled(" • ", dim));
            }
            spans.push(Span::styled("ctrl+p", key));
            spans.push(Span::styled(" commands • ", dim));
            spans.push(Span::styled("esc", key));
            spans.push(Span::styled(" back • ", dim));
            spans.push(Span::styled("q", key));
            spans.push(Span::styled(" quit", dim));
            Line::from(spans)
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
