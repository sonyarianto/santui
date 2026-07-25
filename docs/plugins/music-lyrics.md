# Plugin Spec: Music Lyrics

## Core Purpose
Search song lyrics via LRCLib API and display them with scrollable text view.

## Technical Dependencies
| Crate | Purpose |
|---|---|
| `santui-ipc` | IPC protocol types + binary framing |
| `ureq` | HTTP client for LRCLib API |
| `serde` / `serde_json` | JSON parsing for API responses |

## Architecture

### IPC Model
Standard IPC plugin — spawned as child process, communicates via stdin/stdout JSON + bincode. No mpv dependency — purely HTTP-based.

### State Management
```
App
├── state: LyricsState       — search, results, lyrics display, scroll
├── rx_fetch                 — mpsc receiver for async search results
├── pending_request           — PluginRequest (unused currently)
└── pending_plugin_message    — PluginMessage (unused currently)
```

### Thread Model
- **Main thread**: event loop, UI rendering
- **Worker threads**: spawned per-search via `std::thread::spawn` for LRCLib API call
- No mpv thread — purely HTTP-based

### State Machine
```
Idle → (press '/') → SearchMode
SearchMode → (Enter + query) → Fetching → Done / Error
Done → (Enter on result) → LyricsView
LyricsView → (Esc) → Done (back to results)
LyricsView/Done → (c) → Idle (clear all)
```

### Module Structure
```
main.rs    → App, event loop, key handling
state.rs   → LyricsState, FetchState
ui.rs      → rendering functions, max_visible_tracks
lrclib.rs  → LRCLib API client (search, extract_lyrics)
```

## Features
- Search lyrics by song title/artist
- Display results list with track name, artist, album, duration
- View lyrics in scrollable text view (arrow keys, page up/down)
- Instrumental track detection with message override
- Synced lyrics (LRCLib `syncedLyrics`) auto-converted to plain text
- Stale result protection (query string matching prevents overwrite)
- Clear results (`c` key) resets all state

## Constraints & Rules
- `consumed` must be `false` on Esc from main view (search mode Esc consumed internally)
- Lyrics text is pulled from `plainLyrics` field first, falls back to `syncedLyrics` → plain conversion
- No mpv, no audio — pure text display
- Scroll is blocked when lyrics fit within visible area (no unnecessary scroll state)
