# Santui Architecture

## Current Architecture

### Core

- **Santui** — main app struct; delegates to dedicated subsystems: `PluginManager`, `ThemeManager`, `PaletteController`, `RegistryScreen`, `StatusBar`, `ConfigManager`, and `EventBus`.
- **Plugin trait** — all methods have default implementations; only `id()`, `name()`, and `init()` are required. Additional lifecycle hooks: `handle_key`, `render`, `tick`, `process_pending_requests`, `on_focus`/`on_blur`, `on_theme_change`, `on_user_update`, `status_hints`, `commands`, `handle_palette_command`, `on_plugin_message`, `shutdown`, `binary_path`, `is_alive`, `can_background`, `set_capabilities`.
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
        ├─ sends HostMsg (Init, Key, Tick, Resize, DbValue, ...) via stdin  ──►
        └─ reads PluginMsg { commands, hints, palette_commands, request, consumed } via stdout ◄──
            │ spawns & manages
            ▼
       santui-radio-stream-player.exe (headless, no ratatui)
         └─ pure JSON state machine: input → process → output RenderCmd list
```

- Host owns all ratatui/TUI rendering; plugin is headless with its own native deps (e.g. libmpv).
- Plugin responds to host messages with a full render command list (`Vec<RenderCmd>`). A background reader thread continuously reads plugin stdout, keeping responses non-blocking. `tick()` sends the message and drains pending responses without waiting — `init()` is non-blocking — it spawns background threads and returns immediately without waiting for a response; a loading state is shown until the plugin sends its first response. Key events are non-blocking: the host sends the key, drains pending responses, and uses the latest `consumed` flag. For `Esc`, the host uses an event-driven protocol resolved over ~10 frames (~1s).
- Render commands (`Text`, `Clear`, `Rect`, `Dim`, `Border`, `Paragraph`, `List`, `Table`) are cached on the host and composited into the ratatui buffer each frame — no IPC round-trip on every frame.
- To write a new plugin: create a binary crate depending on `santui-ipc` (protocol types only, no ratatui), implement stdin/stdout JSON loop, then add it to the plugin registry manifest for distribution (see `plugins.json` format in `crates/registry/src/lib.rs`; note that `plugins.json` is gitignored — see `AGENTS.md`). Plugins are discovered through the registry, installed to `~/.santui/plugins/`, and spawned on demand via the `PluginFactory` set in `main.rs`.

### Plugin Registry

Plugins are managed at runtime through the plugin registry (opened via `Ctrl+P` > "Plugin registry"). The registry fetches a manifest from GitHub Releases (or a local file in dev mode), presents available plugins, and handles install/enable/disable. Installed plugin binaries live in `~/.santui/plugins/` and are launched via `IpcPluginHost` when the user selects them from the palette.

#### Plugin Lifecycle Flow

**Data sources:**

| Source | Format | Contents | When loaded |
|--------|--------|----------|-------------|
| `registry.toml` | TOML | `installed` — list of plugins the user installed, with `enabled`, `version`, `path` | At startup (in `Registry::new()`) |
| Plugin manifest | JSON | `available` — full catalogue of plugins (`id`, `name`, `description`, `version`, `download_url`, `sha256`, `size`, `publisher`, `capabilities`) | When the registry screen is opened (`fetch_manifest()` for PROD, `load_local_manifest()` for DEV) |

**Install flow:**

1. User presses Enter on a plugin in the registry screen.
2. `registry.install()` pushes a new `InstalledPlugin { enabled: true, .. }` to `installed` and calls `save_config()` — this writes `registry.toml` immediately, *before* downloading the binary.
3. The binary is downloaded (or copied, in DEV mode). If the download fails, the entry is rolled back (pop + save).
4. The caller receives `RegistryAction::ItemsChanged`, which triggers `plugin_manager.refresh_dynamic_items()`.

**How palette "Plugins" are populated:**

`refresh_dynamic_items()` in `PluginManager` iterates `reg.installed` directly (not `reg.available`), so enabled plugins appear in the palette immediately — even before the manifest is fetched. If the manifest has been loaded, the display name is taken from it; otherwise it's humanized from the binary filename (e.g. `santui-radio-stream-player` → `Radio Stream Player`).

The call sites:

| When | Where | Why |
|------|-------|-----|
| App startup | `Santui::run()` after `init_all` | Populate palette before first render |
| Registry opened | `open_registry()` after manifest load | Refresh names from manifest data |
| Plugin toggled | `handle_key_registry()` on `ItemsChanged` | Reflect enable/disable immediately |

**DEV vs PROD:**

| | DEV (`SANTUI_DEV=1`) | PRODUCTION |
|---|----------------------|------------|
| Manifest source | `plugins.json` (local file, path from `SANTUI_DEV_MANIFEST` or cwd) | `plugins-{triple}.json` from GitHub Releases (configurable via `SANTUI_REPO`) |
| Binary install | Copies from `download_url` path (local file) | Downloads from GitHub release asset, verifies SHA-256 |
| Native deps | `copy_native_deps()` syncs `native/*.dll` alongside the binary | Bundled in release archive, extracted during download |
| Manifest fetch | `load_local_manifest()` — instant | `fetch_manifest()` — HTTP request, ~100-500ms |

Everything else (install, enable/disable, TOML persistence, `refresh_dynamic_items`, plugin spawning) is identical between DEV and PROD.

At build time, `cargo build --workspace` produces all workspace binaries including `santui.exe` (host) and any plugin binaries. For production packaging, see `scripts/package-release.ps1` (Windows) or `scripts/package-release-macos.sh` (macOS), which bundle the host binary, plugins, and native dependencies.

### Background-capable plugins

Plugins that declare `"capabilities": ["background"]` in their manifest entry (e.g. the radio stream player) survive `Esc` — instead of being shut down, they receive a `Blur` message and are hidden from the display. The audio (or any background activity) keeps running.

When the user re-selects the plugin from the palette/carousel, the host sends `Focus` — no re-spawn needed. On app exit, all plugins are shut down regardless of capability.

The `can_background()` method on the `Plugin` trait defaults to `false`. `IpcPluginHost` overrides it to check the `capabilities` vec set at spawn time via `set_capabilities()`. The `PluginFactory` does not need to know about capabilities — they are applied after construction in `spawn_and_init()`.

### Theme

`crates/core/src/theme.rs` defines `Theme` struct with 12 semantic color keys (`accent`, `highlight`, `logo`, `text`, `text_muted`, `background`, `background_panel`, `background_overlay`, `border`, `success`, `error`, `inverted_text`) + 38 OpenCode themes (from `THEMES` const array). `Default` = Santui (dark neutral `0x141414`, yellow primary `0xffb900`). Passed to plugins via `PluginContext.theme` during `init()`. Plugins override `on_theme_change()` to react to runtime theme switches. `Theme::all()` returns `Vec<(&'static str, Theme)>` for the picker. `text_muted` is computed as 60/40 blend of neutral/ink.

#### Semantic colors (OpenCode theme example)

- Accent: `Color::Rgb(157, 124, 216)` (#9d7cd8 purple)
- Highlight bar: `Color::Rgb(250, 178, 131)` (#fab283 — OpenCode's primary; Santui default is #ffb900 gold)

## Architecture Roadmap Status

Most phases from the roadmap are already implemented. The current architecture
largely reflects the target:

```
santui.exe (host)
  ├── PluginManager          ← plugin lifecycle, IPC, event bus (Phase 2.1 ✅)
  │    ├── Vec<Box<dyn Plugin>>
  │    ├── IpcPluginHost
  │    ├── db: Box<dyn DbAccess>  ← per-user key-value store
  │    └── EventBus           ← plugin-to-plugin messaging (Phase 2.2 ✅)
  ├── PaletteController       ← dynamic command registry (Phase 1.1 ✅)
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

See the GitHub Issues & Milestones for details on remaining phases.
