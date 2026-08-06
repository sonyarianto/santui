# Plugin Spec: System Monitor

## Core Purpose
Real-time system monitoring: CPU, memory, disk, network, and top processes, with per-screen detail views and 60-second history sparklines.

## Technical Dependencies
| Crate | Purpose |
|---|---|
| `santui-ipc` | IPC protocol types + binary framing, shared UI primitives (draw_panel, text_at, push_text, truncate) |
| `sysinfo` | Cross-platform CPU / memory / disk / network / process sampling |

## Architecture

### IPC Model
Standard IPC plugin — spawned as child process, communicates via stdin/stdout JSON + bincode. Sampling happens on the main thread once per second (tick-driven), no worker threads.

### Module Structure
```
main.rs   → App, event loop, key handling, screen switching
sampler.rs → sysinfo sampling + byte formatting (fmt_bytes)
state.rs  → snapshots, metric history (60s ring), Screen / SortBy enums
ui.rs     → rendering (overview + 5 detail screens)
```

### State Management
```
App
├── state: SysMonState
│   ├── snapshot: SystemSnapshot  — cpu / mem / disks / net / processes / hostname / load avg
│   ├── history: MetricHistory    — 60-sample ring buffers for CPU, RAM, net RX/TX
│   ├── screen: Screen            — Overview / CpuDetail / MemDetail / DiskDetail / NetDetail / ProcessList
│   ├── process_sort: SortBy      — CPU % / Memory / Name
│   ├── selected_process          — process list cursor
│   └── last_second               — tick guard for 1s sampling
├── sampler: Sampler              — sysinfo handles + cached process top-10
├── theme / area / dirty / cached_commands
└── pending_request               — always None (stateless, no persistence)
```

### Thread Model
- **Main thread only**: single-threaded event loop; sampling and rendering are both tick-driven on the main thread.

### Sampling & Tick
- `HostMsg::Tick` → `handle_tick()` compares current epoch second against `last_second`; samples only on actual second change
- Full process list is refreshed only on Overview / ProcessList screens; the top-10 process table is cached between refreshes (cheap ticks on detail screens)
- History ring buffers are capped at `HISTORY_LEN` (60 samples)

### Preferences Persistence
- None — the plugin is stateless; no `DbGet`/`DbSet` requests are ever issued.

## Features
- Full-window panel with title ("System Monitor") using the shared `ui::draw_panel` (same visual language as other stable plugins); inner panels share the same component
- Overview: Computer info (name/OS/uptime), CPU / Memory / Disk / Network 4-panel row with usage bars + 60s sparklines, top-10 processes list
- Color-coded bars: `theme.success` (<60%), `theme.highlight` (60-80%), `theme.error` (>80%); disk uses its own thresholds
- Detail screens (`1`-`5`):
  1. CPU: per-core bars (2 columns), global %, sparkline, load average
  2. Memory: RAM/SWAP bars + usage text, 60s RAM/SWAP history sparklines
  3. Disk: mount / device / FS / used / total / usage table
  4. Network: per-interface ↓/↑ speed + totals table
  5. Processes: PID / name / CPU % / memory table, `s` cycles sort (CPU → Memory → Name), `↑↓`/`jk` navigate
- `Esc` on detail screens returns to Overview; `Esc` on Overview passes through to the host (plugin close)
- `status_hints` reflect the active screen (1-5 on overview, sort/navigate on process list)
- Load average and uptime formatting handle long-running systems

## Constraints & Rules
- `consumed` must be `false` on Esc from Overview (pass through to host to close plugin); every other handled key returns `true`
- Sampling is capped at 1 Hz via the `last_second` guard — never samples more than once per second
- Process refresh is skipped on detail screens (CPU/Mem/Disk/Net) to keep ticks cheap; the process table is only rebuilt when Overview or ProcessList is active
- No persistence: all state is ephemeral; the plugin never issues `DbGet`/`DbSet`
- Semantic `Theme` colors only — no hardcoded `Color::*` values
