use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const BASE_URL: &str = "https://open.er-api.com/v6";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatesResponse {
    pub result: String,
    pub base_code: String,
    pub rates: HashMap<String, f64>,
    #[serde(rename = "time_last_update_utc")]
    pub time_last_update_utc: String,
}

pub fn fetch_latest(base: &str) -> Result<RatesResponse, String> {
    let url = format!("{BASE_URL}/latest/{base}");
    let mut resp = ureq::get(&url)
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Read failed: {e}"))?;
    serde_json::from_str::<RatesResponse>(&body).map_err(|e| format!("Parse failed: {e}"))
}

pub fn convert(amount: f64, rate_from: f64, rate_to: f64) -> f64 {
    (amount / rate_from) * rate_to
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_identity() {
        let result = convert(100.0, 1.0, 1.0);
        assert!((result - 100.0).abs() < 0.001);
    }

    #[test]
    fn convert_double() {
        let result = convert(10.0, 2.0, 4.0);
        assert!((result - 20.0).abs() < 0.001);
    }

    #[test]
    fn convert_zero_amount() {
        let result = convert(0.0, 1.5, 2.5);
        assert!((result - 0.0).abs() < 0.001);
    }

    #[test]
    fn convert_usd_to_eur() {
        let result = convert(100.0, 1.0, 0.92);
        assert!((result - 92.0).abs() < 0.001);
    }

    #[test]
    fn rates_response_deserialize() {
        let json = r#"{
            "result": "success",
            "base_code": "USD",
            "rates": {"EUR": 0.92, "GBP": 0.79},
            "time_last_update_utc": "Mon, 01 Jan 2024 00:00:00 +0000"
        }"#;
        let resp: RatesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.result, "success");
        assert_eq!(resp.base_code, "USD");
        assert_eq!(resp.rates.get("EUR"), Some(&0.92));
    }
}
