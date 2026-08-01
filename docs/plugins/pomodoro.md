# Plugin Spec: Pomodoro

## Core Purpose
Focus timer with work/break sessions, per-user configurable durations, auto-start options, and per-day focus statistics.

## Technical Dependencies
| Crate | Purpose |
|---|---|
| `santui-ipc` | IPC protocol types + binary framing, shared UI primitives (palette, dim overlay, truncate) |
| `serde` / `serde_json` | Config + stats serialization for preferences |

## Architecture

### IPC Model
Standard IPC plugin — spawned as child process, communicates via stdin/stdout JSON + bincode. No network, no worker threads, no external dependencies. Date arithmetic is hand-rolled (no chrono).

### Module Structure
```
main.rs  → App, event loop, key handling
state.rs → Phase, TimerState, PomodoroConfig, DailyStats, PomodoroState
ui.rs    → rendering (session screen, settings dialog)
```

### State Management
```
App
├── state: PomodoroState
│   ├── data: PomodoroData
│   │   ├── config: PomodoroConfig   — work / short break / long break durations,
│   │   │                              long break after N sessions, auto-start flags (persisted)
│   │   └── stats: DailyStats        — date, sessions_completed, total_focus_secs,
│   │                                  total_break_secs (persisted, per-day)
│   ├── phase: Phase                 — Work / ShortBreak / LongBreak
│   ├── timer_state: TimerState      — Idle / Running / Paused / Finished
│   ├── remaining_secs               — countdown for the active phase
│   ├── sessions_done                — completed work sessions since last long break
│   ├── show_settings / settings_cursor
│   └── last_second                  — tick guard for 1s re-render
├── theme / area / dirty / cached_commands
└── pending_request                  — PluginRequest (DbGet/DbSet "pomodoro")
```

### Thread Model
- **Main thread only**: single-threaded event loop, rendering, key handling. No worker threads, no network.

### Tick & Countdown
- `HostMsg::Tick` → `handle_tick()` compares current epoch second against `last_second`; marks dirty only on actual second change
- Running timer decrements `remaining_secs` once per second; on reaching zero → `TimerState::Finished`, increments `sessions_completed`, accumulates focus/break seconds into `DailyStats`, and saves

### Daily Stats Rollover
- Stats are per-day (`date` = `YYYY-MM-DD`). On load and on every tick, a date change resets `DailyStats` (keeps the counter correct even if santui stays open past midnight) and schedules a save

### Preferences Persistence
- Key: `pomodoro` — single JSON blob containing `PomodoroData` (config + daily stats)
- Loaded on init via `PluginRequest::DbGet { key: "pomodoro" }`
- Stored via `PluginRequest::DbSet` on phase transitions (skip, finish, auto-start), settings changes, and daily stat rollover
- First run (`None` response) → defaults (25/5/15 min, long break after 4 sessions, auto-start off)
- Corrupt or missing JSON falls back to defaults without panicking

## Features
- Full-window panel with title (same visual language as other stable plugins); content block vertically centered for any window height
- Phase display with semantic colors: Work = `theme.accent`, Short Break = `theme.success`, Long Break = `theme.highlight`
- Countdown (MM:SS) with a centered progress bar and the percentage centered below it
- Work-cycle session dots (`●` completed / `○` upcoming, one per work session in the cycle) — falls back to `n / N sessions` text when the cycle exceeds 12 dots
- Today's stats line: sessions, focus time, and break time (break time was previously collected but never displayed)
- Phase durations summary (`25m work • 5m break • 15m long`) at the bottom of the panel
- `space` — start / pause / resume; on a Finished timer, advance to the next phase
- `s` — skip the current phase (advances the timer)
- `r` — reset the current session back to Idle at full duration
- `,` — open settings dialog (dimmed overlay + border)
- Settings dialog (6 rows, `↑↓`/`jk` navigate, `←→` adjust, `esc` close & save) with a muted description of the selected row at the bottom:
  1. Work duration (minutes, floor 1)
  2. Short break duration (minutes, floor 1)
  3. Long break duration (minutes, floor 1)
  4. Long break after N sessions (floor 1)
  5. Auto-start breaks after work finishes (toggle)
  6. Auto-start work after breaks finish (toggle)
- Long break replaces the short break once `sessions_done` reaches the configured threshold, then the counter resets
- Preferences + daily statistics persisted per user via the central DB

## Constraints & Rules
- `consumed` must be `false` on Esc from the main screen (pass through to host to close plugin)
- Esc inside the settings dialog is consumed internally (closes dialog + saves)
- All other keys inside the settings dialog are consumed (modal)
- Durations are clamped to a 1-minute floor; no negative values possible
- Stats roll over to a new day on the first tick after midnight, not only on restart
