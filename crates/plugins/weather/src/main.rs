mod api;
mod state;
mod ui;

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};

use api::{GeoResult, WeatherData};
use state::{FetchState, Screen, WeatherState, REFRESH_TICKS};
use ui::render_ui;

enum FetchMsg {
    WeatherDone(WeatherData),
    WeatherError(String),
    GeoResults(Vec<GeoResult>),
    #[allow(dead_code)]
    GeoError(String),
}

struct App {
    state: WeatherState,
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
            state: WeatherState::default(),
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
                key: "weather".into(),
            }),
            rx_fetch: None,
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
        match self.state.screen.clone() {
            Screen::Overview => self.handle_overview_key(key),
            Screen::Hourly => self.handle_hourly_key(key),
            Screen::Daily => self.handle_daily_key(key),
            Screen::LocationSearch => self.handle_search_key(key),
        }
    }

    fn handle_overview_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char('h') => {
                self.state.screen = Screen::Hourly;
                self.state.hourly_scroll = 0;
                self.dirty = true;
                true
            }
            IpcKey::Char('d') => {
                self.state.screen = Screen::Daily;
                self.state.daily_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Char('l') => {
                self.state.screen = Screen::LocationSearch;
                self.state.search_query.clear();
                self.state.search_results.clear();
                self.state.search_cursor = 0;
                self.state.search_debounce_ticks = 0;
                self.dirty = true;
                true
            }
            IpcKey::Char('u') => {
                self.state.settings.units = match self.state.settings.units {
                    state::Units::Celsius => state::Units::Fahrenheit,
                    state::Units::Fahrenheit => state::Units::Celsius,
                };
                self.schedule_db_save();
                self.trigger_weather_fetch();
                self.dirty = true;
                true
            }
            IpcKey::Char('r') => {
                self.trigger_weather_fetch();
                self.dirty = true;
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_hourly_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Left | IpcKey::Char('h') => {
                self.state.hourly_scroll = self.state.hourly_scroll.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Right | IpcKey::Char('l') => {
                self.state.hourly_scroll =
                    self.state.hourly_scroll.min(23).saturating_add(1).min(23);
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::Overview;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_daily_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.daily_cursor = self.state.daily_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                self.state.daily_cursor = self.state.daily_cursor.min(6).saturating_add(1).min(6);
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::Overview;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    fn handle_search_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.search_cursor = self.state.search_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.search_results.len().saturating_sub(1);
                self.state.search_cursor =
                    self.state.search_cursor.min(max).saturating_add(1).min(max);
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.search_query.pop();
                self.state.search_debounce_ticks = 3;
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                if let Some(result) = self.state.search_results.get(self.state.search_cursor) {
                    self.state.settings.location = Some(state::SavedLocation {
                        name: result.name.clone(),
                        country: result.country.clone(),
                        latitude: result.latitude,
                        longitude: result.longitude,
                        timezone: result.timezone.clone(),
                    });
                    self.schedule_db_save();
                    self.trigger_weather_fetch();
                }
                self.state.screen = Screen::Overview;
                self.dirty = true;
                true
            }
            IpcKey::Char(c) if !c.is_control() => {
                self.state.search_query.push(c);
                self.state.search_debounce_ticks = 3;
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.screen = Screen::Overview;
                self.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn handle_tick(&mut self) {
        if let Some(ref rx) = self.rx_fetch {
            match rx.try_recv() {
                Ok(FetchMsg::WeatherDone(data)) => {
                    self.state.data = Some(data);
                    self.state.fetch_state = FetchState::Done;
                    self.state.ticks_since_refresh = 0;
                    self.dirty = true;
                }
                Ok(FetchMsg::WeatherError(e)) => {
                    self.state.fetch_state = FetchState::Error(e);
                    self.dirty = true;
                }
                Ok(FetchMsg::GeoResults(results)) => {
                    self.state.search_results = results;
                    self.state.search_fetching = false;
                    self.state.search_cursor = 0;
                    self.dirty = true;
                }
                Ok(FetchMsg::GeoError(_)) => {
                    self.state.search_fetching = false;
                    self.state.search_results.clear();
                    self.dirty = true;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }

        if self.state.search_debounce_ticks > 0 {
            self.state.search_debounce_ticks -= 1;
            if self.state.search_debounce_ticks == 0 {
                self.trigger_geo_search();
            }
        }

        self.state.ticks_since_refresh += 1;
        if self.state.ticks_since_refresh >= REFRESH_TICKS
            && matches!(self.state.fetch_state, FetchState::Done | FetchState::Idle)
            && self.state.settings.location.is_some()
        {
            self.trigger_weather_fetch();
        }
    }

    fn trigger_weather_fetch(&mut self) {
        let Some(ref loc) = self.state.settings.location else {
            return;
        };
        let lat = loc.latitude;
        let lon = loc.longitude;
        let units = self.state.settings.units.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.state.fetch_state = FetchState::Fetching;
        std::thread::spawn(move || match api::fetch_weather(lat, lon, &units) {
            Ok(data) => {
                let _ = tx.send(FetchMsg::WeatherDone(data));
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::WeatherError(e));
            }
        });
    }

    fn trigger_geo_search(&mut self) {
        if self.state.search_query.len() < 2 {
            return;
        }
        let query = self.state.search_query.clone();
        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        self.state.search_fetching = true;
        std::thread::spawn(move || match api::geocode(&query) {
            Ok(results) => {
                let _ = tx.send(FetchMsg::GeoResults(results));
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::GeoError(e));
            }
        });
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "weather" {
            if let Some(json) = value {
                if let Ok(settings) = serde_json::from_str::<state::WeatherSettings>(&json) {
                    self.state.settings = settings;
                    if self.state.settings.location.is_some() {
                        self.trigger_weather_fetch();
                    }
                }
            }
            self.dirty = true;
        }
    }

    fn schedule_db_save(&mut self) {
        self.pending_request = Some(PluginRequest::DbSet {
            key: "weather".into(),
            value: serde_json::to_string(&self.state.settings).unwrap(),
        });
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.state.screen = Screen::Overview;
                self.dirty = true;
            }
            1 => {
                self.state.screen = Screen::Hourly;
                self.state.hourly_scroll = 0;
                self.dirty = true;
            }
            2 => {
                self.state.screen = Screen::Daily;
                self.state.daily_cursor = 0;
                self.dirty = true;
            }
            3 => {
                self.state.screen = Screen::LocationSearch;
                self.state.search_query.clear();
                self.state.search_results.clear();
                self.state.search_cursor = 0;
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match &self.state.screen {
            Screen::Overview => {
                let mut hints = vec![
                    ("h".into(), "hourly".into()),
                    ("d".into(), "7-day".into()),
                    ("l".into(), "location".into()),
                    ("u".into(), "units".into()),
                ];
                if matches!(self.state.fetch_state, FetchState::Error(_)) {
                    hints.push(("r".into(), "retry".into()));
                }
                hints
            }
            Screen::Hourly => {
                vec![
                    ("← →".into(), "scroll".into()),
                    ("esc".into(), "back".into()),
                ]
            }
            Screen::Daily => {
                vec![
                    ("↑↓".into(), "navigate".into()),
                    ("esc".into(), "back".into()),
                ]
            }
            Screen::LocationSearch => {
                vec![
                    ("enter".into(), "select".into()),
                    ("esc".into(), "cancel".into()),
                    ("↑↓".into(), "navigate".into()),
                ]
            }
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
        ("Weather".into(), "Current conditions".into()),
        ("Weather".into(), "Hourly forecast".into()),
        ("Weather".into(), "7-day forecast".into()),
        ("Weather".into(), "Set location".into()),
    ]
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
                        log::error!("[weather] parse error: {e}: {line}");
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
                    HostMsg::DbValue { key, value } => {
                        app.handle_db_value(&key, value);
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

    #[test]
    fn handle_key_h_opens_hourly() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('h')));
        assert_eq!(app.state.screen, Screen::Hourly);
    }

    #[test]
    fn handle_key_d_opens_daily() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('d')));
        assert_eq!(app.state.screen, Screen::Daily);
    }

    #[test]
    fn handle_key_l_opens_location_search() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('l')));
        assert_eq!(app.state.screen, Screen::LocationSearch);
    }

    #[test]
    fn handle_key_u_toggles_units() {
        let mut app = base_app();
        assert!(matches!(app.state.settings.units, state::Units::Celsius));
        assert!(app.handle_key(IpcKey::Char('u')));
        assert!(matches!(app.state.settings.units, state::Units::Fahrenheit));
        assert!(app.handle_key(IpcKey::Char('u')));
        assert!(matches!(app.state.settings.units, state::Units::Celsius));
    }

    #[test]
    fn handle_key_esc_on_overview_not_consumed() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_esc_on_hourly_returns_to_overview() {
        let mut app = base_app();
        app.state.screen = Screen::Hourly;
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.screen, Screen::Overview);
    }

    #[test]
    fn handle_key_enter_in_location_search_saves_location() {
        let mut app = base_app();
        app.state.screen = Screen::LocationSearch;
        app.state.search_results.push(GeoResult {
            name: "Tokyo".into(),
            country: "Japan".into(),
            admin1: Some("Tokyo Prefecture".into()),
            latitude: 35.68,
            longitude: 139.69,
            timezone: "Asia/Tokyo".into(),
        });
        app.state.search_cursor = 0;
        assert!(app.handle_key(IpcKey::Enter));
        assert!(app.state.settings.location.is_some());
        assert_eq!(app.state.settings.location.as_ref().unwrap().name, "Tokyo");
    }

    #[test]
    fn handle_db_value_loads_settings_and_triggers_fetch() {
        let mut app = base_app();
        let json = serde_json::json!({
            "location": {
                "name": "London",
                "country": "UK",
                "latitude": 51.51,
                "longitude": -0.13,
                "timezone": "Europe/London"
            },
            "units": "Celsius"
        });
        app.handle_db_value("weather", Some(json.to_string()));
        assert_eq!(app.state.settings.location.as_ref().unwrap().name, "London");
        assert!(matches!(app.state.fetch_state, FetchState::Fetching));
    }

    #[test]
    fn handle_tick_increments_ticks_since_refresh() {
        let mut app = base_app();
        let old = app.state.ticks_since_refresh;
        app.handle_tick();
        assert!(app.state.ticks_since_refresh >= old);
    }

    #[test]
    fn handle_tick_drains_weather_done() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        let _ = tx.send(FetchMsg::WeatherDone(WeatherData::default()));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Done));
    }

    #[test]
    fn handle_tick_drains_weather_error() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        let _ = tx.send(FetchMsg::WeatherError("err".into()));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Error(_)));
    }

    #[test]
    fn handle_tick_fires_geo_search_after_debounce() {
        let mut app = base_app();
        app.state.search_query = "Tokyo".into();
        app.state.search_debounce_ticks = 1;
        app.handle_tick();
        assert_eq!(app.state.search_debounce_ticks, 0);
        // Geo search would fire but needs network - just verify no crash
    }

    #[test]
    fn palette_command_0_opens_overview() {
        let mut app = base_app();
        app.state.screen = Screen::Hourly;
        app.handle_palette_command(0);
        assert_eq!(app.state.screen, Screen::Overview);
    }

    #[test]
    fn palette_command_3_opens_location_search() {
        let mut app = base_app();
        app.handle_palette_command(3);
        assert_eq!(app.state.screen, Screen::LocationSearch);
    }
}
