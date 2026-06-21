# Architecture Audit

Generated 2026-06-20 from a comprehensive codebase review.

## Fixed

- [x] **Thread leak on plugin hot-reload** (`crates/ipc/src/host.rs`) — background reader thread handle was discarded; now joined in `kill()` and `Drop`
- [x] **No graceful plugin shutdown** (`crates/core/src/plugin.rs`, `crates/ipc/src/host.rs`) — `shutdown()` added to `Plugin` trait; `IpcPluginHost` sends `Shutdown` + waits 1s before kill; called on hot-reload

## Critical — crash or corrupt state

- [x] **Mutex poisoning in auth** (`crates/auth/src/lib.rs`) — replaced `.lock().unwrap()` with `.lock().unwrap_or_else(|e| e.into_inner())` on all 4 call sites to recover from poisoning

## High — design problems

- [ ] **Santui is a god object** (`crates/core/src/app/mod.rs:514`) — 13+ fields, growing with every feature.
- [ ] **`handle_key` is a 337-line monolith** (`crates/core/src/app/handle_key.rs:1`) — impossible to test or extend.
- [x] **Config parse failures are silent** (`crates/core/src/config.rs`, `crates/core/src/app/status_bar.rs`) — added `try_load_from` returning `Result`, `ConfigManager.error` field, and `StatusBar.config_error` renders the error in red on the right side of the status bar
- [ ] **Plugin spawn failure returns `Ok(())`** (`crates/ipc/src/host.rs:236`) — dead plugin registered with no retry mechanism.
- [x] **Unbounded EventBus** (`crates/core/src/event.rs`) — switched internal storage to `VecDeque` with a 1024 cap; oldest event dropped when at capacity

## Medium — latent bugs & resource issues

- [ ] **IPC blocks main thread up to 5 seconds** (`crates/ipc/src/host.rs:130`) — `recv_timeout(5s)` freezes the UI during blocking operations.
- [x] **GitHub OAuth blocks main thread** (`crates/auth/src/lib.rs:235`) — GitHub device flow now runs on background thread; TUI stays responsive; code shown in status bar
- [ ] **Google OAuth blocks main thread** (`crates/auth/src/lib.rs:240`) — Google redirect flow still blocks the TUI while waiting for localhost callback
- [x] **Registry file-write crash loses state** (`crates/registry/src/lib.rs:141`) — config saved *before* binary download; push to installed list first, save, then write binary. On error, entry is rolled back.
- [ ] **`Box::leak` in mpv FFI** (`crates/plugins/radio-streaming-player/src/player.rs:123`) — undocumented memory leak of function pointer table.
- [ ] **Unsafe Send+Sync impls without safety docs** (`crates/plugins/radio-streaming-player/src/player.rs:68-69`) — `unsafe impl Send/Sync for Mpv` lacks justification.
- [ ] **Cell<Area> interior mutability** (`crates/ipc/src/host.rs:35`) — works because single-threaded, but `Cell` is `!Sync`. Would be a data race if rendering or plugin comms ever moved to separate threads.
- [x] **`handle_key` calls blocking GitHub OAuth on main thread** (`crates/core/src/app/handle_key.rs:62`) — switched to non-blocking `start_sign_in()` for GitHub; Google sign-in still blocks but is faster (redirect-based).
- [x] **Radio plugin thread leak** (`crates/plugins/radio-streaming-player/src/main.rs:103`) — mpv event thread handle now stored; `MpvCmd::Quit` sent on shutdown, thread joined.

## Low — polish

- [ ] No structured logging (all `eprintln!`)
- [ ] Themes are compile-time const array — no user-defined or runtime-loaded themes
- [ ] No security/capability model between plugins
- [ ] OAuth redirect ports (9842/9843) are hardcoded with no fallback
- [ ] EventBus is single-consumer — adding an event logger requires modifying core code
- [ ] Tick rate (100ms) is hardcoded, not user-configurable
- [ ] Star count (88) is hardcoded, not adaptive to terminal resolution
- [ ] Platform manifest filenames hardcoded via cfg checks
- [ ] `Plugin` trait doesn't require `Send` or `Sync`, preventing future parallel execution
