# Santui — Agent Guide

## Build & Test

```bash
cargo build              # build all workspace crates (including plugin binaries)
cargo check              # fast compile check
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --check        # formatting check
cargo fmt                # auto-format
cargo test --workspace   # run tests
```

Run: `cargo build --workspace && cargo run -p santui` or `.\target\debug\santui.exe`

## Workspace Structure

```
santui-core/     — framework core: App, Plugin trait, event loop, palette
santui-ipc/      — IPC protocol types + host (`IpcPluginHost`) plugin runner
santui-radio-streaming-player/    — plugin: internet radio streaming player using libmpv
santui/          — binary entry point; wires plugins together
website/         — VitePress documentation site
```

## Architecture

- **Santui** — main app struct; owns plugin list, palette state, event loop, theme system
- **Plugin trait** — each plugin implements `id`, `name`, `init`, `handle_key`, `render`, `tick`, `on_focus`/`on_blur`, `on_theme_change`, `status_hints`
- **IPC Plugin Architecture** — plugins can run as **separate processes** via `IpcPluginHost`, which implements the `Plugin` trait and communicates over JSON lines on stdin/stdout:

  ```
  santui.exe (host)
    └─ IpcPluginHost (implements Plugin trait)
         ├─ sends HostMsg (Init, Key, Tick, Resize, ...) via stdin  ──►
         └─ reads PluginMsg (Render { commands, hints }) via stdout ◄──
              │ spawns & manages
              ▼
         santui-radio-streaming-player.exe (headless, no ratatui)
           └─ pure JSON state machine: input → process → output RenderCmd list
  ```

  - Host owns all ratatui/TUI rendering; plugin is headless with its own native deps (e.g. libmpv).
  - Plugin responds synchronously to every host message with a full render command list (`Vec<RenderCmd>`).
  - Render commands (`Text`, `Clear`) are cached on the host and composited into the ratatui buffer each frame — no IPC round-trip on every frame.
  - To write a new plugin: create a binary crate depending on `santui-ipc` (protocol types only, no ratatui), implement stdin/stdout JSON loop, register it in `santui/src/main.rs` via `IpcPluginHost::new("id", "name", "binary-name")`.
- **Event loop** — `Santui::run()` drives tick, key dispatch, and render
- **Palette** — command palette overlay (`Ctrl+P`); items defined in `CMD_ITEMS`, filtering via substring match. "Switch Theme" opens a searchable theme picker showing all 38 OpenCode themes.
- **Theme** — `santui-core/src/theme.rs` defines `Theme` struct with 10 semantic color keys + all 38 OpenCode themes (from `THEMES` const array). `Default` = Santui (dark neutral `0x141414`, yellow primary `0xffb900`). Passed to plugins via `PluginContext.theme` during `init()`. Plugins override `on_theme_change()` to react to runtime theme switches. `Theme::all()` returns `Vec<(&'static str, Theme)>` for the picker. `text_muted` is computed as 60/40 blend of neutral/ink.
- **About screen** — shown on `?` key; uses `render_about()`
- **Status bar** — rendered at bottom; shows key hints per context

## Conventions

- Rust edition 2021, no nightly features
- Use `ratatui` for all terminal rendering (no direct terminal writes except crossterm for raw mode)
- Use `Color::Rgb(r, g, b)` for custom colors
- Accent color: `Color::Rgb(157, 124, 216)` (#9d7cd8 purple)
- Highlight bar: `Color::Rgb(250, 178, 131)` (#fab283 OpenCode primary)
- All widgets use ratatui's `Frame`, `Layout`, `Rect`, `Style`, `Span`, `Line`, `Paragraph`
- Use `Theme` semantic colors instead of hardcoded `Color::*` — add new fields to `Theme` if needed
- Add `impl Default` for any type with a `new()` constructor (clippy rule)
- `cargo fmt` before commit; clippy must pass with `-D warnings`

## Website

```bash
cd website && npm run dev   # dev server
cd website && npm run build # static build
```

Deployed on Vercel at https://santui.vercel.app
