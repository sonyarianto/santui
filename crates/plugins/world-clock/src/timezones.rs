use chrono_tz::Tz;

pub const ALL: &[(&str, Tz)] = &[
    ("UTC", Tz::UTC),
    ("London", Tz::Europe__London),
    ("Paris", Tz::Europe__Paris),
    ("Berlin", Tz::Europe__Berlin),
    ("Rome", Tz::Europe__Rome),
    ("Madrid", Tz::Europe__Madrid),
    ("Amsterdam", Tz::Europe__Amsterdam),
    ("Brussels", Tz::Europe__Brussels),
    ("Vienna", Tz::Europe__Vienna),
    ("Stockholm", Tz::Europe__Stockholm),
    ("Oslo", Tz::Europe__Oslo),
    ("Copenhagen", Tz::Europe__Copenhagen),
    ("Warsaw", Tz::Europe__Warsaw),
    ("Prague", Tz::Europe__Prague),
    ("Budapest", Tz::Europe__Budapest),
    ("Athens", Tz::Europe__Athens),
    ("Istanbul", Tz::Europe__Istanbul),
    ("Moscow", Tz::Europe__Moscow),
    ("Kyiv", Tz::Europe__Kyiv),
    ("Helsinki", Tz::Europe__Helsinki),
    ("Dubai", Tz::Asia__Dubai),
    ("Karachi", Tz::Asia__Karachi),
    ("Kolkata", Tz::Asia__Kolkata),
    ("Kathmandu", Tz::Asia__Kathmandu),
    ("Dhaka", Tz::Asia__Dhaka),
    ("Jakarta", Tz::Asia__Jakarta),
    ("Bangkok", Tz::Asia__Bangkok),
    ("Hanoi", Tz::Asia__Ho_Chi_Minh),
    ("Singapore", Tz::Asia__Singapore),
    ("Hong Kong", Tz::Asia__Hong_Kong),
    ("Shanghai", Tz::Asia__Shanghai),
    ("Taipei", Tz::Asia__Taipei),
    ("Tokyo", Tz::Asia__Tokyo),
    ("Seoul", Tz::Asia__Seoul),
    ("Perth", Tz::Australia__Perth),
    ("Adelaide", Tz::Australia__Adelaide),
    ("Sydney", Tz::Australia__Sydney),
    ("Melbourne", Tz::Australia__Melbourne),
    ("Brisbane", Tz::Australia__Brisbane),
    ("Auckland", Tz::Pacific__Auckland),
    ("Honolulu", Tz::Pacific__Honolulu),
    ("Anchorage", Tz::America__Anchorage),
    ("Los Angeles", Tz::America__Los_Angeles),
    ("San Francisco", Tz::America__Los_Angeles),
    ("Denver", Tz::America__Denver),
    ("Phoenix", Tz::America__Phoenix),
    ("Chicago", Tz::America__Chicago),
    ("New York", Tz::America__New_York),
    ("Toronto", Tz::America__Toronto),
    ("Montreal", Tz::America__Montreal),
    ("Mexico City", Tz::America__Mexico_City),
    ("Sao Paulo", Tz::America__Sao_Paulo),
    ("Buenos Aires", Tz::America__Argentina__Buenos_Aires),
    ("Santiago", Tz::America__Santiago),
    ("Lagos", Tz::Africa__Lagos),
    ("Cairo", Tz::Africa__Cairo),
    ("Cape Town", Tz::Africa__Johannesburg),
    ("Nairobi", Tz::Africa__Nairobi),
    ("Casablanca", Tz::Africa__Casablanca),
    ("Reykjavik", Tz::Atlantic__Reykjavik),
    ("Azores", Tz::Atlantic__Azores),
];

pub fn search(query: &str) -> Vec<Tz> {
    if query.is_empty() {
        let mut all: Vec<Tz> = ALL.iter().map(|&(_, tz)| tz).collect();
        all.sort_by_key(|tz| city_name(*tz));
        return all;
    }
    let lower = query.to_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();
    for &(name, tz) in ALL {
        let haystack = format!("{} {}", name.to_lowercase(), tz.name().to_lowercase());
        if tokens.iter().all(|t| haystack.contains(t)) && seen.insert(tz) {
            results.push(tz);
        }
    }
    results.sort_by_key(|tz| city_name(*tz));
    results
}

pub fn city_name(tz: Tz) -> String {
    tz.name()
        .split('/')
        .next_back()
        .unwrap_or("")
        .replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_tokyo() {
        let results = search("tokyo");
        assert!(results.contains(&chrono_tz::Tz::Asia__Tokyo));
    }

    #[test]
    fn search_case_insensitive() {
        let results = search("LONDON");
        assert!(results.contains(&chrono_tz::Tz::Europe__London));
    }

    #[test]
    fn search_partial_match() {
        let results = search("tok");
        assert!(results.contains(&chrono_tz::Tz::Asia__Tokyo));
    }

    #[test]
    fn search_empty_returns_all() {
        let results = search("");
        assert_eq!(results.len(), ALL.len());
    }

    #[test]
    fn search_no_match_returns_empty() {
        let results = search("zzzzzznotacity");
        assert!(results.is_empty());
    }

    #[test]
    fn city_name_strips_prefix() {
        let name = city_name(chrono_tz::Tz::Asia__Tokyo);
        assert_eq!(name, "Tokyo");
    }

    #[test]
    fn city_name_utc() {
        let name = city_name(chrono_tz::Tz::UTC);
        assert!(!name.is_empty());
    }

    #[test]
    fn all_list_not_empty() {
        assert!(!ALL.is_empty());
    }
}
