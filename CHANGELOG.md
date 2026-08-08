## [unreleased]

### ⚙️ Miscellaneous

- Update changelog for v0.2.39
- Remove unused packages/winget directory

### 🐛 Bug Fixes

- Remove hardcoded version example from winget iss

### 📚 Documentation

- Remove outdated Windows block warning

### 🚀 Features

- Add winget publishing via Inno Setup installer

Full Changelog: [v0.2.39...](https://github.com/sonyarianto/santui/compare/v0.2.39...)
## [0.2.39] - 2026-08-08

### ⚙️ Miscellaneous

- Update changelog for v0.2.38
- Migrate workspace to Rust edition 2024
- Add scripts/check.sh fast verification set
- Update changelog for v0.2.39

### 🎨 Refactor

- Edition 2024 cleanup (match ergonomics, is_multiple_of, test hygiene)

### 🐛 Bug Fixes

- Keep Processes panel inside the window

### 📚 Documentation

- Sync docs and plugin template with current protocol and tooling

### 🚀 Features

- Promote to stable plugin
- Title Computer panel, remove inter-panel gap
- Scraper CLI args, enable search tests
- Battery, process paths, swap history, detail screens; fix table column alignment

Full Changelog: [v0.2.38...v0.2.39](https://github.com/sonyarianto/santui/compare/v0.2.38...v0.2.39)
## [0.2.38] - 2026-08-06

### ⚙️ Miscellaneous

- Update changelog for v0.2.37
- Remove stale audit config files (audit.toml is the active config)
- Update changelog for v0.2.38

### 🎨 Refactor

- Rename plugin from pomodoro-timer to pomodoro
- Remove radio plugin integration

### 🐛 Bug Fixes

- Use 'o' instead of ',' to open the settings dialog
- Exclude build artifacts from release leftover check

### 📚 Documentation

- Add total npm downloads badge
- Describe footer hints row in settings dialog

### 🚀 Features

- Automate release flow with pre-flight version checks
- Promote to stable plugin
- Redesign UX/UI
- Center progress bar and percentage below it
- Center the phase durations summary at the panel bottom
- Use bullet separators in stats and summary lines
- Redesign settings dialog
- Render settings dialog in the palette style
- Align settings dialog with the world-clock palette layout
- Drop esc hint from settings dialog hints row
- Tighten settings dialog spacing above the fields
- Add blank line before settings dialog hints row

Full Changelog: [v0.2.37...v0.2.38](https://github.com/sonyarianto/santui/compare/v0.2.37...v0.2.38)
## [0.2.37] - 2026-08-01

### ⚙️ Miscellaneous

- Update changelog for v0.2.36
- Stop publishing to crates.io, guard crates with publish = false
- Relicense project from MIT to AGPL-3.0
- Mark as stable plugin
- Update changelog for v0.2.37

### 🎨 Refactor

- Replace dot marker with semantic border selection
- Make draw_panel flexible for any panel style
- Use shared IPC ui helpers for content text

### 🐛 Bug Fixes

- Keep ellipsis on truncated table cells
- Account for ratatui column spacing in all plugins
- Blink rename cursor at the same rate as search
- Label Ctrl+R hint clearly and after save hint
- Drop redundant esc hint in rename popup
- Lowercase ctrl+r in rename hint
- Drop bullet separator from palette and theme picker footers
- Restore bullet separator in palette and theme picker footers
- Color plugin actions footer keys like the command palette
- Draw grid under rename popup so backdrop stays dimmed
- Clear frame each render so dialog backdrop stays dimmed
- Lowercase pgup/pgdn in add timezone footer hint
- Use month-first date format in clock cards
- Spell out DST badge on clock cards
- Dim DST badge to text_muted
- Drop unused palette_footer import in ui tests
- Dim action dialog field keys, brighten values without padding
- Single space around version arrow in action dialog
- Restore colon on track details keys

### 📚 Documentation

- Remove stale roadmap content and fix drifted plugin/dev docs
- Replace static screenshot with video demo on README and website
- Update DST badge reference
- Mandate verifying claims against code before asserting
- Drop hardcoded plugin counts from AGENTS.md
- Remove hardcoded plugin count, align template manifest metadata
- Point manifest docs at plugins-manifest.json source of truth
- Fix plugins.json guidance, document key-value row convention

### 🚀 Features

- Wrap clock grid in full-window panel with title
- Redesign rename popup to match palette style
- Restore default label with shift+R in rename popup
- Use Ctrl+R to restore default label
- Standardize palette footer rendering
- Toggle card date format with t key
- Toggle 12/24 hour clock with f key

Full Changelog: [v0.2.36...v0.2.37](https://github.com/sonyarianto/santui/compare/v0.2.36...v0.2.37)
## [0.2.36] - 2026-08-01

### ◀️ Revert

- Remove radio.log from gitignore - it was a one-off
- Use cargo install --locked for cargo-audit, binstall failed in CI
- Remove broken time-pos/duration polling, revert to END_FILE-driven advancement

### ⚙️ Miscellaneous

- Use cargo-binstall for faster audit install
- Build only stable plugins in release, not full workspace
- Remove radio.log from tracking, add to gitignore
- Update changelog for v0.2.35
- Isolate release assets in _assets/ to avoid uploading repo files
- Update changelog for v0.2.36

### 🎨 Refactor

- Extract player/api/types/ui modules, mark stable
- Rename santui-quran-reader to santui-quran
- Remove redundant 'Loaded N surahs' status
- Remove blank line between border and header
- Overlay lyrics panel on dimmed background
- Centralize draw_panel in santui-ipc with PanelOpts, remove per-plugin duplicates
- Centralize UI primitives in santui-ipc (blink cursor, right-align, scroll pct, table styles, heart overlay, dim overlay)
- Drop per-plugin truncate clones, use shared ui::truncate; unify ellipsis style to '...'
- Centralize clipboard into santui-ipc, drop 31 identical per-plugin clones
- Centralize url_encode, strip_timestamps, to_rc, max_visible_tracks into santui-ipc
- Centralize libmpv wrapper into santui-ipc::mpv, drop 4 per-plugin copies; fix wrong mpv event constants
- Centralize shared utils into santui-ipc (theme, time, platform, ui, text, protocol helpers)
- Centralize search-mode key handling into santui-ipc::search
- Remove vestigial lyrics focus switching, drop Tab hints
- Remove vestigial details focus switching, drop Tab hints
- Lyrics as overlay panel snapped right instead of full-screen view

### 🎨 Styling

- Reader header with '-' separators and dimmed mode/audio keys

### 🐛 Bug Fixes

- Literal \n in cliff.toml changelog template
- Log-viewer UI standards and quran surah list active ayah column
- Reorder hints, add p key for play
- Reorder station panel hints, / search before ↵ play
- Remove p alias, ↵ play only
- Stations panel uses bright colors under dim overlay
- Artist text always muted in lyrics overlay
- Remove v prefix from last active ayah number
- Update audio_state on mpv ayah advance so last-active tracks live
- Track playing surah independently of prefs for live ayah updates
- Track ayah progression via FILE_LOADED instead of END_FILE
- Correct mpv event ID constants for mpv 0.40.0
- Advance ayah counter by actual audio playback position, not mpv decoder EOF
- Ignore late END_FILE events from replaced playlist to prevent counter from jumping ahead
- Use START_FILE (not END_FILE) to advance ayah counter during gapless playback
- Stop END_FILE handler from clearing active playlist
- Poll playlist-pos instead of mpv events to advance ayah counter
- Per-file track playlist with START_FILE and self-managed EndFile
- Restore END_FILE cleanup for single-ayah playback
- Move volume indicator from Now Playing title into panel content, right-aligned
- 'c' clears results without stopping playback, matching radio
- Show edition display names in surah list header
- Add blank row between edition list and panel footer
- Hide bottom status line while fetching, keep top-right Loading indicator
- Position header segments by display width, not char count
- Move playing indicator to top-right corner, drop stale status text
- Robust stable-plugin id parsing on Windows git bash

### 💼 Other

- Track Details panel for music-preview (feature branch)

### 📚 Documentation

- Add lyrics overlay section
- Fix stale bincode doc comment on read_plugin_msg
- Fix stale quran references (plugin rename, picker overlay, edition lists)

### 🚀 Features

- Playlist-based playback, search mode, table view, UI polish
- Cursor follows auto-advance via timer-based approach
- Cursor follows auto-advance via AyahStarted mpv events
- Add Arabic and Last Active columns to surah list
- Track last active ayah per surah
- Word-wrap ayah text instead of truncating
- Add 'p' key alias to play selected station
- Hide search bar placeholder until '/' is pressed
- Hide search bar placeholder until '/' is pressed; add 'p' play alias to music-preview
- Clarify 'r' hint to 'reload stations'
- Add Track Details side panel (d key) with full iTunes track info
- Format details panel as Key: Value; human-readable release date
- Dimmed keys with normal text values in Track Details panel
- Human-friendly explicitness labels in Track Details
- Dynamic status bar hints with 'd' details; '↵' as play hint, 'p' hidden alias; dash in Position
- Fetch editions from API; translation/reciter pickers as overlay panels
- Dim header keys, color values in surah list header
- Support page up/down in edition picker panels
- Distribute surah table columns to fill available width

Full Changelog: [v0.2.35...v0.2.36](https://github.com/sonyarianto/santui/compare/v0.2.35...v0.2.36)
## [0.2.35] - 2026-07-25

### ⚙️ Miscellaneous

- Re-scrape radio stations — 17289 total (+1917 new), 196 genres enriched
- Re-scrape radio stations (10 more runs) — 17396 total
- Re-scrape radio stations (10 more runs) — 17512 total
- Re-scrape radio stations (10 more runs) — 17618 total
- Re-scrape radio stations (10 more runs) — 17692 total
- Re-scrape radio stations (10 more runs) — 17815 total
- Replace space hint with enter for play
- Use ↵ symbol for enter hint
- Mark as stable plugin

### ⚡ Performance

- Only build host + stable plugins in dev-setup, not full workspace

### 🐛 Bug Fixes

- Filter cmd_dev_json to stable-only, update PS script too
- Remove log-viewer from manifest (internal builtin, like registry-plugin)
- Always sync station DB from bundled version on startup
- Decode all numeric HTML entities (&#xxx;) in station names
- Decode &amp; and other named HTML entities in existing station names
- Handle float duration from LRCLib API
- Block scroll when lyrics fit viewport
- Cap scroll at last visible line
- Center mpv init errors and improve install hints
- Center mpv init errors and update install hints

### 💼 Other

- Split title/artist into two lines in lyrics view
- Scroll percentage indicator in lyrics view

### 📚 Documentation

- Document plugin status field and dev-setup behavior in AGENTS.md + development.md
- Sync libmpv install instructions across website and quran-reader
- Add gh release create --notes-file to release workflow
- Use --unreleased --strip header for gh release notes

### 🚀 Features

- Add Log Viewer to System category in command palette
- Continuous play, auto-advance, and centered hint
- Add 's' key to stop playback, auto-scroll on advance
- Endless loop — wrap to first track on last item
- Live countdown on now-playing Duration cell
- Auto-sync dev plugin binaries on startup
- New plugin to search lyrics from LRCLib

Full Changelog: [v0.2.34...v0.2.35](https://github.com/sonyarianto/santui/compare/v0.2.34...v0.2.35)
## [0.2.34] - 2026-07-25

### 📚 Documentation

- Update getting-started guide to reflect only stable plugins are in release

### 🚀 Features

- Add status field to manifest, only package stable plugins in release

Full Changelog: [v0.2.33...v0.2.34](https://github.com/sonyarianto/santui/compare/v0.2.33...v0.2.34)
## [0.2.33] - 2026-07-24

### ⚙️ Miscellaneous

- Skip workspace test, remove redundant cargo check

### 🐛 Bug Fixes

- Drain high-priority messages before exiting writer loop on low channel disconnect
- Upload plugin binaries sequentially to avoid GitHub secondary rate limit

Full Changelog: [v0.2.32...v0.2.33](https://github.com/sonyarianto/santui/compare/v0.2.32...v0.2.33)
## [0.2.32] - 2026-07-24

### 🐛 Bug Fixes

- Include all plugin binaries in release archive to fix santui-log-viewer startup failure

### 🚀 Features

- Add filter indicator, pgup/pgdn, remove # column

Full Changelog: [v0.2.31...v0.2.32](https://github.com/sonyarianto/santui/compare/v0.2.31...v0.2.32)
## [0.2.31] - 2026-07-18

### ⚙️ Miscellaneous

- Add generated plugins.json to gitignore

### 🎨 Refactor

- Replace Python build scripts with Rust (santui-dev-setup)
- Normalize all palette categories to 'Plugins'
- Remove redundant palette_commands() from all plugins

### 🐛 Bug Fixes

- Checkout repo before validating asset sizes

### 🚀 Features

- Add mpv playback, search mode, and dimmed search hint

Full Changelog: [v0.2.30...v0.2.31](https://github.com/sonyarianto/santui/compare/v0.2.30...v0.2.31)
## [0.2.30] - 2026-07-18

### ⚙️ Miscellaneous

- Update radio station database
- SANTUI_NO_MOUSE env, improve help order and status bar hint
- Update radio station database (30 enrichment runs)

### 🎨 Refactor

- Use time-based ticks instead of frame-based
- Move plugin hints from panel bottom border to IPC footer (92 plugins)
- Merge dns-lookup into dns-globe to remove duplicate DNS plugin
- Remove JSON fallback from read_plugin_msg

### 🐛 Bug Fixes

- Log-viewer palette_commands format + agent test instructions
- Correct palette_commands format and render cmd serialization across 37 plugins
- Ascii-table add 'l' key to load sample data
- Move hints to IPC footer, persist clipboard, let plugin consume ? key
- Remove output border box, fix trailing separator dot in status bar
- Unescape HTML entities in parse_genres, clean 61 existing genre entries
- Disable mouse capture by default (opt-in via Alt+M)
- Migrate remaining 111 plugins to binary bincode format
- Add 10s timeout to all HTTP requests
- Use Unicode width for info line right-alignment
- Move alt+m mouse off hint to left side, lowercase alt+m
- Use #[cfg(unix)] for PermissionsExt to fix Windows build

### 💼 Other

- Increase genre enrichment from 50 to 200 per run

### 📚 Documentation

- Add --no-build fast dev cycle and fix stale plugin counts in AGENTS.md
- Clarify --prune catches dead URLs without HEAD validation
- Document that native/radio_stream_stations.db must be committed

### 🚀 Features

- Runtime log capture via LoggerBuffer + log-viewer plugin
- Rate limiting, UPSERT, radio_id dedup, --db-path, --prune CLI
- Mouse capture toggle (Alt+M), --no-mouse flag, config option
- Binary bincode format for PluginMsg (plugin→host)
- Add plugin favorites (bookmark with ♥, filter with f)

Full Changelog: [v0.2.29...v0.2.30](https://github.com/sonyarianto/santui/compare/v0.2.29...v0.2.30)
## [0.2.29] - 2026-07-11

### ⚙️ Miscellaneous

- Add all 84 missing plugins to plugins-manifest.json

### 🐛 Bug Fixes

- Add 31 missing plugins to workspace members
- Use correct table height (ah-4 instead of ah-10) to eliminate 6 empty lines at bottom
- Replace meval with fasteval to remove deprecated nom v1.2.4 dependency
- Add 14 orphaned plugins to workspace members
- Remove redundant borrow in format arg to satisfy new clippy lint
- Prevent infinite retry loop from fake Metadata on LoadUrl
- Drain stale mpv events on LoadUrl, remove redundant Stop, scrap broken seq approach
- Only drain END_FILE events after LoadUrl, not FILE_LOADED from new stream
- Separate FileLoaded from Metadata events to prevent stale-stream retry poisoning
- Pre-drain ALL stale mpv events before LoadUrl, not after
- Recreate mpv handle on every station switch

### 📚 Documentation

- Fix git push command in release instructions to avoid tag event loss
- Add plugins-manifest.json note to AGENTS.md
- Sync plugin listings with 110-plugin count
- Clarify plugin addition requires both manifest + Cargo.toml members

### 🚀 Features

- Implement all 28 remaining plugin IPC binaries
- Implement 10 more plugins — AI Chat, SQLite Browser, GitHub Browser, Code Runner, Network Monitor, Time Tracker, Recipe Manager, Package Manager, Music Controller, Mermaid Renderer
- Add mysql-browser and irc-client plugins
- Add live search to plugin list
- Add mouse support to plugin host, radio player, and registry plugin
- Add optional self-hosted sync server (santui-server) and sync client
- Add exponential backoff retry for stream connection failures
- Persist volume across restarts; remove dead palette_command handler

Full Changelog: [v0.2.28...v0.2.29](https://github.com/sonyarianto/santui/compare/v0.2.28...v0.2.29)
## [0.2.28] - 2026-07-06

### ◀️ Revert

- Restore muted table header color

### ⚙️ Miscellaneous

- Remove lefthook, rely on CI and manual checks
- Use clearer 'show lyrics'/'hide lyrics' hint labels
- Replace 'Sony AK' with 'Santui contributors' in website footer
- Fix formatting, set up lefthook pre-commit hook

### 🎨 Refactor

- Dynamic field rendering in dialog + add PageUp/PageDown support
- Extract Now Playing title to constant for volume positioning

### 🐛 Bug Fixes

- Clean temp dir in TestHarness to prevent stale config from flaking enable test
- Use accent color for table headers, add spacing before hint lines
- PageUp/PageDown in lyrics now scrolls by full page, not 1 line
- Opening lyrics panel no longer steals keyboard focus
- Stations panel appeared dimmed when lyrics were hidden
- Show 'tab stations' hint on stations footer when lyrics focused, not scroll hints
- Lyrics content follows panel focus — dimmed when unfocused, bright when focused
- Align stations table header color with registry (text_muted)
- Now Playing panel should never appear dimmed
- Dim stations table text when lyrics panel has focus
- Remove redundant palette commands from home screen
- Invalidate palette cache when registry items change, preventing wrong group placement
- Add 8s HTTP timeout to lrclib and itunes lyric lookups
- Transition Connecting→Playing on first Metadata even when title is empty
- Always send Metadata on stream events and guard EndFile during Connecting, add 10s timeout
- Dynamically compute copyright year, credit Santui contributors
- Remove double spaces in home screen hint text
- Highlight key names in home screen hint with accent color
- Remove github URL from about screen copyright line

### 📚 Documentation

- Update docs to reflect lefthook pre-commit automation

### 🚀 Features

- Add rich render primitives to RenderCmd protocol
- Add Connecting state for playback lifecycle feedback
- Dim unfocused panel borders and titles when lyrics are visible
- Show scroll position percentage on stations list
- Show lyrics source attribution in lyrics panel footer
- Render logo character-by-character so starfield shows through gaps

Full Changelog: [v0.2.27...v0.2.28](https://github.com/sonyarianto/santui/compare/v0.2.27...v0.2.28)
## [0.2.27] - 2026-07-04

### ⚙️ Miscellaneous

- Add git-cliff changelog generation
- Pass --ignore flags to cargo-audit for quick-xml vulns, add audit.toml as reference

### 🎨 Refactor

- Remove duplicate hints in world-clock, add active panel indicator in radio-stream-player
- Unify focus indicator styling across world-clock and radio-stream-player
- Centralize plugin metadata into plugins-manifest.json

### 🎨 Styling

- Format unsafe blocks per rustfmt

### 🐛 Bug Fixes

- Update plugin names and descriptions in dev scripts, CI, and docs
- Right-align top bar text flush to panel border edge
- Align plugin registry table highlight and info text with panel padding
- Align radio-stream-player table padding, right-aligned text, and add red heart overlay
- Rss-reader & clipboard-history padding/unicode bugs, audit ignore for quick-xml
- Make plugin init, shutdown, and sign-in fully non-blocking
- Validate plugin binary path is within data_dir or exe_dir
- Resolve three TOCTOU/race condition bugs
- Replace THEMES[1] with named lookup and improve spawn() error diagnostics
- Replace unfulfilled expect(dead_code) with allow(dead_code)
- Remove unused test_theme and unnecessary mut in ssh-bookmark-manager
- Remove unused HttpMethod import in http-client ui tests
- Remove invalid local declarations in dev-setup.sh script body
- Suppress mpv/PipeWire stderr output in radio and IPTV plugins
- Correct YAML block scalar indentation in release workflow

### 📚 Documentation

- Add missing plugin Cargo.toml files to release checklist
- Fix config path and keybindings docs in website configuration guide
- Sync plugin lists and IPC protocol docs with new plugins
- Sync plugin lists across docs, website, README, AGENTS with 12 missing plugins

### 🚀 Features

- System-monitor plugin with 4-column dashboard + Processes panel
- World-clock plugin with timezone grid, detail view, search, and rename
- Add PluginMessage struct and plugin_message field to PluginMsg for plugin-to-plugin messaging
- Weather plugin with current conditions, hourly/daily forecast, and location search
- Clipboard-history and rss-reader plugins, fix host.rs missing plugin_message fields
- Add carousel cache, configurable keybindings, and missing plugin tests
- Add 8 new plugins + LaunchPlugin IPC support
- Add Quran reader plugin
- Add IPTV Player plugin with M3U parser and mpv video playback

Full Changelog: [v0.2.26...v0.2.27](https://github.com/sonyarianto/santui/compare/v0.2.26...v0.2.27)
## [0.2.26] - 2026-07-01

### 🎨 Refactor

- Unify data directory to platform-standard path

### 🐛 Bug Fixes

- Isolate dev and production data directories

### 📚 Documentation

- Add reset subcommand to CLI docs
- Mention radio plugin favorites in README
- Mention radio plugin favorites in website guide
- Drop keybinding detail from favorites mention

### 🚀 Features

- Add santui reset command
- Add --help flag

Full Changelog: [v0.2.25...v0.2.26](https://github.com/sonyarianto/santui/compare/v0.2.25...v0.2.26)
## [0.2.25] - 2026-07-01

### 🚀 Features

- Add filter indicator and 'c' to clear search in radio plugin

Full Changelog: [v0.2.24...v0.2.25](https://github.com/sonyarianto/santui/compare/v0.2.24...v0.2.25)
## [0.2.24] - 2026-06-30

### 🚀 Features

- Replace mtime polling with filesystem notification (notify)
- Add favorites to radio stream player with DB persistence
- Add favorites to radio stream player with DB persistence

Full Changelog: [v0.2.23...v0.2.24](https://github.com/sonyarianto/santui/compare/v0.2.23...v0.2.24)
## [0.2.23] - 2026-06-30

### ⚙️ Miscellaneous

- Remove cargo-outdated step (too heavy for every push/PR)

### 📚 Documentation

- Clarify plugins.json is gitignored, should not be committed
- Align docs, website, and templates with recent code changes

Full Changelog: [v0.2.22...v0.2.23](https://github.com/sonyarianto/santui/compare/v0.2.22...v0.2.23)
## [0.2.22] - 2026-06-29

### 🐛 Bug Fixes

- Eliminate layout shift during plugin install
- Prevent progress bar from overflowing right panel border

Full Changelog: [v0.2.21...v0.2.22](https://github.com/sonyarianto/santui/compare/v0.2.21...v0.2.22)
## [0.2.21] - 2026-06-29

### ⚙️ Miscellaneous

- Drop macOS from CI matrix (release already covers it)
- Simplify to single Linux job
- Update radio station DB (16,099 stations)

### 🐛 Bug Fixes

- Restore consumed flag in send_recv_blocking_timeout
- Remove broken cached_json from respond() — ensures consumed flag always fresh
- Event-driven Esc handling — eliminate 5ms blocking timeout race
- Add missing navigation hint in search mode status bar
- Add missing 'navigate' label to search-mode in-panel footer
- Include stations footer rows in LIST_OVERHEAD so scrolling stops at last visible station
- Lyrics scroll percentage now reaches 100% by subtracting footer and header rows from viewport height
- Separate last_metadata dedup key from song_title so Metadata guard isn't broken by TrackInfo title update
- Set lyrics_loading=true on Enter so 'Searching...' shows immediately instead of 'No lyrics found'
- Tag async lyrics/iTunes responses with metadata_seq so stale responses from previous songs are discarded

### 📚 Documentation

- Add explicit 'no push on every commit' convention to AGENTS.md
- Add architectural skepticism convention to AGENTS.md

### 🚀 Features

- Add title/artist header to lyrics panel from iTunes or station metadata

Full Changelog: [v0.2.20...v0.2.21](https://github.com/sonyarianto/santui/compare/v0.2.20...v0.2.21)
## [0.2.20] - 2026-06-29

### ⚙️ Miscellaneous

- Track plugins.json in git, remove from gitignore

### 📚 Documentation

- Add note to sync plugins.json version on release

Full Changelog: [v0.2.19...v0.2.20](https://github.com/sonyarianto/santui/compare/v0.2.19...v0.2.20)
## [0.2.19] - 2026-06-29

### ⚙️ Miscellaneous

- Update radio station database (15132 stations)
- Update native DB with 10 scraper runs (15689 stations)
- Rename Radio Streaming Player to Radio Stream Player

### ⚡ Performance

- Concurrent genre enrichment with scoped threads
- Reduce key event blocking stall from 50ms to 5ms

### 🎨 Refactor

- Replace plugin-registry magic string with persistent() trait flag
- Deduplicate RegistryConfig schema into shared core module
- Add impl Default for App, ThemeManager, and Starfield

### 🐛 Bug Fixes

- Prevent redundant mpv events from clearing lyrics on radio streams
- Dedup installed plugins by id to prevent duplicates on update
- Migrate old radio_streaming_stations.db to radio_stream_stations.db on open
- Force migrate old database even if empty new one already exists
- Add missing version field to santui-core path dependency for publishing

### 📚 Documentation

- Update architecture, plugin guide, and dev docs for capabilities system
- Fix drifts between code, website docs, and AGENTS.md
- Update plugin dev guide for modifier keys, mouse events, crash recovery, and 5ms key timeout

### 🚀 Features

- Extract radioId and enrich genres from station detail pages
- Display genres during enrichment and update native DB
- Add refresh batch for already-genred stations
- Display genre column in Stations table
- Add modifier keys (Ctrl/Shift/Alt) and function keys to IpcKey
- Add mouse event dispatch to plugins
- Add crash recovery UI with restart keybinding
- Add --version and --list-plugins flags

### 🧪 Testing

- Add 38 unit tests for IpcPluginHost in host.rs
- Add 17 unit tests for SQLite database layer
- Add tests for registry-plugin, scraper, and app binary
- Add 38 handle_key unit tests
- Add tests for lrclib, stations, database, and EventBus
- Add ConfigManager and Config unit tests
- Add 27 render_ui unit tests

Full Changelog: [v0.2.17...v0.2.19](https://github.com/sonyarianto/santui/compare/v0.2.17...v0.2.19)
## [0.2.17] - 2026-06-28

### 🐛 Bug Fixes

- Correct background plugin id check for dev mode

### 🚀 Features

- Add lyrics panel with LRCLib integration
- Add +/- volume hint to status bar
- Show volume percentage in Now Playing title
- Background playback support for radio plugin
- Declare capabilities in plugin manifests instead of hardcoded id check

Full Changelog: [v0.2.16...v0.2.17](https://github.com/sonyarianto/santui/compare/v0.2.16...v0.2.17)
## [0.2.16] - 2026-06-28

### 🐛 Bug Fixes

- Eliminate expect() panics, extract duplicated constants
- Add shell: bash to Set binary directory and Build steps for Windows compat

Full Changelog: [v0.2.15...v0.2.16](https://github.com/sonyarianto/santui/compare/v0.2.15...v0.2.16)
## [0.2.15] - 2026-06-27

### ⚙️ Miscellaneous

- Add GitHub Sponsors config and badge
- Add trailing newline to FUNDING.yml
- Update radio station database
- Update dependencies (anyhow, env_logger, uuid, wasm-bindgen)
- Add cargo-audit and cargo-outdated to CI pipeline
- Suppress cargo-audit warning for stale proc-macro-error2 dep
- Remove migration plan doc (refactor complete)

### ⚡ Performance

- Store builtin palette items as &'static str to avoid per-frame String clones
- Cache terminal height to avoid syscall on every key press

### 🎨 Refactor

- Migrate host UI + IPC bridge to ratatui native widgets
- Transparent panel backgrounds — remove background_panel fill from panels and content

### 🐛 Bug Fixes

- Replace expect() in HostMsg serialization with graceful error handling
- Add null checks before unsafe mpv event data dereference
- Replace expect() panics with graceful error handling in radio plugin
- Handle missing GitHub API id field gracefully instead of producing 'null' string
- Clear registry status message on next user key press so it doesn't linger forever
- Clear registry status on Focus so stale message doesn't persist across palette open/close
- Auto-dismiss registry status after ~2s (120 ticks) + clear on key press + set_status helper
- Reset detail_idx on blur so stale action dialog doesn't reappear; adjust auto-dismiss threshold for 100ms tick rate
- Derive plugin manifest version from crates/core/Cargo.toml in dev-setup scripts
- Sync plugin SDK template and docs with current IPC protocol
- Esc in plugin sub-dialog now closes dialog instead of exiting plugin
- Block for key response so consumed flag is not stale
- Cap theme picker popup height to 20 lines so it doesn't fill the terminal
- Resolve four fragility issues in IPC and plugin handling
- Remove redundant esc hint from theme picker footer
- Add footer, blank lines, and remove leading spaces in plugin actions dialog
- Registry plugin hints lowercase ↵, remove ctrl+p commands from status bar line for active plugins

### 💼 Other

- Simplify palette footer to ↑↓ navigate • ↵ select

### 📚 Documentation

- Add santui_1.png screenshot to README and website
- Update stale PaletteWidget references to PaletteController
- Remove stale audit/roadmap/ideas docs, migrate to GitHub Issues
- Sync docs with current codebase (release process, project structure, broken link)
- Add IPC consumed protocol convention to AGENTS.md
- Fix PluginMsg field listing, timeout values, and template to include consumed

### 🚀 Features

- Add Intel Mac (x86_64) support
- Show full country names in radio plugin; add publisher column to registry
- Native ratatui full-box panels with integrated title via RenderCmd::Border
- Add title_dash_fg to RenderCmd::Border for inline border-colored dashes
- Add Exit to palette menu; show plugin count when status is empty
- Move Plugin registry from palette_command to builtin; reorder System items
- Move palette key hints into palette footer instead of status bar
- Add padding around palette footer hint
- Reorder palette categories to Plugins, Auth, System
- Add theme picker footer with key hints, remove from status bar
- UI polish, dynamic Now Playing height, current-station highlight
- Total stations count at top right, auto-dismiss scan message
- Move search bar to top line, no table row loss, Enter plays

Full Changelog: [v0.2.14...v0.2.15](https://github.com/sonyarianto/santui/compare/v0.2.14...v0.2.15)
## [0.2.14] - 2026-06-25

### ⚙️ Miscellaneous

- Normalize plugins.json key ordering
- Update radio plugin binary hash and size
- Remove unused fix-yaml-temp.js
- Update radio stations DB (14,430 stations)
- Update radio stations DB (14,519 stations)
- Update radio stations DB (14,731 stations)

### ⚡ Performance

- Reuse PluginContext across frames instead of re-constructing
- Cache theme_manager.filtered() Vec<usize> allocation
- Persist DB connection in App struct instead of reopening on every reload
- Avoid cloning cached_commands by returning &[RenderCmd] and serializing via json!()
- Cache JSON string in respond() to skip serialization on idle frames
- Cache palette grouping in PaletteController, rebuild only on query change
- Avoid hint_key.clone() and format!() allocs in StatusBar plugin-hints path
- Reduce theme picker allocs — pre-size list_lines, avoid double format!() per row
- Avoid padding allocs in truncate/station-list; remove dead Event::UserUpdated

### 🎨 Refactor

- Split main.rs into app/ submodules
- Extract FlowCtx + wait_for_pending to reduce duplication
- High-level render protocol, remove core->registry dep, simplify registry plugin UI
- Move serde and serde_json to workspace dependencies

### 🐛 Bug Fixes

- Terminal panic hook — TerminalGuard restores raw mode on crash
- Exclude registry-plugin from plugins.json
- Do not dim status bar when palette is active
- Handle Ctrl+C and SIGINT/SIGTERM for clean terminal shutdown
- Prevent orphan plugin process when kill() fails
- Plugin crash watchdog — detect silent crashes and show in status bar
- Replace 9 unwrap() calls on mpv symbol lookup with proper error handling
- Stability (unwrap/expect/env_logger) + perf (Config clone, palette allocs)
- Radio plugin not launching from palette
- Only mark config dirty on successful poll parse, not on parse failure
- Increase plugin shutdown grace period from 1s to 3s to prevent force-kill mid-write
- Compute palette groups in render() too, not just handle_key() — fixes empty palette on first open
- Plugin accumulation, q-during-plugin UX, and stability unwrap audit
- Dev-mode install uses file copy instead of HTTP; remove misleading 'q' hint from plugin status bar
- Remove volume panel, improve Now Playing spacing, keep registry plugin on Esc

### 💼 Other

- Log warnings for invalid config values

### 📚 Documentation

- Sync audit items — move 4 resolved issues to history, fix 2 descriptions
- Move 5 resolved audit items to history (Mpv Drop, EventBus drain, config/plugin throttling, splash cache)

### 🚀 Features

- Non-blocking send via background writer thread with priority channels
- Station list as table with Name/Country columns; ignore auto-generated plugins.json

### 🧪 Testing

- Add comprehensive unit tests

Full Changelog: [v0.2.13...v0.2.14](https://github.com/sonyarianto/santui/compare/v0.2.13...v0.2.14)
## [0.2.11] - 2026-06-21

### 🚀 Features

- Plugin registry refactor + IPC protocol updates
- Home screen plugin carousel with left/right navigation

Full Changelog: [v0.2.10...v0.2.11](https://github.com/sonyarianto/santui/compare/v0.2.10...v0.2.11)
## [0.2.10] - 2026-06-21

### ⚙️ Miscellaneous

- Add ideas doc

Full Changelog: [v0.2.9...v0.2.10](https://github.com/sonyarianto/santui/compare/v0.2.9...v0.2.10)
## [0.2.8] - 2026-06-21

### ⚙️ Miscellaneous

- Plugin:Send supertrait + god object Phase 1&2
- God object Phase 3 — RegistryController extraction
- EventBus decoupling for theme changes
- Move plugin_factory into PluginManager
- Move tick_rate into ConfigManager
- Update audit — god object resolved
- Split audit into active (audit.md) + history (audit-history.md)
- Wire santui-db into binary; merge audit findings
- Update radio stations DB (14,011 stations)

### 🎨 Refactor

- Split handle_key 337-line monolith into per-state handlers ([#2](https://github.com/sonyarianto/santui/issues/2))
- Derive manifest filename from std::env::consts instead of cfg macros

### 🎨 Styling

- Lowercase provider label in status bar

### 🐛 Bug Fixes

- Non-blocking GitHub device flow with embedded client ID
- Join radio plugin mpv thread on shutdown
- Save registry config before binary download to avoid zombie plugins
- Propagate plugin spawn failure instead of registering dead plugin
- Remove Box::leak in mpv FFI, add safety docs for Send+Sync
- Make IPC send_recv non-blocking — never wait for plugin response
- Make Google OAuth non-blocking, add structured logging
- OAuth redirect port fallback (9842..9850, then OS-assigned)
- Populate Plugins from installed registry at startup
- Add santui-db to publish-crates workflow

### 📚 Documentation

- Update copyright URL and tagline text
- Fix CI badge to point to ci.yml
- Bump website version to v0.2.7, fix tagline
- Add website version update to release checklist
- Update audit.md — GitHub OAuth no longer blocks
- Add safety comment for Cell<Area> interior mutability
- Mark handle_key audit item as fixed

### 🚀 Features

- Show provider prefix (GitHub:/Google:) in status bar
- Add santui-db crate — central SQLite for per-user data
- Configurable tick rate via Santui::set_tick_rate()
- Adaptive star count based on terminal dimensions
- EventBus multi-consumer via subscribe() observer pattern
- User-defined themes from ~/.santui/themes/*.toml

Full Changelog: [v0.2.7...v0.2.8](https://github.com/sonyarianto/santui/compare/v0.2.7...v0.2.8)
## [0.2.7] - 2026-06-21

### ◀️ Revert

- Remove welcome message, back to minimal download log
- Bring back the welcome message 😄

### ⚙️ Miscellaneous

- Fix GitHub URLs to sonyarianto/santui
- Add GitHub Actions workflow with check, clippy, fmt, test
- Remove GitHub URL from About, keep only santui.vercel.app
- Ignore native/mpv-1.dll (too large for GitHub)
- Add .gitattributes for consistent line endings
- Update tagline to 'my terminal home base'
- Reduce stars to 88
- Switch release hosting to GitHub Releases
- Add GitHub Actions build and release workflow
- Bump actions to latest versions (checkout v6, upload v7, download v8, gh-release v3)
- Add brew install mpv step for macOS build
- Silence Homebrew tap trust warning
- Update all workspace dependencies to latest
- Update radio station DB to 12,004 stations
- Update radio station DB to 12,228 stations
- Update radio station DB to 12,799 stations
- Release v0.1.3 — fix libmpv loading on macOS
- Use dylibbundler instead of manual bundle_deps for macOS release
- Multi-platform matrix, rust-cache, and Windows mpv error handling
- Fix version check step — only run on ubuntu to avoid PowerShell/bash issue
- Remove radio player and native deps from main release archive
- Remove dead mpv deps from release workflow
- Make Sony AK footer name link to GitHub profile
- Remove scoop-santui from project, add to gitignore
- Auto-update Scoop bucket manifest on release
- Push scoop bucket to main branch
- Remove all Scoop references — release.yml, README, website
- Clear winget manifests, keep directory with .gitkeep
- Add packages/scoop and packages/chocolatey with .gitkeep
- Add welcome message when binary is first downloaded
- Wire config system in main.rs
- Remove accidentally committed root node_modules
- Remove redundant check from lefthook pre-commit
- Add GitHub Actions release workflow + Linux packaging script
- Restore original release workflow with npm publish added
- Add cargo publish to release workflow
- Update dependencies — ratatui 0.30.2, syn 2.0.118, unicode-width 0.2.2, and others
- Fix publish order for santui (not santui-app)

### 🎨 Refactor

- Extract dim overlay, share dialog constants, remove redundant clone
- Migrate stations to SQLite database + embedded JSON
- Move all crates into crates/ directory
- Move santui-npm → packages/npm, winget-manifests → packages/winget
- Move plugin template to templates/plugin/

### 🎨 Styling

- Apply clippy fixes and fmt formatting
- Match local redirect response style with Tailwind
- Make card border radius more subtle (rounded-lg)

### 🐛 Bug Fixes

- Cursor overlays placeholder first char instead of inserting before it
- Solid cursor block (gold bg) on placeholder first char and at end of query
- Category accent color #9d7cd8 (OpenCode purple)
- Increase About layout height to fit both URLs
- Adapt mpv FFI to custom DLL v0.41.0 constants and struct layout
- Correct Vercel URL to santuiapp.vercel.app
- Correct GitHub repo and handle missing release gracefully
- Use GetTempPath() to avoid TEMP short-path issues
- Replace Unicode chars with ASCII in install script
- Align install scripts with CI outputs
- Release workflow — plugin binary naming and asset size validation
- Plugins.json Windows — correct binary path with santui- prefix
- Auto-unblock downloaded binaries in install.ps1
- Double-unblock in install.ps1 — ZIP before extract + files after
- Replace verify launch with file size check in install.ps1
- Merge download into index.js, no postinstall needed
- Correct registry repo from sony-ak to sonyarianto
- Join background reader thread on plugin kill and drop
- Add 120s read timeout to redirect handler; fix Vercel Content-Type
- Skip npm version bump if already matches tag
- Add version to all santui- path dependencies for crates.io publishing

### 💼 Other

- Nest scraper as sub-crate inside radio player
- Add real-time search/filter mode for stations; scraper: remove 404 country codes
- Concurrent fetching, URL cleanup, remove dead country codes
- Remove onlineradiobox.com from display output
- Also strip ?ref=onlineradiobox26 from URLs, update cleanup SQL
- Update tagline to 'My terminal home base'
- Simplify hero — tagline only
- Dark mode by default
- Tighten hero spacing
- Use Santui gold (#FFB900) as brand color
- Update footer copyright
- Add copyright symbol to footer
- Fix macOS/Linux install description

### 📚 Documentation

- Add AGENTS.md for AI agent context
- Add IPC plugin architecture diagram to AGENTS.md
- Update website tagline and add cargo watch command
- Rewrite website and add README to reflect Santui as a TUI app, not a framework
- Add release process to docs/release.md
- Update getting started for release workflow
- Update what-is-santui page
- Add macOS and Linux install instructions
- Remove Rust requirement from getting started
- Simplify macOS install instructions
- Remove 'built on Ratatui' from tagline
- Document default plugin & feature flag in architecture.md
- Add uninstall section to getting started guide
- Align README and website with plugin registry architecture
- Add dev mode section to website getting started guide
- Add Scoop installation method to README and website
- Make Scoop primary Windows install method, add PowerShell fallback warning
- Make Scoop primary install on landing page, irm as fallback
- Add npm install for macOS/Linux, uninstall tables, Node.js prerequisite
- Change '50,000+' to 'thousands of' — more realistic
- Add 'type santui to launch' note after npm install
- Add architecture roadmap
- Add target architecture section from roadmap
- Add configuration reference page for config.toml
- Add plugin development guide to website
- Replace Internet Radio Player feature card with OAuth Sign-in
- Move libmpv prereq from landing page to plugin guide
- Add release instructions to AGENTS.md
- Add MIT license badge to README
- Add CI and npm badges to README
- Add path-dependency version requirement to release instructions

### 🚀 Features

- Polish command palette to match OpenCode UX
- Add VitePress website with home page and guide
- Set canonical URL to https://santui.vercel.app, add vercel.json
- Add GitHub and website URLs to About screen
- Add theme system with Theme struct, palette-based theme picker, and Nord theme
- Add all 37 OpenCode themes with scrollable theme picker
- Add Santui theme (gold) as default, 38 themes total
- Live theme preview with flicker-free plugin handling
- Load stations from JSON, add iTunes enrichment
- Plugin-contributed status bar hints
- Add santui-radio-streaming-scraper and reload feature
- Wrap arrow navigation in palette and theme picker
- Add santui-auth crate with OAuth (Google/GitHub) sign-in
- Add shooting stars starfield to splash screen
- Realistic shooting stars and comet variant
- Bundled DB shipping, genre support, stars on all screens
- Add curl-based install script for macOS/Linux
- Bundle libmpv in native/ for all platforms, update CI and scripts
- Dim background when palette or theme picker is open
- Add uninstall scripts for Windows (ps1) and macOS/Linux (sh)
- Better overlay with dim_color (scale brightness, preserve contrast for explicit bg colors)
- Plugin registry with dynamic palette and PluginFactory
- Dev mode automation and docs alignment
- Add v0.1.8 version badge to website nav and footer
- Auto-add Windows Defender exclusion in install.ps1
- Add npm package for santui binary distribution
- Phase 1.1 — dynamic command registry for plugins
- Phase 1.2 — simplify Plugin trait with default impls
- Phase 1.3 — extract StatusBar widget
- Phase 2.1 — extract PluginManager from Santui
- Phase 2.2 — EventBus for decoupled event communication
- Phase 3.1 — async IPC with background reader thread
- Phase 3.2 — timeout mechanism for send_recv
- Phase 4.1 — extract ThemeManager from Santui
- Phase 4.2 — extract PaletteWidget from Santui
- Phase 4.3 — extract RegistryScreen from Santui
- Phase 5.1 — config system with hot-reload
- Phase 5.2 — save theme to config on selection
- Phase 5.3 — save custom theme colors infrastructure
- Phase 6.1 — integration tests for Santui framework
- Phase 2.3 — centralized AppState, dynamic built-in commands, full ThemeData IPC
- Phase 3.2 — plugin hot-reload via binary mtime polling
- Phase 3.3 — Plugin SDK / cargo generate template
- Graceful plugin shutdown via Plugin::shutdown()
- Google sign-in via Vercel token exchange instead of embedded credentials
- Style sign-in page with Tailwind CSS

### 🧪 Testing

- Add unit test suites for core, ipc, and radio crates
- Add edge case tests for dim_color and parse_hex helpers
- Add proptest property-based tests for dim_color
