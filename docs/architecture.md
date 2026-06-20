# Santui Architecture

## Core

- **Santui** — main app struct; owns plugin list, palette state, event loop, theme system
- **Plugin trait** — each plugin implements `id`, `name`, `init`, `handle_key`, `render`, `tick`, `on_focus`/`on_blur`, `on_theme_change`, `status_hints`, `on_user_update`
- **Event loop** — `Santui::run()` drives tick, key dispatch, and render
- **Palette** — command palette overlay (`Ctrl+P`); items defined in `CMD_ITEMS`, filtering via substring match. "Switch Theme" opens a searchable theme picker showing all 38 OpenCode themes.
- **About screen** — shown on `?` key; uses `render_about()`
- **Status bar** — rendered at bottom; shows key hints per context

## IPC Plugin Architecture

Plugins can run as separate processes via `IpcPluginHost`, which implements the `Plugin` trait and communicates over JSON lines on stdin/stdout:

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
- Plugin responds synchronously to every host message with a full render command list (`Vec<RenderCmd>`).
- Render commands (`Text`, `Clear`) are cached on the host and composited into the ratatui buffer each frame — no IPC round-trip on every frame.
- To write a new plugin: create a binary crate depending on `santui-ipc` (protocol types only, no ratatui), implement stdin/stdout JSON loop, then add it to the plugin registry manifest for distribution (see `plugins.json` format in `santui-registry/src/lib.rs`). Plugins are discovered through the registry, installed to `~/.santui/plugins/`, and spawned on demand via the `PluginFactory` set in `main.rs`.

## Plugin Registry

Plugins are managed at runtime through the plugin registry (opened via `Ctrl+P` > "Plugin registry"). The registry fetches a manifest from GitHub Releases (or a local file in dev mode), presents available plugins, and handles install/enable/disable. Installed plugin binaries live in `~/.santui/plugins/` and are launched via `IpcPluginHost` when the user selects them from the palette.

At build time, `cargo build --workspace` produces all workspace binaries including `santui.exe` (host) and any plugin binaries. For production packaging, see `scripts/package-release.ps1` (Windows) or `scripts/package-release-macos.sh` (macOS), which bundle the host binary, plugins, and native dependencies.

## Theme

`santui-core/src/theme.rs` defines `Theme` struct with ~10 semantic color keys + all 38 OpenCode themes (from `THEMES` const array). `Default` = Santui (dark neutral `0x141414`, yellow primary `0xffb900`). Passed to plugins via `PluginContext.theme` during `init()`. Plugins override `on_theme_change()` to react to runtime theme switches. `Theme::all()` returns `Vec<(&'static str, Theme)>` for the picker. `text_muted` is computed as 60/40 blend of neutral/ink.

### Semantic colors

- Accent: `Color::Rgb(157, 124, 216)` (#9d7cd8 purple)
- Highlight bar: `Color::Rgb(250, 178, 131)` (#fab283 OpenCode primary)
