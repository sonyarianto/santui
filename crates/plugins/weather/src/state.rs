use serde::{Deserialize, Serialize};

use crate::api::{GeoResult, WeatherData};

pub const REFRESH_TICKS: u32 = 6000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum Units {
    #[default]
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SavedLocation {
    pub name: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeatherSettings {
    pub location: Option<SavedLocation>,
    pub units: Units,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FetchState {
    Idle,
    Fetching,
    Done,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Overview,
    Hourly,
    Daily,
    LocationSearch,
}

pub struct WeatherState {
    pub settings: WeatherSettings,
    pub data: Option<WeatherData>,
    pub fetch_state: FetchState,
    pub screen: Screen,
    pub search_query: String,
    pub search_results: Vec<GeoResult>,
    pub search_cursor: usize,
    pub search_fetching: bool,
    pub search_debounce_ticks: u8,
    pub ticks_since_refresh: u32,
    pub daily_cursor: usize,
    pub hourly_scroll: usize,
}

impl Default for WeatherState {
    fn default() -> Self {
        Self {
            settings: WeatherSettings::default(),
            data: None,
            fetch_state: FetchState::Idle,
            screen: Screen::Overview,
            search_query: String::new(),
            search_results: Vec::new(),
            search_cursor: 0,
            search_fetching: false,
            search_debounce_ticks: 0,
            ticks_since_refresh: 0,
            daily_cursor: 0,
            hourly_scroll: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_default_is_celsius() {
        let u = Units::default();
        assert!(matches!(u, Units::Celsius));
    }

    #[test]
    fn refresh_tick_threshold_constant_is_positive() {
        assert!(REFRESH_TICKS > 0);
    }

    #[test]
    fn weather_state_default_no_location() {
        let s = WeatherState::default();
        assert!(s.settings.location.is_none());
        assert_eq!(s.fetch_state, FetchState::Idle);
    }
}
