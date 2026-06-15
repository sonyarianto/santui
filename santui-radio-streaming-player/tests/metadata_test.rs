use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// Pull in the player module directly
mod player;
use player::Mpv;

const MPV_EVENT_SHUTDOWN: u32 = 1;
const MPV_EVENT_FILE_LOADED: u32 = 6;
const MPV_EVENT_PROPERTY_CHANGE: u32 = 13;
const MPV_EVENT_END_FILE: u32 = 25;

#[test]
fn test_metadata() {
    let (mpv, warns) = Mpv::new().expect("mpv init");
    for w in &warns {
        eprintln!("  ⚠️  {w}");
    }

    let _ = mpv.observe_property(0, "metadata");
    let _ = mpv.observe_property(1, "media-title");
    let _ = mpv.observe_property(2, "volume");
    let _ = mpv.set_volume(75);

    let (_tx_msg, rx_msg) = mpsc::channel();
    let (tx_cmd, rx_cmd) = mpsc::channel::<&'static str>();

    let url = "https://cdnindo.klikhost.net/8766/stream";

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
                        Ok(None) => {
                            eprintln!("[test] metadata_title = None");
                            match mpv.media_title() {
                                Ok(Some(t)) => eprintln!("[test] media_title = {t:?}"),
                                Ok(None) => eprintln!("[test] media_title = None"),
                                Err(e) => eprintln!("[test] media_title error: {e}"),
                            }
                        }
                        Err(e) => eprintln!("[test] metadata_title error: {e}"),
                    }
                }
                if id == MPV_EVENT_PROPERTY_CHANGE {
                    let prop: &player::MpvEventProperty = unsafe { &*(ev.data as *const _) };
                    let name = unsafe {
                        std::ffi::CStr::from_ptr(prop.name)
                            .to_string_lossy()
                            .to_string()
                    };
                    eprintln!("[test] PROP_CHANGE name={name}");
                }
                if id == MPV_EVENT_END_FILE {
                    let ef: &player::MpvEventEndFile = unsafe { &*(ev.data as *const _) };
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
    tx_cmd.send(url).ok();

    // Wait up to 30 seconds for metadata
    for _ in 0..300 {
        thread::sleep(Duration::from_millis(100));
    }

    eprintln!("[test] done");
}
