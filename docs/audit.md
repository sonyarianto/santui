# Architecture Audit

Generated 2026-06-20 from a comprehensive codebase review.

## Fixed

- [x] **Thread leak on plugin hot-reload** (`crates/ipc/src/host.rs`) — background reader thread handle was discarded; now joined in `kill()` and `Drop`

## Critical — crash or corrupt state

- [ ] **Mutex poisoning in auth** (`crates/auth/src/lib.rs:226`) — `.lock().unwrap()` panics if OAuth panics, poisoning the mutex. Every subsequent access panics.
- [ ] **No graceful plugin shutdown** — `Plugin` trait has no `shutdown()` or `cleanup()` method. IPC plugins get SIGKILL with no chance to flush state.

## High — design problems

- [ ] **Santui is a god object** (`crates/core/src/app/mod.rs:514`) — 13+ fields, growing with every feature.
- [ ] **`handle_key` is a 337-line monolith** (`crates/core/src/app/handle_key.rs:1`) — impossible to test or extend.
- [ ] **Config parse failures are silent** (`crates/core/src/config.rs:55`) — user edits `config.toml`, parse error goes to stderr, default loads silently. No UI feedback.
- [ ] **Plugin spawn failure returns `Ok(())`** (`crates/ipc/src/host.rs:236`) — dead plugin registered with no retry mechanism.
- [ ] **Unbounded EventBus** (`crates/core/src/event.rs:26`) — `pending: Vec<Event>` grows without limit if events are emitted faster than drained.

## Medium — latent bugs & resource issues

- [ ] **IPC blocks main thread up to 5 seconds** (`crates/ipc/src/host.rs:130`) — `recv_timeout(5s)` freezes the UI during blocking operations.
- [ ] **OAuth blocks main thread indefinitely** (`crates/auth/src/lib.rs:235`) — no timeout if browser doesn't open or user doesn't complete the flow.
- [ ] **Registry file-write crash loses state** (`crates/registry/src/lib.rs:141`) — binary written to disk but config.toml not saved before crash. Plugin exists but is unknown on restart.
- [ ] **`Box::leak` in mpv FFI** (`crates/plugins/radio-streaming-player/src/player.rs:123`) — undocumented memory leak of function pointer table.
- [ ] **Unsafe Send+Sync impls without safety docs** (`crates/plugins/radio-streaming-player/src/player.rs:68-69`) — `unsafe impl Send/Sync for Mpv` lacks justification.
- [ ] **Cell<Area> interior mutability** (`crates/ipc/src/host.rs:35`) — works because single-threaded, but `Cell` is `!Sync`. Would be a data race if rendering or plugin comms ever moved to separate threads.
- [ ] **`handle_key` calls blocking OAuth on main thread** (`crates/core/src/app/handle_key.rs:62`) — sign-in flow freezes UI.
- [ ] **Radio plugin thread leak** (`crates/plugins/radio-streaming-player/src/main.rs:103`) — mpv event thread is never joined.

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
