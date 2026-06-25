# Architecture Audit

Generated 2026-06-21 from comprehensive codebase review covering architecture, performance, and production-readiness.

Resolved items moved to [audit-history.md](audit-history.md).

## Critical — crash, terminal corruption, or shipped dead code

## High — design problems, resource leaks, unenforced quality

- [ ] **No `deny.toml` or clippy lint config** — no automated enforcement of `unsafe` usage, duplicate dependencies, or advisory checks.

## Medium — latent bugs & performance issues

- [ ] **`status_hints()` allocates per frame** — returns owned `Vec<(String,String)>` cloned from cache every render tick (~8+ string allocs). Change return to `&[(String,String)]`. (`crates/ipc/src/host.rs:408-410`)

- [ ] **JSON serialized on every IPC send** — `serde_json::to_string` called per frame (`Tick`) and per keystroke (`Key`). Pre-compute fixed-shape messages; batch when idle. (`crates/ipc/src/host.rs:126`)

- [ ] **IPC round-trip every tick** — `send(Tick)` + `drain_responses()` every frame even when plugin has no new data. Skip send when plugin is idle; use dirty flag. (`crates/ipc/src/host.rs:350-351`)

- [ ] **Radio plugin clones `cached_commands` on every render** — entire `Vec<RenderCmd>` heap-cloned, then immediately serialized. Return reference or `mem::take`. (`crates/plugins/radio-streaming-player/src/main.rs:399`)

- [ ] **StatusBar `Vec<Span>` built from scratch per frame** — 8-14 string allocations every render tick from hint keys/descriptions. Memoize or pre-compute. (`crates/core/src/app/status_bar.rs:62,109`)

- [ ] **Palette `filtered_items()` + grouping rebuilt every frame** — 50+ small heap allocations when palette is open. Memoize; only recompute on query change. (`crates/core/src/app/palette_widget.rs:118-143`)

- [ ] **Radio `respond()` serializes unconditionally** — full `PluginMsg` JSON on every Tick/Key/Focus/etc. Cache JSON string; only re-serialize when dirty. (`crates/plugins/radio-streaming-player/src/main.rs:417`)



- [ ] **Radio `" ".repeat(fill_w)` per row** — string allocation for each row in panel draw (~40 per frame). Use `Paragraph` with background style. (`crates/plugins/radio-streaming-player/src/ui.rs:30`)

- [ ] **Radio `text_at()` string allocation per text element** — 15-30 small String allocs per radio frame. (`crates/plugins/radio-streaming-player/src/ui.rs:57-63`)

- [ ] **Theme picker `list_lines` + `header_lines` rebuilt per frame** — ~40-80 allocs when picker is open. Memoize. (`crates/core/src/app/theme_manager.rs:206,221`)

- [ ] **Radio DB connection opened per `load()`** — `database::open()` re-creates SQLite connection + runs migrations on every reload. Keep connection alive. (`crates/plugins/radio-streaming-player/src/database.rs:37-71`)



- [ ] **Plugin shutdown timeout too short (1s)** — may force-kill plugin mid-write. Increase grace period. (`crates/ipc/src/host.rs:145-149`)

- [ ] **`Event::UserUpdated` handler is a no-op** — event IS emitted on sign-out/sign-in, but the handler in `process_events()` (`app_state.rs:45`) does nothing because the actual notification is done via a direct call to `on_user_update_all()`. Dead code in event dispatch.

- [ ] **`poll()` sets `dirty=true` even on config parse failure** — stale config re-applied, wasting a render frame. (`crates/core/src/config.rs:130-139`)

## Low — polish

- [ ] **No security/capability model between plugins** — IPC plugins have full system access (filesystem, network, etc.). No sandboxing or permission system. Low priority because plugins are opt-in and typically trusted.

- [ ] **30+ `unwrap()` in production paths** — audit each for provable safety; replace with `?` or `expect("message")` where not provable. (various)

- [ ] **Raw pointer casts in radio player `unsafe` blocks** — `Mpv` FFI uses raw function pointers cast from `libloading::Symbol`. Add safety justification comments. (`crates/plugins/radio-streaming-player/src/player.rs`)

- [ ] **`to_string_lossy()` without fallback** — replaces invalid UTF-8 silently. Consider logging the path on error. (various)

- [ ] **Ratatui deprecated API usage** — some methods called are deprecated in recent ratatui versions. (various)

- [ ] **Unused imports** — clean up across workspace. (various)

- [ ] **`#[allow(dead_code)]` on several items** — `database.rs:search()`, `itunes.rs:collection_name`, `auth.rs:verification_uri`. Remove or use. (various)

- [ ] **`let _ =` swallows errors silently** — widespread; at minimum log at `warn!` level when a fallible operation fails. (various)

- [ ] **EventBus subscriber scan O(n) on emit** — no current subscribers, but worth noting for future. (`crates/core/src/event.rs:64-66`)
