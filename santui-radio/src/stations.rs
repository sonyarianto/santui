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
        url: "https://nbs.jejakdigital.my.id:8443/stream",
        genre: "Pop",
        bitrate: 128,
    },
    Station {
        name: "GOLD 905",
        url: "https://playerservices.streamtheworld.com/api/livestream-redirect/GOLD905AAC.aac",
        genre: "Gold",
        bitrate: 48,
    },
    Station {
        name: "BBC World Service",
        url: "https://stream.live.vc.bbcmedia.co.uk/bbc_world_service",
        genre: "News",
        bitrate: 128,
    },
    Station {
        name: "Jazz24",
        url: "https://live-mzsy.cloud.alibabadns.com/jazz24_aac/high/playlist.m3u8",
        genre: "Jazz",
        bitrate: 128,
    },
    Station {
        name: "Radio Paradise (320k)",
        url: "https://stream.radioparadise.com/mp3-320",
        genre: "Eclectic",
        bitrate: 320,
    },
];
