use crate::types::{Ayah, SurahContent, SurahSummary, ARABIC_EDITION};

pub fn fetch_json(url: &str) -> Result<serde_json::Value, String> {
    let mut resp = ureq::get(url).call().map_err(|e| e.to_string())?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

pub fn fetch_surah_list() -> Result<Vec<SurahSummary>, String> {
    parse_surah_list_value(&fetch_json("https://api.alquran.cloud/v1/surah")?)
}

pub fn parse_surah_list_value(value: &serde_json::Value) -> Result<Vec<SurahSummary>, String> {
    let data = value["data"]
        .as_array()
        .ok_or_else(|| "missing data array".to_string())?;
    let mut out = Vec::new();
    for item in data {
        out.push(SurahSummary {
            number: item["number"]
                .as_u64()
                .ok_or_else(|| "missing surah number".to_string())? as u16,
            name: item["name"].as_str().unwrap_or_default().to_string(),
            english_name: item["englishName"].as_str().unwrap_or_default().to_string(),
            english_translation: item["englishNameTranslation"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            ayah_count: item["numberOfAyahs"].as_u64().unwrap_or(0) as u16,
        });
    }
    Ok(out)
}

pub fn fetch_surah_content(
    summary: SurahSummary,
    translation: &str,
    reciter: &str,
) -> Result<SurahContent, String> {
    let n = summary.number;
    let arabic = fetch_json(&format!(
        "https://api.alquran.cloud/v1/surah/{n}/{ARABIC_EDITION}"
    ))?;
    let trans = fetch_json(&format!(
        "https://api.alquran.cloud/v1/surah/{n}/{translation}"
    ))?;
    let audio = fetch_json(&format!("https://api.alquran.cloud/v1/surah/{n}/{reciter}"))?;
    let ayahs = parse_surah_ayahs(&arabic, &trans, &audio)?;
    Ok(SurahContent {
        summary,
        ayahs,
        translation_edition: translation.into(),
        reciter: reciter.into(),
    })
}

pub fn parse_surah_ayahs(
    arabic: &serde_json::Value,
    translation: &serde_json::Value,
    audio: &serde_json::Value,
) -> Result<Vec<Ayah>, String> {
    let arabic_arr = arabic["data"]["ayahs"]
        .as_array()
        .ok_or_else(|| "missing Arabic ayahs".to_string())?;
    let trans_arr = translation["data"]["ayahs"]
        .as_array()
        .ok_or_else(|| "missing translation ayahs".to_string())?;
    let audio_arr = audio["data"]["ayahs"]
        .as_array()
        .ok_or_else(|| "missing audio ayahs".to_string())?;
    let mut out = Vec::new();
    for (idx, item) in arabic_arr.iter().enumerate() {
        let number = item["numberInSurah"].as_u64().unwrap_or((idx + 1) as u64) as u16;
        out.push(Ayah {
            number,
            arabic: item["text"].as_str().unwrap_or_default().to_string(),
            translation: trans_arr
                .get(idx)
                .and_then(|v| v["text"].as_str())
                .unwrap_or_default()
                .to_string(),
            audio_url: audio_arr
                .get(idx)
                .and_then(|v| v["audio"].as_str())
                .map(str::to_string),
        });
    }
    Ok(out)
}
