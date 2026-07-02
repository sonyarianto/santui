use serde::{Deserialize, Serialize};

use crate::state::Units;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoResult {
    pub name: String,
    pub country: String,
    pub admin1: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurrentWeather {
    pub temp: f32,
    pub feels_like: f32,
    pub humidity: u8,
    pub precip_mm: f32,
    pub wind_speed: f32,
    pub wind_dir: u16,
    pub weather_code: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyPoint {
    pub hour: u8,
    pub temp: f32,
    pub weather_code: u16,
    pub precip_prob: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyForecast {
    pub date: String,
    pub weather_code: u16,
    pub temp_max: f32,
    pub temp_min: f32,
    pub precip_mm: f32,
    pub wind_max: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeatherData {
    pub current: CurrentWeather,
    pub hourly: Vec<HourlyPoint>,
    pub daily: Vec<DailyForecast>,
    pub fetched_at: u64,
}

pub fn fetch_weather(lat: f64, lon: f64, units: &Units) -> Result<WeatherData, String> {
    let temp_unit = match units {
        Units::Celsius => "celsius",
        Units::Fahrenheit => "fahrenheit",
    };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}\
         &current=temperature_2m,relative_humidity_2m,apparent_temperature,\
         precipitation,weather_code,wind_speed_10m,wind_direction_10m\
         &hourly=temperature_2m,weather_code,precipitation_probability\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,\
         precipitation_sum,wind_speed_10m_max\
         &forecast_days=7&wind_speed_unit=kmh&temperature_unit={}&timezone=auto",
        lat, lon, temp_unit
    );

    let mut resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let body: String = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;

    parse_weather(&body)
}

fn parse_weather(json: &str) -> Result<WeatherData, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;

    let current = v["current"].as_object().ok_or("missing current")?;
    let current_units = v["current_units"].as_object();
    let hourly = v["hourly"].as_object().ok_or("missing hourly")?;
    let daily = v["daily"].as_object().ok_or("missing daily")?;

    let weather_code = current["weather_code"].as_f64().unwrap_or(0.0) as u16;
    let fetched_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let data = WeatherData {
        current: CurrentWeather {
            temp: current["temperature_2m"].as_f64().unwrap_or(0.0) as f32,
            feels_like: current["apparent_temperature"].as_f64().unwrap_or(0.0) as f32,
            humidity: current["relative_humidity_2m"].as_f64().unwrap_or(0.0) as u8,
            precip_mm: current["precipitation"].as_f64().unwrap_or(0.0) as f32,
            wind_speed: current["wind_speed_10m"].as_f64().unwrap_or(0.0) as f32,
            wind_dir: current["wind_direction_10m"].as_f64().unwrap_or(0.0) as u16,
            weather_code,
        },
        hourly: parse_hourly(hourly, current_units),
        daily: parse_daily(daily),
        fetched_at,
    };

    Ok(data)
}

fn parse_hourly(
    hourly: &serde_json::Map<String, serde_json::Value>,
    _units: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Vec<HourlyPoint> {
    let times = hourly["time"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    let temps = hourly["temperature_2m"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();
    let codes = hourly["weather_code"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();
    let probs = hourly["precipitation_probability"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut result = Vec::new();
    for i in 0..24.min(temps.len().min(codes.len().min(probs.len()))) {
        let hour = if i < times.len() {
            times[i]
                .split('T')
                .nth(1)
                .and_then(|t| t.split(':').next())
                .and_then(|h| h.parse::<u8>().ok())
                .unwrap_or(0)
        } else {
            0
        };
        result.push(HourlyPoint {
            hour,
            temp: temps.get(i).copied().unwrap_or(0.0) as f32,
            weather_code: codes.get(i).copied().unwrap_or(0.0) as u16,
            precip_prob: probs.get(i).copied().unwrap_or(0.0) as u8,
        });
    }
    result
}

fn parse_daily(daily: &serde_json::Map<String, serde_json::Value>) -> Vec<DailyForecast> {
    let dates = daily["time"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    let codes = daily["weather_code"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();
    let maxs = daily["temperature_2m_max"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();
    let mins = daily["temperature_2m_min"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();
    let precips = daily["precipitation_sum"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();
    let winds = daily["wind_speed_10m_max"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut result = Vec::new();
    let n = dates.len().min(
        codes.len().min(
            maxs.len()
                .min(mins.len().min(precips.len().min(winds.len()))),
        ),
    );
    #[allow(clippy::needless_range_loop)]
    for i in 0..n {
        result.push(DailyForecast {
            date: dates[i].to_string(),
            weather_code: codes.get(i).copied().unwrap_or(0.0) as u16,
            temp_max: maxs.get(i).copied().unwrap_or(0.0) as f32,
            temp_min: mins.get(i).copied().unwrap_or(0.0) as f32,
            precip_mm: precips.get(i).copied().unwrap_or(0.0) as f32,
            wind_max: winds.get(i).copied().unwrap_or(0.0) as f32,
        });
    }
    result
}

pub fn geocode(query: &str) -> Result<Vec<GeoResult>, String> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=8&language=en&format=json",
        urlencoding(query)
    );

    let mut resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let body: String = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;

    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let results = v["results"].as_array().ok_or("no results")?;

    let geo_results: Vec<GeoResult> = results
        .iter()
        .map(|r| GeoResult {
            name: r["name"].as_str().unwrap_or("").to_string(),
            country: r["country"].as_str().unwrap_or("").to_string(),
            admin1: r["admin1"].as_str().map(|s| s.to_string()),
            latitude: r["latitude"].as_f64().unwrap_or(0.0),
            longitude: r["longitude"].as_f64().unwrap_or(0.0),
            timezone: r["timezone"].as_str().unwrap_or("UTC").to_string(),
        })
        .collect();

    Ok(geo_results)
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

pub fn weather_symbol(code: u16) -> &'static str {
    match code {
        0 => "☀",
        1..=3 => "⛅",
        45 | 48 => "🌫",
        51..=55 => "🌦",
        61..=65 => "🌧",
        71..=75 => "❄",
        80..=82 => "🌦",
        95 => "⛈",
        _ => "?",
    }
}

pub fn weather_description(code: u16) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 => "Foggy",
        48 => "Depositing rime fog",
        51 => "Light drizzle",
        53 => "Moderate drizzle",
        55 => "Dense drizzle",
        61 => "Slight rain",
        63 => "Moderate rain",
        65 => "Heavy rain",
        71 => "Slight snow",
        73 => "Moderate snow",
        75 => "Heavy snow",
        80 => "Slight rain showers",
        81 => "Moderate rain showers",
        82 => "Violent rain showers",
        95 => "Thunderstorm",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_symbol_clear() {
        assert_eq!(weather_symbol(0), "☀");
    }

    #[test]
    fn weather_symbol_thunderstorm() {
        assert_eq!(weather_symbol(95), "⛈");
    }

    #[test]
    fn weather_symbol_unknown() {
        assert_eq!(weather_symbol(999), "?");
    }

    #[test]
    fn weather_description_known() {
        assert_eq!(weather_description(0), "Clear sky");
        assert_eq!(weather_description(61), "Slight rain");
    }

    #[test]
    fn weather_description_unknown() {
        assert_eq!(weather_description(999), "Unknown");
    }
}
