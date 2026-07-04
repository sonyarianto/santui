use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    pub source_currency: String,
    pub target_currency: String,
    pub favorite_pairs: Vec<(String, String)>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            source_currency: "USD".into(),
            target_currency: "EUR".into(),
            favorite_pairs: vec![
                ("USD".into(), "EUR".into()),
                ("USD".into(), "GBP".into()),
                ("EUR".into(), "JPY".into()),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FetchState {
    Idle,
    Fetching,
    Done,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Amount,
    Source,
    Target,
    BrowseCurrencies,
    Favorites,
}

pub struct CurrencyState {
    pub amount_input: String,
    pub parsed_amount: Option<f64>,
    pub source_currency: String,
    pub target_currency: String,
    pub rates: HashMap<String, f64>,
    pub rates_base: String,
    pub rates_last_update: String,
    pub rates_fetched_at: u64,
    pub fetch_state: FetchState,
    pub input_mode: InputMode,
    pub favorite_pairs: Vec<(String, String)>,
    pub browse_query: String,
    pub browse_results: Vec<String>,
    pub browse_cursor: usize,
    pub fav_cursor: usize,
}

impl Default for CurrencyState {
    fn default() -> Self {
        Self {
            amount_input: String::new(),
            parsed_amount: None,
            source_currency: "USD".into(),
            target_currency: "EUR".into(),
            rates: HashMap::new(),
            rates_base: String::new(),
            rates_last_update: String::new(),
            rates_fetched_at: 0,
            fetch_state: FetchState::Idle,
            input_mode: InputMode::Amount,
            favorite_pairs: vec![
                ("USD".into(), "EUR".into()),
                ("USD".into(), "GBP".into()),
                ("EUR".into(), "JPY".into()),
            ],
            browse_query: String::new(),
            browse_results: Vec::new(),
            browse_cursor: 0,
            fav_cursor: 0,
        }
    }
}

impl CurrencyState {
    pub fn parse_amount(&mut self) {
        self.parsed_amount = self.amount_input.trim().parse::<f64>().ok();
    }

    pub fn target_rate(&self) -> Option<f64> {
        self.rates.get(&self.target_currency).copied()
    }

    pub fn source_to_usd_rate(&self) -> f64 {
        if self.source_currency == self.rates_base {
            1.0
        } else {
            self.rates
                .get(&self.source_currency)
                .copied()
                .unwrap_or(1.0)
        }
    }

    pub fn filter_currencies(&mut self) {
        let q = self.browse_query.to_lowercase();
        if q.is_empty() {
            self.browse_results = self.rates.keys().cloned().collect();
        } else {
            self.browse_results = self
                .rates
                .keys()
                .filter(|code| code.to_lowercase().contains(&q))
                .cloned()
                .collect();
        }
        self.browse_results.sort();
        if self.browse_cursor >= self.browse_results.len() {
            self.browse_cursor = self.browse_results.len().saturating_sub(1);
        }
    }

    pub fn selected_browse_currency(&self) -> Option<&str> {
        self.browse_results
            .get(self.browse_cursor)
            .map(|s| s.as_str())
    }

    pub fn selected_fav_pair(&self) -> Option<&(String, String)> {
        self.favorite_pairs.get(
            self.fav_cursor
                .min(self.favorite_pairs.len().saturating_sub(1)),
        )
    }

    #[allow(dead_code)]
    pub fn has_favorite(&self, source: &str, target: &str) -> bool {
        self.favorite_pairs
            .iter()
            .any(|(s, t)| s == source && t == target)
    }

    pub fn add_favorite(&mut self) {
        let pair = (self.source_currency.clone(), self.target_currency.clone());
        if !self.favorite_pairs.contains(&pair) {
            self.favorite_pairs.push(pair);
        }
    }

    pub fn remove_favorite(&mut self, index: usize) {
        if index < self.favorite_pairs.len() {
            self.favorite_pairs.remove(index);
            if self.fav_cursor >= self.favorite_pairs.len() {
                self.fav_cursor = self.favorite_pairs.len().saturating_sub(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_state_default_has_usd_eur() {
        let state = PersistedState::default();
        assert_eq!(state.source_currency, "USD");
        assert_eq!(state.target_currency, "EUR");
    }

    #[test]
    fn favorite_pairs_default_has_three_pairs() {
        let state = PersistedState::default();
        assert_eq!(state.favorite_pairs.len(), 3);
    }

    #[test]
    fn amount_input_parses_correctly() {
        let mut state = CurrencyState::default();
        state.amount_input = "123.45".into();
        state.parse_amount();
        assert!((state.parsed_amount.unwrap() - 123.45).abs() < 0.001);
    }

    #[test]
    fn amount_input_empty_parses_to_none() {
        let mut state = CurrencyState::default();
        state.amount_input = String::new();
        state.parse_amount();
        assert!(state.parsed_amount.is_none());
    }

    #[test]
    fn amount_input_invalid_parses_to_none() {
        let mut state = CurrencyState::default();
        state.amount_input = "abc".into();
        state.parse_amount();
        assert!(state.parsed_amount.is_none());
    }

    #[test]
    fn filter_currencies_case_insensitive() {
        let mut state = CurrencyState::default();
        state.rates.insert("USD".into(), 1.0);
        state.rates.insert("EUR".into(), 0.92);
        state.rates.insert("GBP".into(), 0.79);
        state.browse_query = "us".into();
        state.filter_currencies();
        assert_eq!(state.browse_results.len(), 1);
        assert_eq!(state.browse_results[0], "USD");
    }

    #[test]
    fn filter_currencies_empty_shows_all() {
        let mut state = CurrencyState::default();
        state.rates.insert("USD".into(), 1.0);
        state.rates.insert("EUR".into(), 0.92);
        state.filter_currencies();
        assert_eq!(state.browse_results.len(), 2);
    }

    #[test]
    fn add_favorite_prevents_duplicates() {
        let mut state = CurrencyState::default();
        state.source_currency = "USD".into();
        state.target_currency = "EUR".into();
        assert!(state.has_favorite("USD", "EUR"));
        let len = state.favorite_pairs.len();
        state.add_favorite();
        assert_eq!(state.favorite_pairs.len(), len);
    }

    #[test]
    fn remove_favorite_updates_cursor() {
        let mut state = CurrencyState::default();
        state.favorite_pairs.clear();
        state.favorite_pairs.push(("USD".into(), "EUR".into()));
        state.favorite_pairs.push(("GBP".into(), "JPY".into()));
        state.fav_cursor = 1;
        state.remove_favorite(1);
        assert_eq!(state.fav_cursor, 0);
        assert_eq!(state.favorite_pairs.len(), 1);
    }
}
