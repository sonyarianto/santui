# Audit History — Resolved Items

Items from the [architecture audit](audit.md) that have been fixed.

## Fixed

- [x] **Thread leak on plugin hot-reload** (`crates/ipc/src/host.rs`) — background reader thread handle was discarded; now joined in `kill()` and `Drop`

- [x] **No graceful plugin shutdown** (`crates/core/src/plugin.rs`, `crates/ipc/src/host.rs`) — `shutdown()` added to `Plugin` trait; `IpcPluginHost` sends `Shutdown` + waits 1s before kill; called on hot-reload

- [x] **No panic hook** (`crates/core/src/app/mod.rs`) — added `TerminalGuard` Drop guard inside `run()` that calls `disable_raw_mode()` + `LeaveAlternateScreen` + `Show` on any exit path (normal, error, or panic)

- [x] **No Ctrl+C / SIGINT handler** (`crates/core/src/app/handle_key.rs`, `crates/core/src/app/mod.rs`) — two-layer fix: (1) keyboard `Ctrl+C` caught as a raw-mode key event at the top of `handle_key()`, (2) OS-level signal handler via `ctrlc` crate (with `termination` feature covering SIGTERM/SIGHUP) sets an `AtomicBool` flag checked each frame

## Critical — crash or corrupt state

- [x] **Mutex poisoning in auth** (`crates/auth/src/lib.rs`) — replaced `.lock().unwrap()` with `.lock().unwrap_or_else(|e| e.into_inner())` on all 4 call sites to recover from poisoning

## High — design problems

- [x] **Santui is a god object** (`crates/core/src/app/mod.rs:514`) — 13+ fields → 9 fields; extracted `PaletteController` (owns `Option<FilteredListState>` + key handling), `RegistryController` (owns `Option<PluginRegistry>` + `RegistryScreen` + key handling); moved `dynamic_items` + `plugin_factory` into `PluginManager`; moved `tick_rate` into `ConfigManager`; EventBus decoupling for theme changes (`select_theme`/`apply_config` no longer reach across subsystems)

- [x] **`handle_key` is a 337-line monolith** (`crates/core/src/app/handle_key.rs:1`) — split into 6 focused per-state handlers (`handle_key_palette`, `execute_palette_selection`, `handle_key_theme_picker`, `handle_key_about`, `handle_key_registry`, `handle_key_normal`)

- [x] **Config parse failures are silent** (`crates/core/src/config.rs`, `crates/core/src/app/status_bar.rs`) — added `try_load_from` returning `Result`, `ConfigManager.error` field, and `StatusBar.config_error` renders the error in red on the right side of the status bar

- [x] **Plugin spawn failure returns `Ok(())`** (`crates/ipc/src/host.rs:236`) — `spawn()` now returns `Result`; `init()` propagates the error; dead plugins are never registered

- [x] **Unbounded EventBus** (`crates/core/src/event.rs`) — switched internal storage to `VecDeque` with a 1024 cap; oldest event dropped when at capacity

- [x] **`core` depends on `registry` crate** (`crates/core/Cargo.toml`) — `santui-registry` dependency removed; types inlined locally in commit `0d6b5d6`

- [x] **Scraper crate version drift** (`crates/plugins/radio-streaming-player/scraper/Cargo.toml`) — version synced to workspace `0.2.9`; resolved in v0.2.9

## Medium — latent bugs & resource issues

- [x] **IPC blocks main thread up to 5 seconds** (`crates/ipc/src/host.rs:130`) — `send_recv` now non-blocking; all calls use `send` + `drain_responses`; the UI never waits for a plugin response

- [x] **GitHub OAuth blocks main thread** (`crates/auth/src/lib.rs:235`) — GitHub device flow now runs on background thread; TUI stays responsive; code shown in status bar

- [x] **Google OAuth blocks main thread** (`crates/auth/src/lib.rs:240`) — Google redirect flow now runs on background thread via `start_sign_in("google")`; TUI stays responsive; status bar shows "Google: waiting for browser…"

- [x] **Registry file-write crash loses state** (`crates/registry/src/lib.rs:141`) — config saved *before* binary download; push to installed list first, save, then write binary. On error, entry is rolled back

- [x] **`Box::leak` in mpv FFI** (`crates/plugins/radio-streaming-player/src/player.rs:123`) — replaced `Box::leak` with `Box::new`; function table is now dropped when `Mpv` is dropped

- [x] **Unsafe Send+Sync impls without safety docs** (`crates/plugins/radio-streaming-player/src/player.rs:68-69`) — added safety justification comment for `unsafe impl Send/Sync` on `Mpv`

- [x] **Cell<Area> interior mutability** (`crates/ipc/src/host.rs:35`) — added safety doc explaining `Cell` is safe because `IpcPluginHost` is `!Sync`, never shared across threads

- [x] **`handle_key` calls blocking GitHub OAuth on main thread** (`crates/core/src/app/handle_key.rs:62`) — switched to non-blocking `start_sign_in()` for GitHub; Google sign-in still blocks but is faster (redirect-based)

- [x] **Radio plugin thread leak** (`crates/plugins/radio-streaming-player/src/main.rs:103`) — mpv event thread handle now stored; `MpvCmd::Quit` sent on shutdown, thread joined

- [x] **`Mpv::drop` doesn't call `mpv_destroy`** (`crates/plugins/radio-streaming-player/src/player.rs:62-66`) — added `impl Drop for Mpv` that calls `destroy()`; `destroy()` changed to `&mut self` and nulls the handle so double-destroy is safe

- [x] **`EventBus::drain()` allocates every frame** (`crates/core/src/event.rs:74-76`) — replaced `drain(..).collect()` with `std::mem::take`, zero allocation when the queue is empty

- [x] **Config file metadata syscall every frame** (`crates/core/src/config.rs:118`) — throttled to every 30 frames via `poll_skip` counter

- [x] **Plugin binary stat per frame** (`crates/core/src/app/plugin_manager.rs:159-165`) — throttled `check_reloads()` to every 30 frames via `reload_skip` counter

- [x] **Splash/about screen logo rebuilt every frame** (`crates/core/src/app/screens.rs:73-83`) — pre-built `Vec<Line>` cached in `Santui.cached_logo`, invalidated on `ThemeChanged` event

- [x] **Plugin child process leaks on failed `kill()`** (`crates/ipc/src/host.rs:153-161`) — `kill()` now calls `try_wait()` first, logs errors from `kill()`/`wait()`, and safely handles zombies; `reload_plugin()` shuts down old plugin before spawning new one to prevent overlapping orphaned processes

- [x] **`serde` duplicated across workspace** — declared independently (with identical version+features) in 8+ crates instead of using a workspace dep. Moved both `serde` and `serde_json` to `[workspace.dependencies]`; all 9 crates now use `{ workspace = true }`. Template crate kept explicit deps for standalone use.

- [x] **Plugin crash silently tolerated** (`crates/ipc/src/host.rs:117-141`) — added `crashed` flag to `IpcPluginHost`, detected on `send()` and `drain_responses()` when channels disconnect; `is_alive()` added to `Plugin` trait; `PluginManager::tick_all()` collects crashed names; status bar shows `⚠ plugin crashed: <name>` in red

- [x] **`unwrap()` on mpv symbol lookup** (`crates/plugins/radio-streaming-player/src/player.rs:130-138`) — replaced 9 `unwrap()` calls with a `get_sym!()` macro that returns `Err` with a descriptive message when a symbol is missing, instead of panicking

## Medium — latent bugs & performance issues

- [x] **Radio `text_at()` string allocation per text element** — `truncate()` no longer pads when text fits; station list reuses the `text` string directly. (~20-40 allocs saved per frame)
- [x] **Radio `" ".repeat(fill_w)` per row** — migrated from per-row padding to `RenderCmd::Rect` (one alloc per panel, 3 per frame). Host-side `render.rs` still allocates once per `Rect`/`Clear`.
- [x] **Radio DB connection opened per `load()`** — persistent `db: Connection` in `App` struct.
- [x] **`Event::UserUpdated` handler is a no-op** — removed dead `UserUpdated` variant; sign-out/sign-in calls `on_user_update_all()` directly.

## Low — polish

- [x] **No structured logging** — replaced all `eprintln!` with `log::error!`/`log::warn!`; `env_logger` initialized in all 3 binaries with default level `warn`; set `RUST_LOG=debug` for verbose output

- [x] **Themes are compile-time const array** — `config_dir/themes/*.toml` files are now loaded and merged; user themes override built-ins by name

- [x] **OAuth redirect ports are hardcoded** — `bind_with_fallback()` tries 9842…9849, then OS-assigned (port 0); actual port sent to Vercel

- [x] **EventBus is single-consumer** — now supports read-only subscribers via `EventBus::subscribe(Box<dyn FnMut(&Event) + Send>)`

- [x] **Tick rate is hardcoded** — now a `Duration` field on `Santui` with `set_tick_rate()` setter; default 100ms; later moved into `ConfigManager`

- [x] **Star count is hardcoded** — now computed from `(width * height) / 50` (clamped 20-200); resized after terminal init in `run()`

- [x] **Platform manifest filenames hardcoded via cfg** — `manifest_filename()` now uses `std::env::consts::{OS, ARCH}` instead of `cfg!` chains

- [x] **`Plugin` trait doesn't require `Send`** — `trait Plugin: Send`; all `Box<dyn Plugin>` → `Box<dyn Plugin + Send>`; `IpcPluginHost` and `MockPlugin` are already Send

- [x] **`dbg!()` calls remain in production code** — zero `dbg!()` calls found across the workspace; all removed

- [x] **Formatting inconsistencies** — trailing whitespace cleaned up; `cargo fmt` enforced via pre-commit hook

- [x] **`unwrap()` on `tx_msg` before `handle_init`** — replaced with `let Some(tx) = ... else { return; }` to avoid panic if plugin ticked before Init. (`crates/plugins/radio-streaming-player/src/main.rs:322`)

- [x] **`expect()` in scraper SQL transactions** — replaced `expect("begin transaction")`/`expect("commit transaction")` with `if let Err(e) = ... { log::error!(); std::process::exit(1); }`. (`crates/plugins/radio-streaming-player/scraper/src/main.rs:433-434,501`)

- [x] **`env_logger::init()` panics if called twice** — all 3 `init()` calls changed to `let _ = ...try_init()`. (`crates/app/src/main.rs:8-11`, `crates/plugins/radio-streaming-player/src/main.rs:424-427`, `scraper/src/main.rs:384-387`)

- [x] **`Config.clone()` in `apply_config()`** — removed `.clone()`; `config()` already returns `&Config`. Restructured to avoid borrow conflict. (`crates/core/src/app/mod.rs:534`)

- [x] **Unnecessary `PluginCmdItem` clone on palette exec** — removed `.clone()` from tuple; directly index into `commands()` slice. (`crates/core/src/app/handle_key.rs:79`)

- [x] **Dynamic item triple-String clone** — removed `.cloned()` from dynamic items get; clone only `id` + `name` on the cold path. (`crates/core/src/app/handle_key.rs:87`)

- [x] **`filtered_items()` called on arrow key presses** — cached filtered result on `PaletteController`, recompute only on query change. (`crates/core/src/app/palette_controller.rs:54-61`)

- [x] **`query.to_lowercase()` re-alloc per `filtered_items()` call** — controller caching reduces per-frame calls from 2-3× to 1× (render only). (`crates/core/src/app/palette_controller.rs:40`)

- [x] **`PluginContext` constructed twice per frame** — reused `ctx` from before the loop; only updates `ctx.theme` when config triggers a theme change. (`crates/core/src/app/mod.rs:687`)

- [x] **`theme_manager.filtered()` allocates `Vec<usize>` per frame** — cached with dirty-flag comparison against `picker_query`. Returns cached clone when query unchanged. (`crates/core/src/app/theme_manager.rs:86-97`)

- [x] **Config invalid field values silently ignored** — unknown theme names and invalid hex colours now log `warn!` messages. (`crates/core/src/app/mod.rs:555-615`)
