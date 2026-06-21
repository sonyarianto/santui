# Architecture Audit

Generated 2026-06-21 from comprehensive codebase review covering architecture, performance, and production-readiness.

Resolved items moved to [audit-history.md](audit-history.md).

## Critical — crash, terminal corruption, or shipped dead code

- [ ] **No panic hook** — terminal left in raw mode on panic; user must manually `reset`. Install a panic hook at the top of `main()` that calls `disable_raw_mode()` + `LeaveAlternateScreen`. (`crates/core/src/app/mod.rs:629-702`)

- [ ] **No Ctrl+C / SIGINT handler** — the only exit path is `'q'`. Ctrl+C in raw mode falls through unhandled. On Unix, `kill` terminates without cleanup. (`crates/core/src/app/handle_key.rs:209-234`)

- [ ] **`core` depends on `registry` crate** — violates the layered architecture; core must not know about plugin registry. Invert the dependency or extract an interface. (`crates/core/Cargo.toml`)

## High — design problems, resource leaks, unenforced quality

- [ ] **Plugin crash silently tolerated** — no watchdog, no auto-restart, no user-visible error. `send()` and `drain_responses()` silently return when the child process has exited. (`crates/ipc/src/host.rs:117-141`)

- [ ] **Plugin child process leaks on failed `kill()`** — if `child.kill()` fails, the orphan continues running; a new instance is spawned alongside it. (`crates/ipc/src/host.rs:153-161`)

- [ ] **`Mpv::drop` doesn't call `mpv_destroy`** — if dropped before `MpvCmd::Quit`, mpv internals and network streams leak. (`crates/plugins/radio-streaming-player/src/player.rs:62-66`)

- [ ] **No `deny.toml` or clippy lint config** — no automated enforcement of `unsafe` usage, duplicate dependencies, or advisory checks.

- [x] **Scraper crate version drift** — `0.2.5` vs workspace `0.2.7`. (`crates/plugins/radio-streaming-player/scraper/Cargo.toml:3`) — fixed in v0.2.9

- [ ] **`serde` duplicated across workspace** — declared both as path dep and workspace dep in multiple crates.

## Medium — latent bugs & performance issues

- [ ] **`status_hints()` allocates per frame** — returns owned `Vec<(String,String)>` cloned from cache every render tick (~8+ string allocs). Change return to `&[(String,String)]`. (`crates/ipc/src/host.rs:408-410`)

- [ ] **JSON serialized on every IPC send** — `serde_json::to_string` called per frame (`Tick`) and per keystroke (`Key`). Pre-compute fixed-shape messages; batch when idle. (`crates/ipc/src/host.rs:126`)

- [ ] **`EventBus::drain()` allocates every frame** — `self.pending.drain(..).collect()` creates a `Vec<Event>` allocation even when empty. Use `mem::take` instead. (`crates/core/src/event.rs:74-76`)

- [ ] **IPC round-trip every tick** — `send(Tick)` + `drain_responses()` every frame even when plugin has no new data. Skip send when plugin is idle; use dirty flag. (`crates/ipc/src/host.rs:350-351`)

- [ ] **Radio plugin clones `cached_commands` on every render** — entire `Vec<RenderCmd>` heap-cloned, then immediately serialized. Return reference or `mem::take`. (`crates/plugins/radio-streaming-player/src/main.rs:399`)

- [ ] **StatusBar `Vec<Span>` built from scratch per frame** — 8-14 string allocations every render tick from hint keys/descriptions. Memoize or pre-compute. (`crates/core/src/app/status_bar.rs:62,109`)

- [ ] **Palette `filtered_items()` + grouping rebuilt every frame** — 50+ small heap allocations when palette is open. Memoize; only recompute on query change. (`crates/core/src/app/palette_widget.rs:118-143`)

- [ ] **`PluginContext` constructed twice per frame** — both `Theme` and `Arc` cloned each time. Hold references instead of owned values. (`crates/core/src/app/mod.rs:642,658`)

- [ ] **Config file metadata syscall every frame** — `config_manager.poll()` calls `path.metadata()` on every tick. Throttle to every 30-60 frames. (`crates/core/src/config.rs:118`)

- [ ] **Plugin binary stat per frame** — `check_reloads()` stats every plugin binary on every tick. Throttle to every 30-60 frames. (`crates/core/src/app/plugin_manager.rs:159-165`)

- [ ] **Splash/about screen logo rebuilt every frame** — `Vec<Line>` re-allocated each render. Pre-build at startup. (`crates/core/src/app/screens.rs:73-83`)

- [ ] **Radio `respond()` serializes unconditionally** — full `PluginMsg` JSON on every Tick/Key/Focus/etc. Cache JSON string; only re-serialize when dirty. (`crates/plugins/radio-streaming-player/src/main.rs:417`)

- [ ] **`Config.clone()` in `apply_config()`** — clones entire `Config` with up to 14 heap-allocated `Option<String>` fields. Use by-reference. (`crates/core/src/app/mod.rs:536`)

- [ ] **`theme_manager.filtered()` allocates `Vec<usize>` per frame** — 37 elements rebuilt each tick when picker is open. Memoize. (`crates/core/src/app/theme_manager.rs:86-97`)

- [ ] **Unnecessary `PluginCmdItem` clone on palette exec** — 2 String fields cloned then immediately discarded. (`crates/core/src/app/handle_key.rs:79`)

- [ ] **Dynamic item triple-String clone** — `(String, String, String)` cloned on activation when only `id` and `name` needed. (`crates/core/src/app/handle_key.rs:87`)

- [ ] **`filtered_items()` called on arrow key presses in palette** — unnecessary; only needed when query text changes. (`crates/core/src/app/palette_controller.rs:45-49`)

- [ ] **Radio `" ".repeat(fill_w)` per row** — string allocation for each row in panel draw (~40 per frame). Use `Paragraph` with background style. (`crates/plugins/radio-streaming-player/src/ui.rs:30`)

- [ ] **Radio `text_at()` string allocation per text element** — 15-30 small String allocs per radio frame. (`crates/plugins/radio-streaming-player/src/ui.rs:57-63`)

- [ ] **Theme picker `list_lines` + `header_lines` rebuilt per frame** — ~40-80 allocs when picker is open. Memoize. (`crates/core/src/app/theme_manager.rs:206,221`)

- [ ] **Radio DB connection opened per `load()`** — `database::open()` re-creates SQLite connection + runs migrations on every reload. Keep connection alive. (`crates/plugins/radio-streaming-player/src/database.rs:37-71`)

- [ ] **`expect()` in scraper SQL transactions** — panics on broken connection mid-scrape. Use `?` instead. (`crates/plugins/radio-streaming-player/scraper/src/main.rs:434,501`)

- [ ] **`unwrap()` on mpv symbol lookup** — 9 `lib.get()` unwraps can panic if libmpv version doesn't match. Return `Err` instead. (`crates/plugins/radio-streaming-player/src/player.rs:130-138`)

- [ ] **`unwrap()` on `tx_msg` before `handle_init`** — panics if plugin ticked before Init. (`crates/plugins/radio-streaming-player/src/main.rs:322`)

- [ ] **Config invalid field values silently ignored** — invalid hex colours, unknown theme names accepted without feedback. Log warnings. (`crates/core/src/app/mod.rs:555-615`)

- [ ] **Plugin shutdown timeout too short (1s)** — may force-kill plugin mid-write. Increase grace period. (`crates/ipc/src/host.rs:145-149`)

- [ ] **`Event::UserUpdated` processed but never emitted** — dead code in event dispatch. (`crates/core/src/app/app_state.rs:45`)

- [ ] **`poll()` sets `dirty=true` even on config parse failure** — stale config re-applied, wasting a render frame. (`crates/core/src/config.rs:130-139`)

## Low — polish

- [ ] **No security/capability model between plugins** — IPC plugins have full system access (filesystem, network, etc.). No sandboxing or permission system. Low priority because plugins are opt-in and typically trusted.

- [ ] **30+ `unwrap()` in production paths** — audit each for provable safety; replace with `?` or `expect("message")` where not provable. (various)

- [ ] **Raw pointer casts in radio player `unsafe` blocks** — `Mpv` FFI uses raw function pointers cast from `libloading::Symbol`. Add safety justification comments. (`crates/plugins/radio-streaming-player/src/player.rs`)

- [ ] **`dbg!()` calls remain in production code** — remove before release. (various)

- [ ] **`to_string_lossy()` without fallback** — replaces invalid UTF-8 silently. Consider logging the path on error. (various)

- [ ] **Ratatui deprecated API usage** — some methods called are deprecated in recent ratatui versions. (various)

- [ ] **Unused imports** — clean up across workspace. (various)

- [ ] **`#[allow(dead_code)]` on several items** — `database.rs:search()`, `itunes.rs:collection_name`, `auth.rs:verification_uri`. Remove or use. (various)

- [ ] **`let _ =` swallows errors silently** — widespread; at minimum log at `warn!` level when a fallible operation fails. (various)

- [ ] **`env_logger::init()` panics if called twice** — guard with `try_init()` or `OnceCell`. (`crates/app/src/main.rs:8-11`)

- [ ] **`query.to_lowercase()` re-allocated per `filtered_items()` call** — called 2-3× per frame when palette open. Cache lowered query. (`crates/core/src/app/palette_widget.rs:40`)

- [ ] **EventBus subscriber scan O(n) on emit** — no current subscribers, but worth noting for future. (`crates/core/src/event.rs:64-66`)

- [ ] **Formatting inconsistencies** — trailing whitespace in `radio-player/src/main.rs:93`, `scraper/src/main.rs:454`. Run `cargo fmt`. (various)
