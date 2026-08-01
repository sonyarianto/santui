# Plugin Spec: World Clock

## Core Purpose
World timezone clock with a grid of clock cards, timezone search, and custom labels.

## Technical Dependencies
| Crate | Purpose |
|---|---|
| `santui-ipc` | IPC protocol types + binary framing, shared UI primitives (palette, dim overlay, truncate) |
| `chrono` | Time computation + UTC/local offset lookup |
| `chrono-tz` | IANA timezone database (`Tz`) with serde support |
| `serde` / `serde_json` | Clock list serialization for preferences |

## Architecture

### IPC Model
Standard IPC plugin — spawned as child process, communicates via stdin/stdout JSON + bincode. No network, no worker threads, no external dependencies.

### Module Structure
```
main.rs      → App, event loop, key handling
state.rs     → WorldTimeState, ClockEntry, Screen
timezones.rs → curated timezone list + search + city_name
ui.rs        → rendering (grid cards, search palette, rename popup)
```

### State Management
```
App
├── state: WorldTimeState
│   ├── clocks: Vec<ClockEntry>      — tz + label (persisted)
│   ├── date_format: DateFormat      — DayFirst / MonthFirst (persisted)
│   ├── hour_format: HourFormat      — Twelve / TwentyFour (persisted)
│   ├── selected: usize              — grid cursor
│   ├── screen: Screen               — Grid / Search / Rename(idx)
│   ├── search_query / search_results / search_cursor / search_scroll
│   ├── tick_counter                 — blink state (search + rename cursors)
│   ├── rename_buf
│   └── last_second                  — tick guard for 1s re-render
├── theme / area / dirty / cached_commands
└── pending_request                  — PluginRequest (DbGet/DbSet "clocks" / "date_format" / "hour_format")
```

### Thread Model
- **Main thread only**: single-threaded event loop, rendering, key handling. No worker threads, no mpv, no network.

### Time Refresh
- `HostMsg::Tick` → `handle_tick()` compares current epoch second against `last_second`; marks dirty only on actual second change (no wasted re-renders)
- Search cursor blink uses shared `ui::blink_cursor(tick_counter)`; re-rendered every 3rd tick while in Search or Rename screen (both have a blinking cursor)

### Preferences Persistence
- Key: `clocks` — tz + label list
- Key: `date_format` — `"day"` (Sat, 1 Aug 2026) or `"month"` (Sat, Aug 1 2026, default)
- Key: `hour_format` — `"12"` (2:05:09 PM) or `"24"` (14:05:09, default)
- Loaded on init via `PluginRequest::DbGet { key: "clocks" }`, then chained `DbGet "date_format"` → `DbGet "hour_format"` after each value arrives (one request in flight at a time)
- Stored via `PluginRequest::DbSet` on add/delete/rename (clocks), on `t` (date format), and on `f` (time format)
- First run (`None` response) → default clocks (UTC + first zone matching the local offset), then saved
- Invalid/missing `date_format` / `hour_format` values fall back to defaults without panicking

## Features
- Full-window panel with title (same visual language as other stable plugins), cards inside at `x + 2, y + 1`
- Grid of clock cards: label, UTC offset, HH:MM:SS time, date, DST indicator (`D` badge)
- Navigate grid with arrows/hjkl (wrap-around, column-aware)
- Search timezones (`a` key) — multi-token matching over city name + IANA name, case-insensitive, 60 curated zones
- Rename clocks (`r` key) — trimmed input, empty input cancels; `Ctrl+R` restores the default city name of the timezone
- Delete clocks (`d` key) — selection clamps after removal
- Toggle card date format (`t` key) — day-first ↔ month-first, persisted per user
- Toggle card time format (`f` key) — 24-hour ↔ 12-hour (AM/PM), persisted per user
- Duplicate timezone protection on add
- Preferences persisted across sessions via central DB

## Constraints & Rules
- `consumed` must be `false` on Esc from Grid (pass through to host to close plugin)
- Esc in Search/Rename is consumed internally (returns to Grid)
- Grid navigation column count derives from the same area width as rendering (`ui::grid_cols`) — navigation and highlight can never drift
- Empty-state prompt when no clocks: "Add a timezone (press 'a')"
- `tz` is the identity for dedup; labels are free-form
- Corrupt or missing `clocks` JSON in DB falls back to defaults without panicking
