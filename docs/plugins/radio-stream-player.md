# Plugin Spec: Radio Stream Player

## Core Purpose
Search, browse, and play thousands of internet radio stations with metadata display, lyrics lookup, and favorites management.

## Technical Dependencies
| Crate | Purpose |
|---|---|
| `santui-ipc` | IPC protocol types + binary framing |
| `libloading` | Dynamic FFI loading of `libmpv` |
| `ureq` | HTTP client for station list, iTunes lookup, LRCLib |
| `rusqlite` (bundled) | Local station database + favorites |
| `libc` | stderr redirect for mpv log suppression |
| `serde` / `serde_json` | JSON parsing for API responses + preferences |

## Architecture

### IPC Model
Standard IPC plugin — spawned as child process, communicates via stdin/stdout JSON + bincode.

### State Management
```
App
├── state: RadioState      — search, stations, song/lyrics, volume, favorites
├── tx_cmd / rx_msg        — mpsc channels to/from mpv thread
├── mpv_wakeup             — cross-thread mpv wakeup handle
├── mpv_heartbeat (AtomicU64) — heartbeat counter to detect stuck mpv thread
├── db: rusqlite::Connection — bundled radio station database
└── user: Option<UserData>
```

### Thread Model
- **Main thread**: event loop (HostMsg dispatch), rendering, UI logic
- **Mpv thread**: `wait_event_raw` loop with 0.1s timeout, processes `MpvCmd` (LoadUrl, Stop, SetVolume, Quit), emits `MpvMsg` (FileLoaded, Metadata, TrackInfo, Lyrics, EndFile, MpvReset)
- **Worker threads**: spawned per-metadata-change for iTunes track lookup + LRCLib lyrics fetch

### Mpv Integration
- Observes properties: `metadata`, `media-title`, `volume`
- Uses `MpvWakeup` for prompt command delivery
- Retry logic on failed `load_url` (3 attempts with backoff)
- Pre-drains stale events before loading a new URL
- Heartbeat monitoring detects and resets stuck mpv thread

## Features
- Search radio stations by name, country, language, tag
- Browse by country/category with accordion sidebar
- Play station via mpv with auto-reconnect on failure
- Real-time metadata display (song title, artist)
- iTunes track info lookup (album art, genre)
- LRCLib lyrics fetch with synced/plain support
- Favorites management with DB persistence
- Volume control (per-plugin, persisted)
- Mouse support (scroll, click rows)
- Preferences persisted via `PluginRequest::DbSet`

## Constraints & Rules
- `consumed` flag must be `true` on Esc to prevent host from closing the plugin (can_background)
- Mpv thread heartbeat must never false-positive mark a healthy thread as stuck
- Favorites loaded on init via `PluginRequest::DbGet { key: "favorites" }`
- Station DB (`radio_stream_stations.db`) bundled in `native/` directory
- Metadata seq number prevents stale iTunes/lyrics results from overwriting newer metadata
