# Plugin Spec: Quran Reader

## Core Purpose
Browse and read Quran surahs with Arabic text, translations, and ayah-by-ayah recitation playback via mpv.

## Technical Dependencies
| Crate | Purpose |
|---|---|
| `santui-ipc` | IPC protocol types + binary framing |
| `libloading` | Dynamic FFI loading of `libmpv` |
| `ureq` | HTTP client for alquran.cloud API |
| `serde` / `serde_json` | JSON parsing + preferences serialization |

## Architecture

### IPC Model
Standard IPC plugin — spawned as child process, communicates via stdin/stdout JSON + bincode.

### State Management
```
App
├── prefs: Preferences         — translation, reciter, display_mode, last position
├── screen: Screen             — SurahList / Reader / TranslationPicker / ReciterPicker
├── surahs: Vec<SurahSummary>  — cached surah list
├── content_cache: BTreeMap    — cached surah content by surah number
├── selected_surah / selected_ayah / scroll
├── tx_mpv / rx_mpv           — mpsc channels to/from mpv thread
├── audio_state               — Stopped / Buffering / Playing / Paused / Error
├── play_surah_mode           — auto-advance through all ayahs
└── repeat_ayah               — loop current ayah
```

### Thread Model
- **Main thread**: event loop, UI rendering, key handling
- **Mpv thread**: `wait_event_raw` loop with 0.1s timeout, processes `MpvCmd` (Load, TogglePause, Stop, Quit), emits `MpvMsg` (Started, EndFile, Error)
- **Worker threads**: spawned per-fetch (surah list + per-surah content with 3 parallel API calls)

### Mpv Integration
- Full wrapper matching stable plugin pattern (set_property, observe_property, wakeup, volume)
- Currently uses basic subset: load_url, toggle_pause, stop, wait_event_raw
- No property observation needed (no streaming metadata)
- `MPV_EVENT_END_FILE` triggers next ayah in surah-mode or repeat

### Data Flow (Surah Content Fetch)
```
open_selected_surah()
  ├── cache hit → immediate (Screen::Reader)
  └── cache miss → spawn thread
       ├── fetch_json(arabic)    — quran-uthmani edition
       ├── fetch_json(translation) — user-selected translation
       └── fetch_json(audio)     — user-selected reciter
       └── parse_surah_ayahs() → SurahContent → cache → Screen::Reader
```

### Preferences Persistence
- Key: `quran-reader-preferences`
- Stored via `PluginRequest::DbSet` on every ayah navigation + setting change
- Loaded on init via `PluginRequest::DbGet`
- Fields: translation_edition, reciter, display_mode, last_surah, last_ayah

## Features
- Browse 114 surahs with search by name/number
- Read ayah-by-ayah with Arabic, translation, or both display modes
- Play ayah recitation via mpv (space = toggle, x = stop)
- Play entire surah sequentially (`a` key)
- Repeat single ayah (`r` key toggle)
- Select from 3 translations (Sahih, Asad, Indonesian)
- Select from 4 reciters (Alafasy, Abdul Basit, Husary, Minshawi)
- Position memory (resume last surah + ayah on next open)
- Content caching (fetched surah stays cached until translation/reciter change)

## Constraints & Rules
- `consumed` must be `false` on Esc when search is empty (pass through to host)
- Surah mode + repeat mode are independent: repeat takes priority over advance
- Audio unavailable does NOT block reading — recitation is optional
- Three parallel API calls per surah (arabic + translation + audio) — all required for content
