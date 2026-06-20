# Santui Architecture Roadmap

High-level plan for making Santui's architecture scalable, maintainable, and
plugin-friendly as the project grows.

**Status legend:** ✅ implemented · ❌ not yet started

---

## Phase 1 — Quick Wins (all done ✅)

### 1.1 Dynamic Command Registry ✅

**Problem:** `CMD_ITEMS` in `palette.rs` was hardcoded. Plugins couldn't register
their own commands dynamically.

**Solution (implemented):**
- `PluginCmdItem` type added to `santui-core`
- `commands()` method on `Plugin` trait returns `Vec<PluginCmdItem>`
- `PaletteWidget` renders built-in + dynamic + plugin commands side by side

**Key files:** `plugin.rs` (trait), `palette_widget.rs` (rendering), `handle_key.rs` (dispatch)

### 1.2 Simplify Plugin Trait with Default Implementations ✅

**Problem:** Plugin trait had 10 methods. Every plugin had to implement all of
them, even if most were no-ops.

**Solution (implemented):**
```rust
pub trait Plugin {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn init(&mut self, ctx: &PluginContext) -> Result<(), Box<dyn Error>>;

    // All optional — default no-op implementations
    fn handle_key(&mut self, _key: KeyEvent) -> bool { false }
    fn render(&self, _f: &mut Frame, _area: Rect) {}
    fn tick(&mut self) {}
    fn on_focus(&mut self) {}
    fn on_blur(&mut self) {}
    fn on_theme_change(&mut self, _theme: &Theme) {}
    fn on_user_update(&mut self, _user: Option<&User>) {}
    fn status_hints(&self) -> Vec<(String, String)> { vec![] }
    fn commands(&self) -> Vec<PluginCmdItem> { vec![] }
    fn handle_palette_command(&mut self, _index: usize) {}
    fn on_plugin_message(&mut self, _from: &str, _action: &str, _data: &str) {}
}
```

**Key files:** `plugin.rs`

### 1.3 Extract StatusBar from Santui ✅

**Problem:** `Santui` struct handled status bar rendering inline.

**Solution (implemented):** `StatusBar` is now its own module (`crates/core/src/app/status_bar.rs`).
`Santui::render()` creates a `StatusBar` value and calls `render()` on it.

**Key files:** `status_bar.rs`

---

## Phase 2 — Structural Improvements (all done ✅)

### 2.1 PluginManager — Extract Plugin Lifecycle from Santui ✅

**Problem:** `Santui` owned the plugin list directly — loading, spawning, IPC host
management was all inline.

**Solution (implemented):**
```
Santui
  └── PluginManager
        ├── Vec<Box<dyn Plugin>>
        ├── active plugin dispatch
        └── palette-command registry
```

`Santui` delegates to `PluginManager`:
```rust
self.plugin_manager.handle_key(idx, key);
self.plugin_manager.render(idx, f, area);
self.plugin_manager.tick_all();
```

**Key files:** `plugin_manager.rs`

### 2.2 Event Bus — Plugin-to-Plugin Communication ✅

**Problem:** Plugins couldn't talk to each other. No way for one plugin to
trigger action in another.

**Solution (implemented):** Simple `EventBus` with `emit()`/`drain()` queue.
Main loop drains events once per frame and forwards to `PluginManager::process_events()`.
Plugin-to-plugin messages are forwarded via `on_plugin_message()`.

**Key files:** `event.rs`, `plugin_manager.rs` (process_events)

### 2.3 App State — Centralized State ✅

**Problem:** State was scattered across `Santui`, plugins, and palette. No
single source of truth. Theme was duplicated in 3 places (`Santui.theme`,
`ThemeManager.current`, `PluginContext.theme`).

**Solution (implemented):**
```rust
pub struct AppState {
    pub running: bool,
    pub show_about: bool,
    pub theme: Theme,
    pub theme_picker_open: bool,
    pub registry_open: bool,
}
```

- `theme` is now single-sourced in `AppState`
- `PluginContext` is created on-the-fly during `init()`, no longer stored on `Santui`
- Starfield animation extracted to its own `Starfield` module
- `RegistryScreen.open` and `ThemeManager.picker_open` moved to `AppState`

**Key files:** `app_state.rs`, `starfield.rs`

---

## Phase 3 — Advanced (partially done ✅ / ❌)

### 3.1 Async / Non-blocking IPC ✅

**Problem:** Plugin IPC was synchronous — host sent message, plugin responded
before anything else happened. Slow plugins blocked the UI.

**Solution (implemented — option B):**
```
Event loop runs on main thread
IPC runs on separate thread with channel
Plugin messages queued → processed on next tick
```

A background reader thread continuously reads plugin stdout. `tick()` sends the
message and drains pending responses without blocking. A 5-second timeout on
`send_recv()` prevents the main thread from hanging on a crashed plugin.

**Key files:** `host.rs` (`spawn` creates reader thread, `drain_responses`, `recv` with timeout)

### 3.2 Plugin Hot-Reload ✅

**Problem:** Plugin changes require full app restart.

**Solution (implemented):**
- `Plugin::binary_path()` returns the filesystem path to the plugin's binary
- `PluginManager` polls binary mtimes once per frame in the event loop
- When a binary changes, `PluginManager::reload_plugin()` recreates the plugin via
  the factory, calls `init()` with current context, then swaps in the new instance
- Old `IpcPluginHost` is dropped, which kills the stale child process and its
  background reader thread
- In-process plugins (no binary path) are skipped

**Key files:** `plugin.rs` (trait method), `plugin_manager.rs` (reload_plugin, check_reloads), `host.rs` (binary_path)

### 3.3 Plugin SDK / Generator ❌

**Problem:** Writing a plugin requires understanding IPC protocol, Plugin trait,
registry manifest, etc.

**Planned solution:**
```
cargo generate --git https://github.com/sonyarianto/santui-plugin-template
```

A `cargo generate` template that scaffolds a working plugin with:
- `Cargo.toml` with `santui-ipc` dependency
- `main.rs` with JSON stdin/stdout loop
- Build script that packages manifest

**Status:** Not yet started.

---

## Summary

| Phase | Item | Effort | Priority | Status |
|---|---|---|---|---|
| 1.1 | Dynamic Command Registry | 🟢 Low | 🔥 High | ✅ |
| 1.2 | Simplify Plugin Trait | 🟢 Low | 🔥 High | ✅ |
| 1.3 | Extract StatusBar | 🟢 Low | 🟡 Medium | ✅ |
| 2.1 | PluginManager | 🟡 Medium | 🔥 High | ✅ |
| 2.2 | Event Bus | 🟡 Medium | 🟡 Medium | ✅ |
| 2.3 | App State | 🟡 Medium | 🟡 Medium | ✅ |
| 3.1 | Async IPC | 🔴 High | 🔵 Low | ✅ |
| 3.2 | Hot-Reload | 🔴 High | 🔵 Low | ✅ |
| 3.3 | Plugin SDK | 🟡 Medium | 🟡 Medium | ❌ |
