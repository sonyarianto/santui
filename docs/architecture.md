# Santui Architecture

## Current Architecture

### Core

- **Santui** — main app struct; delegates to dedicated subsystems: `PluginManager`, `ThemeManager`, `PaletteWidget`, `RegistryScreen`, `StatusBar`, `ConfigManager`, and `EventBus`.
- **Plugin trait** — all methods have default implementations; only `id()`, `name()`, and `init()` are required. Additional lifecycle hooks: `handle_key`, `render`, `tick`, `on_focus`/`on_blur`, `on_theme_change`, `on_user_update`, `status_hints`, `commands`, `handle_palette_command`, `on_plugin_message`.
- **Event loop** — `Santui::run()` drives tick, key dispatch, config polling, event bus draining, and render.
- **Palette** — command palette overlay (`Ctrl+P`); combines built-in commands from `AppState.builtin_items` + dynamic plugin commands from `PluginCmdItem` + plugin-registered commands for enabled registry plugins. "Switch Theme" opens a searchable theme picker (via `ThemeManager`).
- **About screen** — shown on `?` key; uses `render_about()`.
- **Status bar** — extracted to its own module (`status_bar.rs`); rendered at bottom; shows key hints per context.
- **Config** — `ConfigManager` with TOML-based hot-reload; theme + custom colour overrides persist to `config.toml`.

### IPC Plugin Architecture

Plugins run as separate processes via `IpcPluginHost`, which implements the `Plugin` trait and communicates over JSON lines on stdin/stdout:

```
santui.exe (host)
  └─ IpcPluginHost (implements Plugin trait)
       ├─ sends HostMsg (Init, Key, Tick, Resize, ...) via stdin  ──►
       └─ reads PluginMsg { commands, hints, request } via stdout ◄──
            │ spawns & manages
            ▼
       santui-radio-streaming-player.exe (headless, no ratatui)
         └─ pure JSON state machine: input → process → output RenderCmd list
```

- Host owns all ratatui/TUI rendering; plugin is headless with its own native deps (e.g. libmpv).
- Plugin responds to host messages with a full render command list (`Vec<RenderCmd>`). A background reader thread continuously reads plugin stdout, keeping responses non-blocking. `tick()` sends the message and drains pending responses without waiting — only `init`, `key`, `resize`, and `theme_change` block for a response (with a 5-second timeout).
- Render commands (`Text`, `Clear`) are cached on the host and composited into the ratatui buffer each frame — no IPC round-trip on every frame.
- To write a new plugin: create a binary crate depending on `santui-ipc` (protocol types only, no ratatui), implement stdin/stdout JSON loop, then add it to the plugin registry manifest for distribution (see `plugins.json` format in `crates/registry/src/lib.rs`). Plugins are discovered through the registry, installed to `~/.santui/plugins/`, and spawned on demand via the `PluginFactory` set in `main.rs`.

### Plugin Registry

Plugins are managed at runtime through the plugin registry (opened via `Ctrl+P` > "Plugin registry"). The registry fetches a manifest from GitHub Releases (or a local file in dev mode), presents available plugins, and handles install/enable/disable. Installed plugin binaries live in `~/.santui/plugins/` and are launched via `IpcPluginHost` when the user selects them from the palette.

At build time, `cargo build --workspace` produces all workspace binaries including `santui.exe` (host) and any plugin binaries. For production packaging, see `scripts/package-release.ps1` (Windows) or `scripts/package-release-macos.sh` (macOS), which bundle the host binary, plugins, and native dependencies.

### Theme

`crates/core/src/theme.rs` defines `Theme` struct with 12 semantic color keys (`accent`, `highlight`, `logo`, `text`, `text_muted`, `background`, `background_panel`, `background_overlay`, `border`, `success`, `error`, `inverted_text`) + 38 OpenCode themes (from `THEMES` const array). `Default` = Santui (dark neutral `0x141414`, yellow primary `0xffb900`). Passed to plugins via `PluginContext.theme` during `init()`. Plugins override `on_theme_change()` to react to runtime theme switches. `Theme::all()` returns `Vec<(&'static str, Theme)>` for the picker. `text_muted` is computed as 60/40 blend of neutral/ink.

#### Semantic colors

- Accent: `Color::Rgb(157, 124, 216)` (#9d7cd8 purple)
- Highlight bar: `Color::Rgb(250, 178, 131)` (#fab283 OpenCode primary)

## Architecture Roadmap Status

Most phases from the roadmap are already implemented. The current architecture
largely reflects the target:

```
santui.exe (host)
  ├── PluginManager          ← plugin lifecycle, IPC, event bus (Phase 2.1 ✅)
  │    ├── Vec<Box<dyn Plugin>>
  │    ├── IpcPluginHost
  │    └── EventBus           ← plugin-to-plugin messaging (Phase 2.2 ✅)
  ├── PaletteWidget           ← dynamic command registry (Phase 1.1 ✅)
  ├── StatusBar               ← own module (Phase 1.3 ✅)
  ├── ThemeManager            ← theme selection, preview, picker UI (Phase 4.1 ✅)
  ├── ConfigManager           ← hot-reload TOML config (Phase 5.1-5.3 ✅)
  ├── RegistryScreen          ← plugin registry UI (Phase 4.3 ✅)
  ├── About screen
  └── Event loop              ← delegates to subsystems
```

| Component | Status | Phase |
|---|---|---|
| **Command Palette** | Dynamic registry — `AppState.builtin_items` + plugin `PluginCmdItem` entries | 1.1 ✅ |
| **Plugin trait** | Default implementations — only `id`+`name`+`init` required | 1.2 ✅ |
| **Status bar** | Extracted to `status_bar.rs` | 1.3 ✅ |
| **Plugin lifecycle** | `PluginManager` struct | 2.1 ✅ |
| **Plugin comms** | `EventBus` with `emit`/`drain` | 2.2 ✅ |
| **App state** | Centralized in `AppState` struct (`crates/core/src/app/app_state.rs`) | 2.3 ✅ |
| **IPC** | Async background reader thread, non-blocking `tick()` | 3.1 ✅ |
| **Plugin reload** | Binary mtime polling + re-spawn via factory | 3.2 ✅ |
| **Plugin SDK** | `cargo generate` template in `templates/plugin/` | 3.3 ✅ |

See [`docs/roadmap.md`](roadmap.md) for details on remaining phases.
