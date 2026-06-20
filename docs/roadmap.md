# Santui Architecture Roadmap

High-level plan for making Santui's architecture scalable, maintainable, and
plugin-friendly as the project grows.

---

## Phase 1 — Quick Wins (low effort, high impact)

### 1.1 Dynamic Command Registry

**Problem:** `CMD_ITEMS` in `palette.rs` is hardcoded. Plugins can't register
their own commands dynamically.

**Solution:**
```
Palette
  ├── built-in commands (hardcoded)
  └── plugin commands (registered at init via PluginContext)
```

Plugin trait gets a new method:
```rust
fn commands(&self) -> Vec<Command> {
    vec![]  // default: none
}
```

Where `Command { id, name, handler }` is a new shared type in `santui-core`.

**Impact:** Low effort (~2 files). High impact — plugins can add to Ctrl+P.

### 1.2 Simplify Plugin Trait with Default Implementations

**Problem:** Plugin trait has 10 methods. Every plugin must implement all of
them, even if most are no-ops.

**Solution:**
```rust
pub trait Plugin {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;

    // All optional — default no-op implementations
    fn init(&mut self, _ctx: &PluginContext) -> Result<(), String> { Ok(()) }
    fn handle_key(&mut self, _key: KeyEvent) -> Result<PluginAction, String>
        { Ok(PluginAction::None) }
    fn render(&mut self, _area: Rect, _buf: &mut Buffer) -> Result<(), String>
        { Ok(()) }
    fn tick(&mut self) -> Result<PluginAction, String>
        { Ok(PluginAction::None) }
    fn on_focus(&mut self) {}
    fn on_blur(&mut self) {}
    fn on_theme_change(&mut self, _theme: &Theme) {}
    fn status_hints(&self) -> Vec<StatusHint> { vec![] }
    fn commands(&self) -> Vec<Command> { vec![] }  // from 1.1
}
```

**Impact:** Low effort. Plugin code halves in size. No breaking changes.

### 1.3 Extract StatusBar from Santui

**Problem:** `Santui` struct handles status bar rendering inline.

**Solution:** Move status bar into its own module/file (`crates/core/src/app/status_bar.rs`).
`Santui` just calls `StatusBar::render(&self.status_bar, area, buf)`.

**Impact:** Trivial. Cleaner separation.

---

## Phase 2 — Structural Improvements (moderate effort)

### 2.1 PluginManager — Extract Plugin Lifecycle from Santui

**Problem:** `Santui` owns plugin list directly — loading, spawning, IPC host
management is all inline.

**Solution:**
```
Santui
  └── PluginManager
        ├── Vec<Box<dyn Plugin>>
        ├── load / unload / enable / disable
        ├── IPC host lifecycle
        └── plugin-to-plugin event bus
```

`Santui` delegates to `PluginManager`:
```rust
self.plugin_manager.handle_key(key)?;
self.plugin_manager.tick()?;
self.plugin_manager.render(area, buf)?;
```

**Impact:** Moderate (~3 new files). `Santui` shrinks significantly. Plugin
lifecycle becomes testable in isolation.

### 2.2 Event Bus — Plugin-to-Plugin Communication

**Problem:** Plugins can't talk to each other. No way for one plugin to
trigger action in another.

**Solution:**
```rust
pub struct EventBus {
    subscribers: HashMap<EventId, Vec<Box<dyn Fn(&Event)>>>,
}

pub enum Event {
    StationChanged(String),
    ThemeSwitched(String),
    AppQuit,
    // ...
}
```

`PluginManager` owns `EventBus`. Plugins subscribe during `init()`:
```rust
fn init(&mut self, ctx: &PluginContext) -> Result<(), String> {
    ctx.event_bus.subscribe("station-changed", |event| {
        // react
    });
}
```

**Impact:** Moderate. Enables powerful workflows (e.g. radio player →
lyrics plugin).

### 2.3 App State — Centralized State

**Problem:** State is scattered across `Santui`, plugins, and palette. No
single source of truth.

**Solution:**
```rust
pub struct AppState {
    pub theme: Theme,
    pub screen: Screen,
    pub palette_visible: bool,
    pub status_message: Option<String>,
    // ...
}
```

Passed by reference to all subsystems instead of each holding its own copy.

**Impact:** Moderate. Reduces bugs from stale/mismatched state.

---

## Phase 3 — Advanced (high effort, high reward)

### 3.1 Async / Non-blocking IPC

**Problem:** Plugin IPC is synchronous — host sends message, plugin responds
before anything else happens. Slow plugins block the UI.

**Solution (option A — simple):**
```
Host sends message → continues event loop
Plugin responds asynchronously → render cached frame
Plugin response queued → applied on next render
```

**Solution (option B — proper):**
```
Event loop runs on main thread
IPC runs on separate thread with channel
Plugin messages queued → processed on next tick
```

**Impact:** High effort. But unlocks non-blocking plugins (e.g. network fetch).

### 3.2 Plugin Hot-Reload

**Problem:** Plugin changes require full app restart.

**Solution:**
```
File watcher (notify crate) monitors ~/.santui/plugins/
On change: graceful shutdown → dlopen or re-spawn → init
```

**Impact:** High effort. Great DX for plugin developers.

### 3.3 Plugin SDK / Generator

**Problem:** Writing a plugin requires understanding IPC protocol, Plugin trait,
registry manifest, etc.

**Solution:**
```
cargo generate --git https://github.com/sonyarianto/santui-plugin-template
```

A `cargo generate` template that scaffolds a working plugin with:
- `Cargo.toml` with `santui-ipc` dependency
- `main.rs` with JSON stdin/stdout loop
- `lib.rs` with plugin logic
- Build script that packages manifest

**Impact:** Medium effort. Lowers barrier to entry significantly.

---

## Summary

| Phase | Item | Effort | Priority |
|---|---|---|---|
| 1.1 | Dynamic Command Registry | 🟢 Low | 🔥 High |
| 1.2 | Simplify Plugin Trait | 🟢 Low | 🔥 High |
| 1.3 | Extract StatusBar | 🟢 Low | 🟡 Medium |
| 2.1 | PluginManager | 🟡 Medium | 🔥 High |
| 2.2 | Event Bus | 🟡 Medium | 🟡 Medium |
| 2.3 | App State | 🟡 Medium | 🟡 Medium |
| 3.1 | Async IPC | 🔴 High | 🔵 Low (for now) |
| 3.2 | Hot-Reload | 🔴 High | 🔵 Low (for now) |
| 3.3 | Plugin SDK | 🟡 Medium | 🟡 Medium |
