# Ideas

## Home screen plugin widget + scrollability

**Status:** idea

Show a panel of currently installed plugins on the home screen (splash), below the logo.

### Why

The home screen is currently static — ASCII logo + tagline + version. Installed plugins are only accessible via the command palette (`Ctrl+P`). A dedicated panel would make the launcher nature of Santui visible immediately: see what's installed, which are running, and get a quick shortcut hint.

### How

**Current splash layout** (`screens.rs:99-106`):

```
Fill(1)     ← top spacer
Length(8)   ← logo + tagline + version
Fill(1)     ← bottom spacer
```

**Proposed layout:**

```
Fill(1)           ← top spacer
Length(8)         ← logo + tagline + version
Length(N)         ← "Plugins" panel (List widget, scrollable)
Fill(1)           ← bottom spacer
```

**Plugin panel** — a ratatui `List` or `Paragraph` iterating over installed plugins:

| Column | Source |
|--------|--------|
| name | `InstalledPlugin::name` or humanized `id` |
| status | "running" / "stopped" — check `PluginManager::find_by_id()` |
| shortcut | e.g. `1`, `2`, `3` — future hotkey to activate directly |

The data already exists:
- `PluginManager::dynamic_items()` → enabled installed plugins
- `PluginManager::plugins` → currently loaded/running
- `Registry::installed` → full list with `enabled`, `version`, `path`, `id`, `name`

### Scrollability

If the terminal is too short or too many plugins are installed, the panel overflows. Ratatui `Paragraph`/`List` supports `.scroll((y, 0))`. Same pattern as the palette, theme picker, and registry overlays.

Two possible scroll models:

1. **Panel-scroll (recommended start):** Only the plugin list scrolls within its `Length(N)` slot. Logo stays fixed. Simple, same pattern as overlays.

2. **Full-page scroll:** The entire home screen (logo + plugin list) scrolls as one unit. Would need to wrap all content into a single scrollable widget. More refactoring, but useful if we later add more sections below plugins.

### Data flow

```
Registry::installed
       │
       ▼
PluginManager::refresh_dynamic_items()  ← called at startup + on registry change
       │
       ▼
Home screen render reads PluginManager for:
  - list of enabled installed plugins (name + id)
  - which are currently loaded (find_by_id)
  - renders as a scrollable list
```

### Open questions

- Hotkey shortcuts to activate a plugin directly from the home screen? (e.g. `1`–`9`)
- Show enabled + disabled plugins, or only enabled?
- Show plugin version? Description?
- Auto-scroll to newly installed plugin after registry install?

---

## Official Plugins

**Status:** planned

Santui should ship a curated set of official plugins — headless IPC binaries distributed via the registry. Each plugin follows the same pattern as `santui-radio-streaming-player`: depends on `santui-ipc` (protocol types only, no ratatui), runs as a child process, communicates over JSON-lined stdout/stdin, and returns `Vec<RenderCmd>` per tick.

### RSS Reader

**Why:** Read feeds without leaving the terminal. Fits Santui's "terminal home base" concept.

**Plan:**
- New crate: `crates/plugins/rss-reader/`
- Dependencies: `santui-ipc`, `ureq` (HTTP), `rss` crate (feed parsing), `serde`/`serde_json`
- Feeds stored in a simple JSON file or SQLite (via `rusqlite` with `bundled` feature)
- UI: two-panel — left = feed list (with unread count), right = article list. Press Enter to show article body in a scrollable view
- IPC state machine: `Init` → load feeds from disk; `Tick` → poll feeds for new articles (throttled, e.g. every 5 min); `Key` → navigate panels, open articles, mark read
- Challenges: background polling without blocking render; storing read/unread state
- Nice-to-haves: OPML import/export, article caching offline

### SSH Manager

**Why:** Quick SSH host access from the command palette. No more `~/.ssh/config` grepping or `ssh` history scrolling.

**Plan:**
- New crate: `crates/plugins/ssh-manager/`
- Dependencies: `santui-ipc`, `serde`/`serde_json`
- Reads `~/.ssh/config` to discover hosts (parse `Host` blocks), optionally `known_hosts`
- UI: scrollable list of hosts grouped by category (from `# comments` in config), searchable via palette
- Activation: pressing Enter on a host either:
  - Launches a new terminal window with `ssh user@host` (platform-specific: `cmd /c start` on Windows, `x-terminal-emulator` on Linux, `open -a Terminal` on macOS)
  - Or copies the SSH command to clipboard for manual paste
- Challenges: no IPC dep needed for clipboard or terminal launch — just `std::process::Command`
- Nice-to-haves: connection history with timestamps, bookmark/favorite hosts, SSH config validation

### Podcast Player

**Why:** Audio content in the terminal, same pattern as radio but subscription-based. Natural companion to the radio plugin.

**Plan:**
- New crate: `crates/plugins/podcast-player/`
- Dependencies: `santui-ipc`, `ureq`, `rss` (for podcast RSS feeds), `libloading` (for libmpv), `serde`/`serde_json`
- Reuses libmpv audio playback from radio plugin (same FFI pattern, shareable as a small helper crate)
- Subscription list persisted to JSON/SQLite — feed URL, last fetched, per-episode play state
- UI: three panels — subscriptions (with unread count) → episode list (with progress/duration) → now playing (title, progress bar, controls)
- IPC state machine: `Tick` → check for new episodes (throttled); `Key` → navigate, play/pause, seek, change speed
- Challenges: downloading vs streaming episodes; storing playback position; libmpv lifecycle for audio-only
- Nice-to-haves: playback speed control, sleep timer, chapter markers, OPML import

### IRC Client

**Why:** IRC is still the default chat for open-source. Santui as a persistent IRC bouncer + client fits well.

**Plan:**
- New crate: `crates/plugins/irc-client/`
- Dependencies: `santui-ipc`, `irc` crate (or raw TCP with `std::net`), `serde`/`serde_json`
- Connection management: one thread per server, non-blocking channel-based message passing
- UI: classic IRC layout — server/channel list (left) → message log (center) → input line (bottom). Tab-complete nicks
- IPC state machine: `Init` → connect to configured servers; `Tick` → drain message queue; `Key` → type messages, switch channels, `/commands`
- Challenges: async I/O in a headless process; message backlog; nick coloring; IRCv3 capabilities (SASL, message tags)
- Nice-to-haves: DCC, notification on mention, logging to file, ZNC bouncer integration

### IPTV

**Why:** Watch IPTV streams in-terminal (or at least control playback). Natural extension of the radio plugin's libmpv usage.

**Plan:**
- New crate: `crates/plugins/iptv-player/`
- Dependencies: `santui-ipc`, `libloading` (libmpv), `ureq`, `serde`/`serde_json`
- Parses M3U/M3U8 playlists (local file or URL) to populate channel list with EPG data (XMLTV)
- UI: channel list (with current program info if EPG available) → now playing (title, progress if VOD)
- Reuses radio plugin's libmpv wrapper — primarily audio but with `--vo=gpu` for video (video rendered externally by mpv window, not in-terminal)
- IPC state machine: same as radio but with channel groups/categories from M3U groups
- Challenges: mpv `--vo` window management; EPG parsing; channel zapping speed
- Nice-to-haves: recording, timeshift, catch-up TV

### Chat / Messaging System

**Why:** A native terminal messaging system — group chats, private messages, user presence. Think Slack/Discord for the terminal. Santui becomes your communication hub alongside everything else.

**Architecture:** [Centrifugo](https://centrifugal.dev) as the real-time messaging layer + a lightweight Rust API server for business logic.

```
santui (client)
  └─ IPC plugin connects via WebSocket ──► Centrifugo
                                              ▲
                                              │ HTTP API (publish)
                                              ▼
                                    santui-chat-api (Rust)
                                      (users, rooms, invites, JWT)
                                              │
                                              ▼
                                           SQLite
```

**Why Centrifugo instead of a custom server:**
- Handles WebSocket connections, multiplexing, reconnection, presence out of the box
- Channel-based PUB/Sub — each room is a channel, automatic fan-out
- Built-in history with recovery on reconnect
- JWT-based auth
- Horizontal scaling if needed (Redis/PG broker)
- Battle-tested, open source (Go)

**API server (`santui-chat-api`):**
- New crate: `crates/chat/api/`
- Dependencies: `axum` (HTTP), `serde`/`serde_json`, `rusqlite` (users, rooms), `jsonwebtoken` (JWT), `argon2` (passwords), `reqwest` (call Centrifugo publish API)
- Endpoints: `POST /register`, `POST /login` → returns JWT (used by client to connect to Centrifugo), `POST /rooms`, `POST /rooms/:id/invite`, `GET /rooms`, `GET /rooms/:id/history`
- When a user sends a message: client publishes directly to Centrifugo channel via its client API (or POSTs to API server which publishes via Centrifugo server API)
- Rooms stored in SQLite with member list; private chats auto-created on first DM
- Centrifugo config: one namespace per room type, presence enabled, history with recovery

**Client (`santui-chat-client`):**
- New crate: `crates/plugins/chat-client/`
- Dependencies: `santui-ipc`, `tungstenite` (WebSocket), `serde`/`serde_json`, `ureq` (REST calls to API server)
- Uses Centrifugo client protocol (JSON) — connect with JWT, subscribe to channels, receive messages
- UI: three-panel layout — room list (left, with unread badges) → message log (center, scrollable) → input bar (bottom). Rooms split into "Direct Messages" and "Group Chats"
- Connection flow: `Init` → REST login/register to get JWT → WebSocket connect to Centrifugo → subscribe to joined room channels → drain messages from WS thread into msg queue
- Presence: Centrifugo provides `presence` API per channel — show online indicators
- Notifications: Santui status bar hint when new message arrives in inactive room
- IPC state machine: `Tick` → drain incoming message queue from WS reader thread; `Key` → navigate rooms, scroll, type, send
- Challenges: WebSocket thread management (same pattern as radio's MPV thread); typing indicators; multi-line message input; emoji
- Storage: cache recent messages locally (JSON) for fast startup and offline access

**MVP scope:**
1. Centrifugo running as Docker/compose or native binary
2. API server with register, login, create room, invite, list rooms
3. Client connects, subscribes to rooms, sends/receives messages
4. Private DMs (auto-room between two users)
5. Unread badges, basic presence

**Post-MVP:**
- File/image upload + inline display
- Message reactions, threads, replies
- Read receipts
- Search history
- User roles (admin, moderator)
- Desktop notifications

### System Monitor

**Why:** See resource usage at a glance — CPU, memory, disk, network, processes. Fits the "terminal home base" concept; no external deps needed since data is local.

**Plan:**
- New crate: `crates/plugins/system-monitor/`
- Dependencies: `santui-ipc`, `sysinfo` (cross-platform system info), `serde`/`serde_json`
- Data collection via `sysinfo`: `System::new_all()` polls CPU load, memory/swap usage, disk mounts/usage, network I/O counters, process list
- UI: tabbed or vertical panels — CPU history sparkline, memory/swap bar, disk usage bars, network up/down, top processes by CPU/mem. All rendered as `RenderCmd::Rect` (filled bars) + `RenderCmd::Text` (labels)
- IPC state machine: `Init` → calibrate (get core count, total RAM); `Tick` → poll sensors, compute rates (delta since last tick), build render commands; `Key` → switch tab, sort processes, kill a process
- No external storage needed — state is transient
- Challenges: cross-platform process names; CPU rate calculation (need idle vs total delta); network speed smoothing
- Nice-to-haves: process tree view, GPU stats (nvidia-smi / AMD), battery, temperature sensors, alert threshold (e.g. highlight if CPU > 90%)

### Weather

**Why:** Current conditions + forecast right in the terminal. Saves a browser tab. Quick glance when starting the day.

**Plan:**
- New crate: `crates/plugins/weather/`
- Dependencies: `santui-ipc`, `ureq` (HTTP), `serde`/`serde_json`. No external API SDK needed — raw HTTPS to Open-Meteo (free, no API key) or OpenWeatherMap
- API choice: **Open-Meteo** (free, no key, no rate limits for personal use) — `/v1/forecast?latitude=...&longitude=...&current=temperature_2m&daily=temperature_2m_max`
- Location: prompt user for city on first launch → geocode with a simple CSV/cache or Open-Meteo geocoding API → store lat/lon in a JSON file
- UI: current conditions (large temp, weather icon, feels-like, humidity, wind) + 7-day forecast (high/low, precipitation chance, icon). Use Unicode weather symbols (☀ 🌤 ☁ 🌧 ⛈ ❄)
- IPC state machine: `Init` → load cached location + last weather (if within 30 min, show stale data while fetching); `Tick` → if cache expired, fetch new forecast in a background thread; `Key` → change location, switch °C/°F, refresh
- Challenges: async HTTP without blocking render (use a `mpsc` channel and fetch in a thread, same pattern as radio's iTunes lookup); graceful degradation when offline; rate limiting
- Nice-to-haves: hourly forecast, weather alerts, multiple saved locations, sunrise/sunset, UV index, air quality, moon phase

### More Ideas

| Plugin | Pitch | Key dep | Difficulty |
|--------|-------|---------|------------|
| **Notes** | Plain-text / Markdown notes with local filesystem sync. Quick note-taking from palette. | `syntect` (syntax highlight) | Medium |
| **Calculator** | Inline expression evaluation in the palette. | `meval` or `fend` | Easy |
| **File Manager** | Directory tree, file preview, basic operations (copy/move/delete). | `walkdir` | Medium |
| **Clipboard History** | Track clipboard changes, searchable history, quick paste. | `arboard` | Medium |
| **Todo/Task Manager** | Simple todo.txt or Taskwarrior integration. | — | Medium |
| **Dictionary/Thesaurus** | Word lookup from palette. Offline dictionary (e.g. `dict` crate or bundled data). | `ureq` or bundled DB | Easy |
| **GitHub Notifications** | Unread notifications, PR review reminders, issue mentions. | `ureq` + GitHub API | Medium |
| **Hacker News / Reddit** | Browse top stories, comments. Read in terminal. | `ureq` | Easy |
| **Password Manager** | GPG-encrypted password store (e.g. `pass` compat), search + copy to clipboard. | `gpgme` or subprocess | Medium |
| **Timer / Pomodoro** | Countdown timer with desktop notification on finish. | — | Easy |
| **Music Player (local)** | Play local music files via libmpv, browse by folder/tags. | `libloading`, ` lofty` (tags) | Medium |

All plugins follow the same architecture: `crates/plugins/<name>/` binary crate → depends on `santui-ipc` → registered in workspace `Cargo.toml` → published via the plugin registry manifest.
