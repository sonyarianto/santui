#[derive(Clone)]
pub struct Station {
    pub name: &'static str,
    pub url: &'static str,
    pub genre: &'static str,
    pub bitrate: u32,
}

pub const STATIONS: &[Station] = &[
    Station {
        name: "NBS Radio Indonesia",
        url: "https://cdnindo.klikhost.net/8766/stream",
        genre: "Rock'n'roll",
        bitrate: 128,
    },
    Station {
        name: "GOLD 905",
        url: "https://playerservices.streamtheworld.com/api/livestream-redirect/GOLD905AAC.aac",
        genre: "Oldies",
        bitrate: 48,
    },
    Station {
        name: "BBC World Service",
        url: "https://stream.live.vc.bbcmedia.co.uk/bbc_world_service",
        genre: "News",
        bitrate: 128,
    },
    Station {
        name: "Radio Paradise (320k)",
        url: "https://stream.radioparadise.com/mp3-320",
        genre: "Eclectic",
        bitrate: 320,
    },
];
