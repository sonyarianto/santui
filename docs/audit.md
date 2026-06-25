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



- [x] **Radio `" ".repeat(fill_w)` per row** — now uses `RenderCmd::Rect` (one alloc per panel, not per row); 3 allocs per frame. Host-side `render.rs` still allocates once per `Rect`/`Clear` via `" ".repeat(cw)`. (`crates/ipc/src/render.rs:140`)

- [x] **Radio `text_at()` string allocation per text element** — `truncate()` no longer pads when text fits; station list reuses the `text` string directly instead of re-formatting. (`crates/ipc/src/ui.rs:174`, `crates/plugins/radio-streaming-player/src/ui.rs:71`)

- [x] **Radio DB connection opened per `load()`** — persistent `db: Connection` in `App` struct, initialized once in `new()`. (`crates/plugins/radio-streaming-player/src/main.rs:54`)



- [x] **`Event::UserUpdated` handler is a no-op** — removed `UserUpdated` variant; sign-out/sign-in calls `on_user_update_all()` directly. (`crates/core/src/event.rs`, `crates/core/src/app/mod.rs:701`, `crates/core/src/app/handle_key.rs:62`, `crates/core/src/app/plugin_manager.rs:292`)

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
