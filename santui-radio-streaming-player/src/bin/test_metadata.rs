use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use santui_radio_streaming_player::player::Mpv;
use santui_radio_streaming_player::player::{MpvEventProperty, MpvEventEndFile};

const MPV_EVENT_SHUTDOWN: u32 = 1;
const MPV_EVENT_FILE_LOADED: u32 = 6;
const MPV_EVENT_PLAYBACK_RESTART: u32 = 18;
const MPV_EVENT_PROPERTY_CHANGE: u32 = 22;
const MPV_EVENT_END_FILE: u32 = 25;

fn main() {
    let (mpv, warns) = Mpv::new().expect("mpv init");
    for w in &warns {
        eprintln!("  ⚠️  {w}");
    }

    let r = mpv.observe_property(0, "metadata");
    eprintln!("[test] observe metadata: {r:?}");
    let r = mpv.observe_property(1, "media-title");
    eprintln!("[test] observe media-title: {r:?}");
    let r = mpv.observe_property(2, "volume");
    eprintln!("[test] observe volume: {r:?}");
    let r = mpv.set_volume(75);
    eprintln!("[test] set_volume: {r:?}");

    let (tx_cmd, rx_cmd) = mpsc::channel::<&'static str>();

    thread::spawn(move || {
        loop {
            let ev = mpv.wait_event_raw(0.1);
            if let Some(ev) = ev {
                let id = ev.event_id;
                eprintln!("[test] event id={id}");
                if id == MPV_EVENT_SHUTDOWN { break; }
                if id == MPV_EVENT_FILE_LOADED {
                    eprintln!("[test] FILE_LOADED");
                    match mpv.metadata_title() {
                        Ok(Some(t)) => eprintln!("[test] metadata_title = {t:?}"),
                        Ok(None) => eprintln!("[test] metadata_title = None"),
                        Err(e) => eprintln!("[test] metadata_title error: {e}"),
                    }
                }
                if id == MPV_EVENT_PLAYBACK_RESTART {
                    eprintln!("[test] PLAYBACK_RESTART");
                    match mpv.metadata_title() {
                        Ok(Some(t)) => eprintln!("[test] metadata_title = {t:?}"),
                        Ok(None) => eprintln!("[test] metadata_title = None"),
                        Err(e) => eprintln!("[test] metadata_title error: {e}"),
                    }
                    match mpv.media_title() {
                        Ok(Some(t)) => eprintln!("[test] media_title = {t:?}"),
                        Ok(None) => eprintln!("[test] media_title = None"),
                        Err(e) => eprintln!("[test] media_title error: {e}"),
                    }
                }
                if id == MPV_EVENT_PROPERTY_CHANGE {
                    let prop: &MpvEventProperty = unsafe { &*(ev.data as *const _) };
                    let name = unsafe {
                        std::ffi::CStr::from_ptr(prop.name)
                            .to_string_lossy()
                            .to_string()
                    };
                    eprintln!("[test] PROP_CHANGE name={name}, format={}", prop.format);
                }
                if id == MPV_EVENT_END_FILE {
                    let ef: &MpvEventEndFile = unsafe { &*(ev.data as *const _) };
                    eprintln!("[test] END_FILE reason={}", ef.reason);
                }
            }
            while let Ok(url) = rx_cmd.try_recv() {
                eprintln!("[test] loading {url}");
                let _ = mpv.load_url(url);
            }
        }
        mpv.destroy();
    });

    thread::sleep(Duration::from_millis(500));
    eprintln!("[test] sending load URL");
    let url = "https://cdnindo.klikhost.net/8766/stream";
    tx_cmd.send(url).ok();

    // Wait 30 seconds
    for _ in 0..300 {
        thread::sleep(Duration::from_millis(100));
    }

    eprintln!("[test] done");
}
