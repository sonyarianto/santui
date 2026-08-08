use crate::types::{ARABIC_EDITION, Ayah, Edition, SurahContent, SurahSummary};

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

pub fn fetch_editions() -> Result<Vec<Edition>, String> {
    parse_editions_value(&fetch_json("https://api.alquran.cloud/v1/edition")?)
}

pub fn parse_editions_value(value: &serde_json::Value) -> Result<Vec<Edition>, String> {
    let data = value["data"]
        .as_array()
        .ok_or_else(|| "missing data array".to_string())?;
    let mut out = Vec::new();
    for item in data {
        let format = item["format"].as_str().unwrap_or_default().to_string();
        if format != "text" && format != "audio" {
            continue;
        }
        out.push(Edition {
            identifier: item["identifier"].as_str().unwrap_or_default().to_string(),
            name: item["name"].as_str().unwrap_or_default().to_string(),
            english_name: item["englishName"].as_str().unwrap_or_default().to_string(),
            language: item["language"].as_str().unwrap_or_default().to_string(),
            format,
            kind: item["type"].as_str().unwrap_or_default().to_string(),
        });
    }
    Ok(out)
}

pub fn translation_editions(editions: &[Edition]) -> Vec<Edition> {
    let mut list: Vec<Edition> = editions
        .iter()
        .filter(|e| e.format == "text" && e.kind == "translation")
        .cloned()
        .collect();
    list.sort_by(|a, b| {
        lang_rank(&a.language)
            .cmp(&lang_rank(&b.language))
            .then_with(|| a.language.cmp(&b.language))
            .then_with(|| a.english_name.cmp(&b.english_name))
    });
    list
}

pub fn reciter_editions(editions: &[Edition]) -> Vec<Edition> {
    let mut list: Vec<Edition> = editions
        .iter()
        .filter(|e| e.format == "audio" && e.language == "ar" && !e.identifier.ends_with("-2"))
        .cloned()
        .collect();
    list.sort_by(|a, b| a.english_name.cmp(&b.english_name));
    list
}

fn lang_rank(lang: &str) -> u8 {
    match lang {
        "en" => 0,
        "id" => 1,
        _ => 2,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const EDITIONS_JSON: &str = r#"{"data":[
        {"identifier":"en.sahih","language":"en","name":"Saheeh International","englishName":"Saheeh International","format":"text","type":"translation","direction":"ltr"},
        {"identifier":"id.indonesian","language":"id","name":"Bahasa Indonesia","englishName":"Unknown","format":"text","type":"translation","direction":"ltr"},
        {"identifier":"en.transliteration","language":"en","name":"Transliteration","englishName":"English Transliteration","format":"text","type":"transliteration","direction":"ltr"},
        {"identifier":"ar.jalalayn","language":"ar","name":"تفسير الجلالين","englishName":"Jalal ad-Din","format":"text","type":"tafsir","direction":"rtl"},
        {"identifier":"quran-uthmani","language":"ar","name":"Uthmani","englishName":"Uthmani","format":"text","type":"quran","direction":"rtl"},
        {"identifier":"ar.alafasy","language":"ar","name":"مشاري العفاسي","englishName":"Alafasy","format":"audio","type":"versebyverse","direction":null},
        {"identifier":"ar.alafasy-2","language":"ar","name":"مشاري العفاسي","englishName":"Alafasy","format":"audio","type":"versebyverse","direction":null},
        {"identifier":"en.walk","language":"en","name":"Ibrahim Walk","englishName":"Ibrahim Walk","format":"audio","type":"versebyverse","direction":null},
        {"identifier":"ar.husary","language":"ar","name":"محمود خليل الحصري","englishName":"Husary","format":"audio","type":"versebyverse","direction":null}
    ]}"#;

    fn editions() -> Vec<Edition> {
        parse_editions_value(&serde_json::from_str(EDITIONS_JSON).unwrap()).unwrap()
    }

    #[test]
    fn parses_all_editions() {
        assert_eq!(editions().len(), 9);
    }

    #[test]
    fn translation_filter_excludes_audio_and_other_types() {
        let list = translation_editions(&editions());
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].identifier, "en.sahih");
        assert_eq!(list[1].identifier, "id.indonesian");
    }

    #[test]
    fn reciter_filter_excludes_duplicates_and_non_arabic() {
        let list = reciter_editions(&editions());
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].identifier, "ar.alafasy");
        assert_eq!(list[1].identifier, "ar.husary");
    }

    #[test]
    fn display_name_falls_back_to_native_name() {
        let list = editions();
        let id_edition = list
            .iter()
            .find(|e| e.identifier == "id.indonesian")
            .unwrap();
        assert_eq!(id_edition.display_name(), "Bahasa Indonesia");
        let en_edition = list.iter().find(|e| e.identifier == "en.sahih").unwrap();
        assert_eq!(en_edition.display_name(), "Saheeh International");
    }
}
