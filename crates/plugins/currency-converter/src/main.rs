mod api;
mod state;
mod ui;

use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;

use santui_ipc::protocol::{Area, HostMsg, IpcKey, PluginRequest, RenderCmd, ThemeData};
use ui::render_ui;

use state::{CurrencyState, FetchState, InputMode};

const RATES_CACHE_TICKS: u64 = 10 * 60; // ~10 min at ~1 tick/sec

enum FetchMsg {
    RatesDone(api::RatesResponse),
    RatesError(String),
}

struct App {
    state: CurrencyState,
    theme: ThemeData,
    area: Area,
    dirty: bool,
    cached_commands: Vec<RenderCmd>,
    pending_request: Option<PluginRequest>,
    rx_fetch: Option<mpsc::Receiver<FetchMsg>>,
    tick_count: u64,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: CurrencyState::default(),
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
                key: "currency-converter".into(),
            }),
            rx_fetch: None,
            tick_count: 0,
        }
    }
}

impl App {
    fn handle_init(&mut self, theme: ThemeData, area: Area) {
        self.theme = theme;
        self.area = area;
        self.dirty = true;
        self.trigger_rates_fetch();
    }

    fn handle_key(&mut self, key: IpcKey) -> bool {
        match &self.state.input_mode {
            InputMode::BrowseCurrencies => self.handle_browse_key(key),
            InputMode::Favorites => self.handle_favorites_key(key),
            _ => self.handle_main_key(key),
        }
    }

    fn handle_main_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Tab => {
                let next = match self.state.input_mode {
                    InputMode::Amount => InputMode::Source,
                    InputMode::Source => InputMode::Target,
                    InputMode::Target => InputMode::Amount,
                    _ => InputMode::Amount,
                };
                self.state.input_mode = next;
                self.dirty = true;
                true
            }
            IpcKey::Char('f') => {
                self.state.add_favorite();
                self.schedule_db_save();
                self.dirty = true;
                true
            }
            IpcKey::Char('F') => {
                self.state.fav_cursor = 0;
                self.state.input_mode = InputMode::Favorites;
                self.dirty = true;
                true
            }
            IpcKey::Char('s') => {
                std::mem::swap(
                    &mut self.state.source_currency,
                    &mut self.state.target_currency,
                );
                self.trigger_rates_fetch();
                self.dirty = true;
                true
            }
            IpcKey::Char(c) if matches!(self.state.input_mode, InputMode::Amount) => {
                if c.is_ascii_digit() || c == '.' {
                    if c == '.' && self.state.amount_input.contains('.') {
                        return true;
                    }
                    self.state.amount_input.push(c);
                    self.state.parse_amount();
                    self.dirty = true;
                }
                true
            }
            IpcKey::Backspace if matches!(self.state.input_mode, InputMode::Amount) => {
                self.state.amount_input.pop();
                self.state.parse_amount();
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                match self.state.input_mode {
                    InputMode::Source | InputMode::Target => {
                        self.state.browse_query.clear();
                        self.state.filter_currencies();
                        self.state.browse_cursor = 0;
                        self.state.input_mode = InputMode::BrowseCurrencies;
                        self.dirty = true;
                    }
                    _ => {}
                }
                true
            }
            IpcKey::Esc => false,
            _ => false,
        }
    }

    fn handle_browse_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Char(c) if !c.is_control() => {
                self.state.browse_query.push(c);
                self.state.filter_currencies();
                self.state.browse_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Backspace => {
                self.state.browse_query.pop();
                self.state.filter_currencies();
                self.state.browse_cursor = 0;
                self.dirty = true;
                true
            }
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.browse_cursor = self.state.browse_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.browse_results.len().saturating_sub(1);
                self.state.browse_cursor =
                    self.state.browse_cursor.min(max).saturating_add(1).min(max);
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                if let Some(code) = self.state.selected_browse_currency().map(|s| s.to_string()) {
                    self.state.source_currency = code;
                    self.trigger_rates_fetch();
                }
                self.state.input_mode = InputMode::Amount;
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.input_mode = InputMode::Amount;
                self.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn handle_favorites_key(&mut self, key: IpcKey) -> bool {
        match key {
            IpcKey::Up | IpcKey::Char('k') => {
                self.state.fav_cursor = self.state.fav_cursor.saturating_sub(1);
                self.dirty = true;
                true
            }
            IpcKey::Down | IpcKey::Char('j') => {
                let max = self.state.favorite_pairs.len().saturating_sub(1);
                self.state.fav_cursor = self.state.fav_cursor.min(max).saturating_add(1).min(max);
                self.dirty = true;
                true
            }
            IpcKey::Enter => {
                if let Some((s, t)) = self.state.selected_fav_pair().cloned() {
                    self.state.source_currency = s;
                    self.state.target_currency = t;
                    self.trigger_rates_fetch();
                }
                self.state.input_mode = InputMode::Amount;
                self.dirty = true;
                true
            }
            IpcKey::Char('d') => {
                let idx = self.state.fav_cursor;
                self.state.remove_favorite(idx);
                self.schedule_db_save();
                self.dirty = true;
                true
            }
            IpcKey::Esc => {
                self.state.input_mode = InputMode::Amount;
                self.dirty = true;
                true
            }
            _ => true,
        }
    }

    fn handle_tick(&mut self) {
        self.tick_count += 1;

        if let Some(ref rx) = self.rx_fetch {
            match rx.try_recv() {
                Ok(FetchMsg::RatesDone(resp)) => {
                    self.state.rates = resp.rates;
                    self.state.rates_base = resp.base_code;
                    self.state.rates_last_update = resp.time_last_update_utc;
                    self.state.fetch_state = FetchState::Done;
                    self.state.rates_fetched_at = self.tick_count;
                    self.dirty = true;
                }
                Ok(FetchMsg::RatesError(e)) => {
                    self.state.fetch_state = FetchState::Error(e);
                    self.dirty = true;
                }
                Err(_) => {}
            }
        }

        if self.tick_count.wrapping_sub(self.state.rates_fetched_at) > RATES_CACHE_TICKS
            && matches!(self.state.fetch_state, FetchState::Done | FetchState::Idle)
        {
            self.trigger_rates_fetch();
        }
    }

    fn trigger_rates_fetch(&mut self) {
        let base = self.state.source_currency.clone();
        self.state.fetch_state = FetchState::Fetching;
        self.dirty = true;

        let (tx, rx) = mpsc::channel();
        self.rx_fetch = Some(rx);
        std::thread::spawn(move || match api::fetch_latest(&base) {
            Ok(resp) => {
                let _ = tx.send(FetchMsg::RatesDone(resp));
            }
            Err(e) => {
                let _ = tx.send(FetchMsg::RatesError(e));
            }
        });
    }

    fn handle_db_value(&mut self, key: &str, value: Option<String>) {
        if key == "currency-converter" {
            if let Some(json) = value {
                if let Ok(p) = serde_json::from_str::<state::PersistedState>(&json) {
                    self.state.source_currency = p.source_currency;
                    self.state.target_currency = p.target_currency;
                    self.state.favorite_pairs = p.favorite_pairs;
                    self.trigger_rates_fetch();
                }
            }
            self.dirty = true;
        }
    }

    fn schedule_db_save(&mut self) {
        let p = state::PersistedState {
            source_currency: self.state.source_currency.clone(),
            target_currency: self.state.target_currency.clone(),
            favorite_pairs: self.state.favorite_pairs.clone(),
        };
        self.pending_request = Some(PluginRequest::DbSet {
            key: "currency-converter".into(),
            value: serde_json::to_string(&p).unwrap(),
        });
    }

    fn handle_palette_command(&mut self, index: u32) {
        match index {
            0 => {
                self.state.input_mode = InputMode::Amount;
                self.dirty = true;
            }
            1 => {
                std::mem::swap(
                    &mut self.state.source_currency,
                    &mut self.state.target_currency,
                );
                self.trigger_rates_fetch();
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn status_hints(&self) -> Vec<(String, String)> {
        match &self.state.input_mode {
            InputMode::BrowseCurrencies => {
                vec![
                    ("enter".into(), "select".into()),
                    ("esc".into(), "cancel".into()),
                    ("\u{2191}\u{2193}".into(), "navigate".into()),
                ]
            }
            InputMode::Favorites => {
                vec![
                    ("enter".into(), "load pair".into()),
                    ("d".into(), "delete".into()),
                    ("esc".into(), "back".into()),
                ]
            }
            _ => {
                let mut hints = vec![
                    ("tab".into(), "next field".into()),
                    ("f".into(), "save favorite".into()),
                    ("F".into(), "favorites".into()),
                    ("s".into(), "swap".into()),
                ];
                if matches!(self.state.fetch_state, FetchState::Error(_)) {
                    hints.push(("wait".into(), "retrying...".into()));
                }
                hints
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
        ("Currency".into(), "Open currency converter".into()),
        ("Currency".into(), "Swap source/target".into()),
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
                        log::error!("[currency] parse error: {e}: {line}");
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
    fn handle_key_tab_cycles_field() {
        let mut app = base_app();
        assert_eq!(app.state.input_mode, InputMode::Amount);
        assert!(app.handle_key(IpcKey::Tab));
        assert_eq!(app.state.input_mode, InputMode::Source);
        assert!(app.handle_key(IpcKey::Tab));
        assert_eq!(app.state.input_mode, InputMode::Target);
        assert!(app.handle_key(IpcKey::Tab));
        assert_eq!(app.state.input_mode, InputMode::Amount);
    }

    #[test]
    fn handle_key_digit_appends_to_amount() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('1')));
        assert!(app.handle_key(IpcKey::Char('2')));
        assert!(app.handle_key(IpcKey::Char('3')));
        assert_eq!(app.state.amount_input, "123");
        assert!((app.state.parsed_amount.unwrap() - 123.0).abs() < 0.001);
    }

    #[test]
    fn handle_key_dot_in_amount() {
        let mut app = base_app();
        assert!(app.handle_key(IpcKey::Char('1')));
        assert!(app.handle_key(IpcKey::Char('.')));
        assert!(app.handle_key(IpcKey::Char('5')));
        assert_eq!(app.state.amount_input, "1.5");
    }

    #[test]
    fn handle_key_backspace_pops_amount() {
        let mut app = base_app();
        app.state.amount_input = "123".into();
        app.state.parse_amount();
        assert!(app.handle_key(IpcKey::Backspace));
        assert_eq!(app.state.amount_input, "12");
    }

    #[test]
    fn handle_key_enter_opens_currency_picker() {
        let mut app = base_app();
        app.state.input_mode = InputMode::Source;
        assert!(app.handle_key(IpcKey::Enter));
        assert_eq!(app.state.input_mode, InputMode::BrowseCurrencies);
    }

    #[test]
    fn handle_key_f_adds_favorite() {
        let mut app = base_app();
        app.state.favorite_pairs.clear();
        app.state.source_currency = "USD".into();
        app.state.target_currency = "EUR".into();
        assert!(app.handle_key(IpcKey::Char('f')));
        assert!(app.state.has_favorite("USD", "EUR"));
    }

    #[test]
    fn handle_key_esc_on_main_not_consumed() {
        let mut app = base_app();
        assert!(!app.handle_key(IpcKey::Esc));
    }

    #[test]
    fn handle_key_esc_on_browse_closes() {
        let mut app = base_app();
        app.state.input_mode = InputMode::BrowseCurrencies;
        assert!(app.handle_key(IpcKey::Esc));
        assert_eq!(app.state.input_mode, InputMode::Amount);
    }

    #[test]
    fn handle_key_s_swaps() {
        let mut app = base_app();
        let src = app.state.source_currency.clone();
        let tgt = app.state.target_currency.clone();
        app.state.fetch_state = FetchState::Done;
        app.handle_key(IpcKey::Char('s'));
        assert_eq!(app.state.source_currency, tgt);
        assert_eq!(app.state.target_currency, src);
    }

    #[test]
    fn handle_tick_drains_rates_done() {
        let mut app = base_app();
        let mut rates = std::collections::HashMap::new();
        rates.insert("EUR".into(), 0.92);
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        let _ = tx.send(FetchMsg::RatesDone(api::RatesResponse {
            result: "success".into(),
            base_code: "USD".into(),
            rates,
            time_last_update_utc: "today".into(),
        }));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Done));
        assert_eq!(app.state.rates.get("EUR"), Some(&0.92));
    }

    #[test]
    fn handle_tick_drains_rates_error() {
        let mut app = base_app();
        let (tx, rx) = mpsc::channel();
        app.rx_fetch = Some(rx);
        let _ = tx.send(FetchMsg::RatesError("fail".into()));
        app.handle_tick();
        assert!(matches!(app.state.fetch_state, FetchState::Error(_)));
    }

    #[test]
    fn handle_db_value_loads_persisted_state() {
        let mut app = base_app();
        let json =
            r#"{"source_currency":"GBP","target_currency":"JPY","favorite_pairs":[["GBP","JPY"]]}"#;
        app.handle_db_value("currency-converter", Some(json.into()));
        assert_eq!(app.state.source_currency, "GBP");
        assert_eq!(app.state.target_currency, "JPY");
        assert_eq!(app.state.favorite_pairs.len(), 1);
    }

    #[test]
    fn handle_db_value_none_uses_defaults() {
        let mut app = base_app();
        app.handle_db_value("currency-converter", None);
        assert_eq!(app.state.source_currency, "USD");
        assert_eq!(app.state.target_currency, "EUR");
    }
}
