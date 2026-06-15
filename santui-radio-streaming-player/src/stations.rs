use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Deserialize, Serialize)]
pub struct Station {
    pub name: String,
    pub url: String,
    pub genre: String,
    pub bitrate: u32,
}

fn default_stations() -> Vec<Station> {
    vec![
        Station {
            name: "NBS Radio Indonesia".into(),
            url: "https://cdnindo.klikhost.net/8766/stream".into(),
            genre: "Rock'n'roll".into(),
            bitrate: 128,
        },
        Station {
            name: "GOLD 905".into(),
            url: "https://playerservices.streamtheworld.com/api/livestream-redirect/GOLD905AAC.aac".into(),
            genre: "Oldies".into(),
            bitrate: 48,
        },
        Station {
            name: "BBC World Service".into(),
            url: "https://stream.live.vc.bbcmedia.co.uk/bbc_world_service".into(),
            genre: "News".into(),
            bitrate: 128,
        },
        Station {
            name: "Radio Paradise".into(),
            url: "https://stream.radioparadise.com/mp3-320".into(),
            genre: "Eclectic".into(),
            bitrate: 320,
        },
    ]
}

fn stations_path() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    if let Some(dir) = exe_dir {
        let p = dir.join("stations.json");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("stations.json")
}

pub fn load() -> Vec<Station> {
    let path = stations_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            let stations = default_stations();
            if let Ok(json) = serde_json::to_string_pretty(&stations) {
                let _ = std::fs::write(&path, json);
            }
            return stations;
        }
    };
    serde_json::from_str(&content).unwrap_or_else(|e| {
        eprintln!("  ⚠️  stations.json parse error: {e}");
        default_stations()
    })
}
