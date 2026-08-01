# Plugin Spec: Music Preview

## Core Purpose
Search iTunes catalog and play 30-second track previews with inline mpv playback and auto-advance.

## Technical Dependencies
| Crate | Purpose |
|---|---|
| `santui-ipc` | IPC protocol types + binary framing |
| `libloading` | Dynamic FFI loading of `libmpv` |
| `ureq` | HTTP client for iTunes Search API |
| `serde` / `serde_json` | JSON parsing for API responses |

## Architecture

### IPC Model
Standard IPC plugin — spawned as child process, communicates via stdin/stdout JSON + bincode.

### State Management
```
App
├── state: MusicState       — search, results, now_playing, track_elapsed
├── mpv_tx                  — mpsc sender to mpv thread (MpvCommand)
├── mpv_wakeup              — cross-thread mpv wakeup handle
├── track_start: Instant    — timestamp for 30-sec preview timer
├── init_error              — mpv init failure message
└── rx_fetch                — mpsc receiver for async search results
```

### Thread Model
- **Main thread**: event loop, UI rendering, 30-sec preview timer (`Instant::elapsed`)
- **Mpv thread**: simple loop with `sleep(50ms)`, processes `MpvCommand` (LoadUrl, Stop)
- **Worker threads**: spawned per-search via `std::thread::spawn` for iTunes API call

### Mpv Integration
- Minimal mpv wrapper — no property observation, no event waiting
- Uses `MpvWakeup` to wake mpv thread when new URL is queued
- No `MPV_EVENT_END_FILE` handling — auto-advance is purely timer-based (30 sec)

### Auto-Advance Logic
```
┌──────────────────────────────────────────────┐
│ track_start.elapsed() >= PREVIEW_DURATION(30s) │
│         → advance_to_next_track()              │
│         → wraps around to index 0 at end       │
└──────────────────────────────────────────────┘
```

## Features
- Search iTunes catalog by artist/song name
- Display results with track name, artist, album, genre
- Play 30-second preview via mpv
- Stop playback (`s` key)
- Auto-advance to next track after 30 seconds (wraps around)
- Centered error display when mpv initialization fails
- Clear results (`c` key) stops playback + resets state

## Constraints & Rules
- No `MPV_EVENT_END_FILE` handling — relies solely on timer for auto-advance
- `consumed` must be `false` on Esc (search mode exit consumes, main view Esc passes through)
- Search query matching prevents stale results from overwriting newer searches
- mpv thread is silent (no event processing) — purely command-driven
