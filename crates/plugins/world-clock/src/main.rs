mod state;
mod timezones;
mod ui;

use std::io::{BufRead, BufReader, Write};

use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};

use state::{Screen, WorldTimeState};
use ui::render_ui;

struct App {
    state: WorldTimeState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: WorldTimeState::default(),
            theme: ThemeData {
                text: [220; 3],
                text_muted: [140; 3],
                accent: [180; 3],
                highlight: [220; 3],
                logo: [255; 3],
                background: [0; 3],
                background_panel: [20; 3],
                background_overlay: [10; 3],
                border: [150; 3],
                success: [0; 3],
                error: [255; 3],
                inverted_text: [255; 3],
            },
            area: Area { w: 80, h: 24 },
            dirty: true,
            cached_commands: Vec::new(),
            pending_request: Some(PluginRequest::DbGet {
                key: "clocks".into(),
            }),
        }
    }
}

impl App {
    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.dirty = true;
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        match &self.state.screen {
            Screen::Search => self.handle_search_key(key),
            Screen::Rename(_) => self.handle_rename_key(key),
            Screen::Detail(idx) => self.handle_detail_key(key, *idx),
            Screen::Grid => self.handle_grid_key(key),
        }
    }

    fn handle_grid_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Right | IpcKey::Char('l') => {
                if !self.state.clocks.is_empty() {
                    if self.state.selected + 1 < self.state.clocks.len() {
                        self.state.selected += 1;
                    } else {
                        self.state.selected = 0;
                    }
                    self.dirty = true;
                }
                true
            }
            IpcKey::Left | IpcKey::Char('h') => {
                if !self.state.clocks.is_empty() {
                    if self.state.selected > 0 {
                        self.state.selected -= 1;
                    } else {
                        self.state.selected = self.state.clocks.len() - 1;
                    }
                    self.dirty = true;
                }
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                if !self.state.clocks.is_empty() {
                    let cols = grid_cols();
                    self.state.selected =
                        (self.state.selected + cols).min(self.state.clocks.len() - 1);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                if !self.state.clocks.is_empty() {
                    let cols = grid_cols();
                    self.state.selected = self.state.selected.saturating_sub(cols);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Enter => {
                if !self.state.clocks.is_empty() {
                    let idx = self.state.selected;
                    self.state.screen = Screen::Detail(idx);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Char('a') => {
                self.state.screen = Screen::Search;
                self.state.search_query.clear();
                self.state.apply_search();
                self.dirty = true;
                true
            }
            IpcKey::Char('d') => {
                if !self.state.clocks.is_empty() {
                    self.state.remove_selected();
                    self.save();
                    self.dirty = true;
                }
                true
            }
            IpcKey::Char('r') => {
                if !self.state.clocks.is_empty() {
                    let idx = self.state.selected;
                    self.state.rename_buf = self.state.clocks[idx].label.clone();
                    self.state.screen = Screen::Rename(idx);
                    self.dirty = true;
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_detail_key(&mut self, key: IpcKey, _idx: usize) -> bool {
        match key {
            IpcKey::Esc | IpcKey::Tab => {
                self.state.screen = Screen::Grid;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_search_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Esc => {
                self.state.screen = Screen::Grid;
                self.state.search_query.clear();
                self.state.search_results.clear();
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                if let Some(&tz) = self.state.search_results.get(self.state.search_cursor) {
                    self.state.add_clock(tz);
                    self.save();
                }
                self.state.screen = Screen::Grid;
                self.state.search_query.clear();
                self.state.search_results.clear();
                self.dirty = true;
                true
            }
            IpcKey::Up => {
                self.state.search_cursor = self.state.search_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down => {
                let max = self.state.search_results.len().saturating_sub(1);
                self.state.search_cursor =
                    self.state.search_cursor.min(max).saturating_add(1).min(max);
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.search_query.pop();
                self.state.apply_search();
                self.dirty = true;
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.state.search_query.push(c);
                self.state.apply_search();
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_rename_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Esc => {
                self.state.screen = Screen::Grid;
                self.state.rename_buf.clear();
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                let buf = self.state.rename_buf.trim().to_string();
                if !buf.is_empty() {
                    if let Screen::Rename(idx) = self.state.screen {
                        self.state.clocks[idx].label = buf;
                        self.save();
                    }
                }
                self.state.screen = Screen::Grid;
                self.state.rename_buf.clear();
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.rename_buf.pop();
                self.dirty = true;
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.state.rename_buf.push(c);
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_tick(&mut self) {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        if now_secs != self.state.last_second {
            self.state.last_second = now_secs;
            self.dirty = true;
        }
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "clocks" {
            match value {
                Some(json) => self.state.load_clocks(&json),
                None => {
                    self.state.clocks = WorldTimeState::default_clocks();
                    self.save();
                }
            }
            self.dirty = true;
        }
    }

    fn save(&mut self) {
        let json = self.state.serialize_clocks();
        self.pending_request = Some(PluginRequest::DbSet {
            key: "clocks".into(),
            value: json,
        });
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match &self.state.screen {
            Screen::Grid => {
                let mut hints = vec![
                    ("↑↓←→".into(), "nav".into()),
                    ("enter".into(), "detail".into()),
                    ("a".into(), "add".into()),
                ];
                if !self.state.clocks.is_empty() {
                    hints.push(("d".into(), "delete".into()));
                    hints.push(("r".into(), "rename".into()));
                }
                hints
            }
            Screen::Detail(_) => vec![("tab".into(), "next".into()), ("esc".into(), "back".into())],
            Screen::Search => vec![
                ("↵".into(), "add".into()),
                ("↑↓".into(), "nav".into()),
                ("⌫".into(), "del".into()),
                ("esc".into(), "cancel".into()),
            ],
            Screen::Rename(_) => vec![("↵".into(), "save".into()), ("esc".into(), "cancel".into())],
        }
    }

    fn render(&mut self) -> &[RenderCmd] {
        if self.dirty || self.cached_commands.is_empty() {
            self.cached_commands = render_ui(&self.state, &self.theme, self.area.w, self.area.h);
            self.dirty = false;
        }
        &self.cached_commands
    }
}

fn palette_commands() -> Vec<(String, String)> {
    vec![("World Clock".into(), "Grid".into())]
}

fn grid_cols() -> usize {
    4
}

fn respond(app: &mut App, consumed: bool) {
    let commands_val = match serde_json::to_value(app.render()) {
        Ok(v) => v,
        Err(e) => {
            log::error!("failed to serialize render commands: {e}");
            return;
        }
    };
    let hints = app.status_hints();
    let palette = palette_commands();
    let request = app.pending_request.take();
    let json = serde_json::json!({
        "commands": commands_val,
        "hints": hints,
        "palette_commands": palette,
        "request": request,
        "consumed": consumed,
    });
    let Ok(json_str) = serde_json::to_string(&json) else {
        return;
    };
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json_str}");
    let _ = out.flush();
}

fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .format_target(false)
        .try_init();
    let mut reader = BufReader::new(std::io::stdin().lock());

    let mut app = App::default();
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let msg: HostMsg = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(e) => {
                        log::error!("[world-clock] parse error: {e}: {line}");
                        continue;
                    }
                };

                match msg {
                    HostMsg::Init {
                        theme,
                        area,
                        data_dir: _,
                    } => {
                        app.handle_init(theme, area);
                        respond(&mut app, false);
                    }
                    HostMsg::Key { key, .. } => {
                        let consumed = app.handle_key(key);
                        respond(&mut app, consumed);
                    }
                    HostMsg::Tick => {
                        app.handle_tick();
                        respond(&mut app, false);
                    }
                    HostMsg::Focus | HostMsg::Blur => {
                        respond(&mut app, false);
                    }
                    HostMsg::ThemeChange { theme } => {
                        app.theme = theme;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    HostMsg::Resize { area } => {
                        app.area = area;
                        app.dirty = true;
                        respond(&mut app, false);
                    }
                    HostMsg::PaletteCommand { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::PluginMessage { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::Mouse { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::UserUpdate { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::DbValue { key, value } => {
                        app.handle_db_value(&key, value);
                        respond(&mut app, false);
                    }
                    HostMsg::Shutdown => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Tz;
    use state::Screen;

    fn base_app() -> App {
        App::default()
    }

    #[test]
    fn initial_screen_is_grid() {
        let app = base_app();
        assert_eq!(app.state.screen, Screen::Grid);
    }

    #[test]
    fn handle_key_a_opens_search() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('a')));
        assert_eq!(app.state.screen, Screen::Search);
    }

    #[test]
    fn handle_key_esc_on_search_returns_to_grid() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.screen, Screen::Grid);
    }

    #[test]
    fn handle_key_esc_on_grid_returns_false() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_enter_on_grid_opens_detail() {
        let mut app = base_app();
        app.state.clocks = state::WorldTimeState::default_clocks();
        app.state.selected = 0;
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.screen, Screen::Detail(0));
    }

    #[test]
    fn handle_key_esc_on_detail_returns_to_grid() {
        let mut app = base_app();
        app.state.screen = Screen::Detail(0);
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.screen, Screen::Grid);
    }

    #[test]
    fn handle_key_tab_on_detail_returns_to_grid() {
        let mut app = base_app();
        app.state.screen = Screen::Detail(0);
        assert!(app.handle_key(IpcKey::Tab));
        assert_eq!(app.state.screen, Screen::Grid);
    }

    #[test]
    fn handle_key_d_removes_selected_clock() {
        let mut app = base_app();
        app.state.clocks = state::WorldTimeState::default_clocks();
        let len = app.state.clocks.len();
        assert!(app.handle_key(IpcKey::Char('d')));
        assert_eq!(app.state.clocks.len(), len - 1);
    }

    #[test]
    fn handle_key_d_with_empty_clocks_does_nothing() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('d')));
        assert!(app.state.clocks.is_empty());
    }

    #[test]
    fn handle_key_r_opens_rename() {
        let mut app = base_app();
        app.state.clocks = state::WorldTimeState::default_clocks();
        assert!(app.handle_key(IpcKey::Char('r')));
        assert_eq!(app.state.screen, Screen::Rename(0));
    }

    #[test]
    fn handle_key_esc_on_rename_returns_to_grid() {
        let mut app = base_app();
        app.state.screen = Screen::Rename(0);
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.screen, Screen::Grid);
    }

    #[test]
    fn rename_enter_saves_label() {
        let mut app = base_app();
        app.state.clocks = state::WorldTimeState::default_clocks();
        app.state.screen = Screen::Rename(0);
        app.state.rename_buf = "My City".into();
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.clocks[0].label, "My City");
        assert_eq!(app.state.screen, Screen::Grid);
    }

    #[test]
    fn rename_enter_empty_trims_and_skips() {
        let mut app = base_app();
        app.state.clocks = state::WorldTimeState::default_clocks();
        app.state.screen = Screen::Rename(0);
        app.state.rename_buf = "  ".into();
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.screen, Screen::Grid);
    }

    #[test]
    fn search_backspace_removes_char() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        app.state.search_query = "abc".into();
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.search_query, "ab");
    }

    #[test]
    fn search_backspace_empty_does_not_panic() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        assert!(app.handle_key(IpcKey::Backspace));
        assert!(app.state.search_query.is_empty());
    }

    #[test]
    fn search_char_adds_to_query() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        assert!(app.handle_key(IpcKey::Char('L')));
        assert_eq!(app.state.search_query, "L");
    }

    #[test]
    fn search_up_moves_cursor_back() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        app.state.search_results = timezones::ALL.iter().map(|&(_, tz)| tz).collect();
        app.state.search_cursor = 2;
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.search_cursor, 1);
    }

    #[test]
    fn search_down_moves_cursor_forward() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        app.state.search_results = timezones::ALL.iter().map(|&(_, tz)| tz).collect();
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.search_cursor, 1);
    }

    #[test]
    fn search_enter_adds_clock() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        app.state.search_results = vec![Tz::Europe__London];
        app.state.search_cursor = 0;
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.screen, Screen::Grid);
        assert_eq!(app.state.clocks.len(), 1);
        assert_eq!(app.state.clocks[0].tz, Tz::Europe__London);
    }

    #[test]
    fn search_enter_does_not_duplicate() {
        let mut app = base_app();
        app.state.screen = Screen::Search;
        app.state.clocks.push(state::ClockEntry {
            tz: Tz::Europe__London,
            label: "London".into(),
        });
        app.state.search_results = vec![Tz::Europe__London];
        app.state.search_cursor = 0;
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.clocks.len(), 1);
    }

    #[test]
    fn handle_tick_marks_dirty_on_new_second() {
        let mut app = base_app();
        app.dirty = false;
        app.state.last_second = 0;
        // We can't mock time; just verify no crash
        app.handle_tick();
    }

    #[test]
    fn handle_tick_no_dirty_same_second() {
        let mut app = base_app();
        app.dirty = false;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        app.state.last_second = now;
        app.handle_tick();
        assert!(!app.dirty);
    }

    #[test]
    fn handle_db_value_loads_clocks_from_json() {
        let mut app = base_app();
        let json = r#"[{"tz":"Europe/London","label":"London"}]"#;
        app.handle_db_value("clocks", Some(json.into()));
        assert_eq!(app.state.clocks.len(), 1);
        assert_eq!(app.state.clocks[0].label, "London");
    }

    #[test]
    fn handle_db_value_none_sets_default_clocks() {
        let mut app = base_app();
        app.handle_db_value("clocks", None);
        assert!(!app.state.clocks.is_empty());
    }

    #[test]
    fn handle_db_value_ignores_other_keys() {
        let mut app = base_app();
        app.handle_db_value("other", Some(r#"[]"#.into()));
        assert!(app.state.clocks.is_empty());
    }

    #[test]
    fn unhandled_key_returns_false() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::F(1)));
        assert!(!app.handle_key(IpcKey::F(2)));
        assert!(!app.handle_key(IpcKey::Home));
        assert!(!app.handle_key(IpcKey::End));
    }

    #[test]
    fn grid_navigation_arrows() {
        let mut app = base_app();
        app.state.clocks = state::WorldTimeState::default_clocks();
        let len = app.state.clocks.len();

        app.state.selected = 0;
        assert!(app.handle_key(IpcKey::Right));
        assert_eq!(app.state.selected, 1.min(len - 1));

        app.state.selected = 0;
        assert!(app.handle_key(IpcKey::Left));
        assert_eq!(app.state.selected, len - 1);

        app.state.selected = 0;
        assert!(app.handle_key(IpcKey::Down));
        let cols = grid_cols();
        assert_eq!(app.state.selected, cols.min(len - 1));

        app.state.selected = cols;
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 0);
    }
}
