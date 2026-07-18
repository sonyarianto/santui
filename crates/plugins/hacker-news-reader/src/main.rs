mod api;
mod state;
mod ui;

use std::io::{BufRead, BufReader};
use std::sync::mpsc;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};

use state::{FetchState, HnState, Screen};
use ui::render_ui;

enum FetchMsg {
    IdsDone(Vec<u32>),
    StoriesDone(Vec<api::HnItem>),
    CommentsDone(Vec<api::HnItem>),
    FetchError(String),
}

struct App {
    state: HnState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    rx_fetch: Option<mpsc::Receiver<FetchMsg>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: HnState::default(),
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
            pending_request: None,
            rx_fetch: None,
        }
    }
}

impl App {
    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.dirty = true;
        self.trigger_fetch_ids();
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        match self.state.screen.clone() {
            Screen::StoryList => self.handle_story_list_key(key),
            Screen::Comments => self.handle_comments_key(key),
        }
    }

    fn handle_story_list_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.selected = self.state.selected.saturating_sub(1);
                if self.state.selected < self.state.scroll {
                    self.state.scroll = self.state.selected;
                }
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.stories.len().saturating_sub(1);
                self.state.selected = self.state.selected.min(max).saturating_add(1).min(max);
                let visible_count = self.area.h.saturating_sub(3) as usize / 2;
                let visible_start = self.state.scroll;
                let visible_end = visible_start + visible_count.saturating_sub(1);
                if self.state.selected > visible_end {
                    self.state.scroll = self
                        .state
                        .selected
                        .saturating_sub(visible_count.saturating_sub(1));
                }
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                self.open_comments_for_selected();
                self.dirty = true;
                true
            }
            IpcKey::Char('t') => {
                self.set_category(state::Category::Top);
                self.dirty = true;
                true
            }
            IpcKey::Char('n') => {
                self.set_category(state::Category::New);
                self.dirty = true;
                true
            }
            IpcKey::Char('b') => {
                self.set_category(state::Category::Best);
                self.dirty = true;
                true
            }
            IpcKey::Tab => {
                let next = match self.state.category {
                    state::Category::Top => state::Category::New,
                    state::Category::New => state::Category::Best,
                    state::Category::Best => state::Category::Top,
                };
                self.set_category(next);
                self.dirty = true;
                true
            }
            IpcKey::Char('r') => {
                self.trigger_fetch_ids();
                self.dirty = true;
                true
            }
            IpcKey::Char('o') => {
                if let Some(story) = self.state.stories.get(self.state.selected) {
                    if let Some(ref url) = story.url {
                        open_url(url);
                    }
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_comments_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.scroll = self.state.scroll.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.state.scroll = self.state.scroll.saturating_add(1);
                self.dirty = true;
                true
            }
            IpcKey::Char('o') => {
                if let Some(ref story) = self.state.comment_story {
                    if let Some(ref url) = story.url {
                        open_url(url);
                    }
                }
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::StoryList;
                self.state.scroll = 0;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn set_category(&mut self, category: state::Category) {
        self.state.category = category;
        self.state.stories.clear();
        self.state.story_ids.clear();
        self.state.loaded_count = 0;
        self.state.selected = 0;
        self.state.scroll = 0;
        self.state.fetch_state = FetchState::Idle;
        self.trigger_fetch_ids();
    }

    fn open_comments_for_selected(&mut self) {
        if let Some(story) = self.state.stories.get(self.state.selected) {
            let story = story.clone();
            self.state.comment_story = Some(story);
            self.state.comments.clear();
            self.state.screen = Screen::Comments;
            self.state.scroll = 0;
            self.trigger_fetch_comments();
        }
    }

    fn trigger_fetch_ids(&mut self) {
        if matches!(self.state.fetch_state, FetchState::FetchingIds) {
            return;
        }
        let endpoint = self.state.category.endpoint().to_string();
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.state.fetch_state = FetchState::FetchingIds;
        std::thread::spawn(move || match api::fetch_story_ids(&endpoint) {
            Ok(ids) => {
                let _ = tx.send(FetchMsg::IdsDone(ids));
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::FetchError(e));
            }
        });
    }

    fn trigger_fetch_stories(&mut self) {
        let ids: Vec<u32> = self
            .state
            .story_ids
            .iter()
            .skip(self.state.loaded_count)
            .take(30)
            .copied()
            .collect();
        if ids.is_empty() {
            self.state.fetch_state = FetchState::Done;
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.state.fetch_state = FetchState::FetchingStories;
        std::thread::spawn(move || match api::fetch_items(&ids) {
            Ok(stories) => {
                let _ = tx.send(FetchMsg::StoriesDone(stories));
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::FetchError(e));
            }
        });
    }

    fn trigger_fetch_comments(&mut self) {
        let comment_ids = self
            .state
            .comment_story
            .as_ref()
            .and_then(|s| s.kids.clone())
            .unwrap_or_default();
        if comment_ids.is_empty() {
            self.state.fetch_state = FetchState::Done;
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.state.fetch_state = FetchState::FetchingComments;
        std::thread::spawn(move || match api::fetch_items(&comment_ids) {
            Ok(comments) => {
                let _ = tx.send(FetchMsg::CommentsDone(comments));
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::FetchError(e));
            }
        });
    }

    fn handle_tick(&mut self) {
        if let Some(ref rx) = self.rx_fetch {
            match rx.try_recv() {
                Ok(FetchMsg::IdsDone(ids)) => {
                    self.state.story_ids = ids;
                    self.state.fetch_state = FetchState::FetchingStories;
                    self.trigger_fetch_stories();
                    self.dirty = true;
                }
                Ok(FetchMsg::StoriesDone(stories)) => {
                    self.state.stories.extend(stories);
                    self.state.loaded_count = self.state.stories.len();
                    self.state.fetch_state = FetchState::Done;
                    self.dirty = true;
                }
                Ok(FetchMsg::CommentsDone(comments)) => {
                    self.state.comments = comments;
                    self.state.fetch_state = FetchState::Done;
                    self.dirty = true;
                }
                Ok(FetchMsg::FetchError(e)) => {
                    self.state.fetch_state = FetchState::Error(e);
                    self.dirty = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.set_category(state::Category::Top);
                self.dirty = true;
            }
            1 => {
                self.set_category(state::Category::New);
                self.dirty = true;
            }
            2 => {
                self.set_category(state::Category::Best);
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match &self.state.screen {
            Screen::StoryList => vec![
                ("t".into(), "top".into()),
                ("n".into(), "new".into()),
                ("b".into(), "best".into()),
                ("r".into(), "refresh".into()),
                ("enter".into(), "comments".into()),
                ("o".into(), "open".into()),
            ],
            Screen::Comments => vec![
                ("\u{2191}\u{2193}".into(), "navigate".into()),
                ("esc".into(), "back".into()),
                ("o".into(), "open story link".into()),
            ],
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
    vec![
        ("Plugins".into(), "Top stories".into()),
        ("Plugins".into(), "New stories".into()),
        ("Plugins".into(), "Best stories".into()),
    ]
}

fn respond(app: &mut App, consumed: bool) {
    let msg = santui_ipc::protocol::PluginMsg {
        commands: app.render().to_vec(),
        hints: app.status_hints(),
        palette_commands: palette_commands(),
        request: app.pending_request.take(),
        plugin_message: None,
        consumed,
    };
    let mut out = std::io::stdout().lock();
    let _ = santui_ipc::protocol::write_plugin_msg(&mut out, &msg);
}

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .spawn();
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
                        log::error!("[hacker-news-reader] parse error: {e}: {line}");
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
                    HostMsg::PaletteCommand { index } => {
                        app.handle_palette_command(index);
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
                    HostMsg::DbValue { .. } => {
                        respond(&mut app, false);
                    }
                    HostMsg::LogEntries { .. } => {
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

    fn base_app() -> App {
        App::default()
    }

    fn app_with_stories() -> App {
        let mut app = App::default();
        app.state.stories = vec![
            api::HnItem {
                id: 1,
                item_type: api::HnItemType::Story,
                by: Some("user1".into()),
                time: Some(1710000000),
                title: Some("Story 1".into()),
                text: None,
                url: Some("https://example.com/1".into()),
                score: Some(100),
                descendants: Some(10),
                kids: None,
                parent: None,
                deleted: None,
                dead: None,
            },
            api::HnItem {
                id: 2,
                item_type: api::HnItemType::Story,
                by: Some("user2".into()),
                time: Some(1710000000),
                title: Some("Story 2".into()),
                text: None,
                url: Some("https://example.com/2".into()),
                score: Some(50),
                descendants: Some(5),
                kids: None,
                parent: None,
                deleted: None,
                dead: None,
            },
            api::HnItem {
                id: 3,
                item_type: api::HnItemType::Story,
                by: Some("user3".into()),
                time: Some(1710000000),
                title: Some("Story 3".into()),
                text: None,
                url: Some("https://example.com/3".into()),
                score: Some(25),
                descendants: Some(3),
                kids: Some(vec![100]),
                parent: None,
                deleted: None,
                dead: None,
            },
        ];
        app.state.fetch_state = FetchState::Done;
        app
    }

    #[test]
    fn handle_key_up_down_navigates() {
        let mut app = app_with_stories();
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 1);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 2);
        assert!(app.handle_key(IpcKey::Down));
        assert_eq!(app.state.selected, 2);
        assert!(app.handle_key(IpcKey::Up));
        assert_eq!(app.state.selected, 1);
    }

    #[test]
    fn handle_key_t_switches_to_top() {
        let mut app = app_with_stories();
        app.state.category = state::Category::New;
        assert!(app.handle_key(IpcKey::Char('t')));
        assert!(matches!(app.state.category, state::Category::Top));
    }

    #[test]
    fn handle_key_n_switches_to_new() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('n')));
        assert!(matches!(app.state.category, state::Category::New));
    }

    #[test]
    fn handle_key_b_switches_to_best() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('b')));
        assert!(matches!(app.state.category, state::Category::Best));
    }

    #[test]
    fn handle_key_enter_opens_comments() {
        let mut app = app_with_stories();
        app.state.selected = 2;
        assert!(app.handle_key(IpcKey::Enter));
        assert!(matches!(app.state.screen, Screen::Comments));
        assert!(app.state.comment_story.is_some());
        assert_eq!(app.state.comment_story.unwrap().id, 3);
    }

    #[test]
    fn handle_key_esc_on_list_not_consumed() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_esc_on_comments_returns_to_list() {
        let mut app = app_with_stories();
        app.state.screen = Screen::Comments;
        app.state.comment_story = app.state.stories.first().cloned();
        app.state.scroll = 5;
        assert!(app.handle_key(IpcKey::Esc));
        assert!(matches!(app.state.screen, Screen::StoryList));
        assert_eq!(app.state.scroll, 0);
    }

    #[test]
    fn handle_key_r_refreshes() {
        let mut app = app_with_stories();
        assert!(app.handle_key(IpcKey::Char('r')));
        assert!(matches!(app.state.fetch_state, FetchState::FetchingIds));
    }

    #[test]
    fn handle_key_tab_cycles_category() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Tab));
        assert!(matches!(app.state.category, state::Category::New));
        assert!(app.handle_key(IpcKey::Tab));
        assert!(matches!(app.state.category, state::Category::Best));
        assert!(app.handle_key(IpcKey::Tab));
        assert!(matches!(app.state.category, state::Category::Top));
    }

    #[test]
    fn handle_tick_drains_ids_done() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        let _ = tx.send(FetchMsg::IdsDone(vec![1, 2, 3]));
        app.handle_tick();
        assert_eq!(app.state.story_ids, vec![1, 2, 3]);
        assert!(matches!(app.state.fetch_state, FetchState::FetchingStories));
    }

    #[test]
    fn handle_tick_drains_stories_done() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        app.state.fetch_state = FetchState::FetchingStories;
        let story = api::HnItem {
            id: 1,
            item_type: api::HnItemType::Story,
            by: Some("user".into()),
            time: Some(1710000000),
            title: Some("Story".into()),
            text: None,
            url: None,
            score: Some(10),
            descendants: Some(3),
            kids: None,
            parent: None,
            deleted: None,
            dead: None,
        };
        let _ = tx.send(FetchMsg::StoriesDone(vec![story]));
        app.handle_tick();
        assert_eq!(app.state.stories.len(), 1);
        assert!(matches!(app.state.fetch_state, FetchState::Done));
    }

    #[test]
    fn handle_tick_drains_fetch_error() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        let _ = tx.send(FetchMsg::FetchError("network error".into()));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Error(_)));
    }

    #[test]
    fn handle_tick_empty_channel_noop() {
        let mut app = base_app();
        let (_tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Idle));
    }

    #[test]
    fn palette_command_0_sets_top() {
        let mut app = base_app();
        app.state.category = state::Category::New;
        app.handle_palette_command(0);
        assert!(matches!(app.state.category, state::Category::Top));
    }

    #[test]
    fn palette_command_1_sets_new() {
        let mut app = base_app();
        app.handle_palette_command(1);
        assert!(matches!(app.state.category, state::Category::New));
    }

    #[test]
    fn palette_command_2_sets_best() {
        let mut app = base_app();
        app.handle_palette_command(2);
        assert!(matches!(app.state.category, state::Category::Best));
    }
}
