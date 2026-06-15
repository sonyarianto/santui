# Santui — Agent Guide

## Build & Test

```bash
cargo build              # build all workspace crates
cargo check              # fast compile check
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --check        # formatting check
cargo fmt                # auto-format
cargo test --workspace   # run tests
```

Run: `cargo run` or `.\target\debug\santui.exe`

## Workspace Structure

```
santui-core/     — framework core: App, Plugin trait, event loop, palette
santui-radio/    — plugin: internet radio player using libmpv
santui/          — binary entry point; wires plugins together
website/         — VitePress documentation site
```

## Architecture

- **Santui** — main app struct; owns plugin list, palette state, event loop, theme system
- **Plugin trait** — each plugin implements `id`, `name`, `init`, `handle_key`, `render`, `tick`, `on_focus`/`on_blur`, `on_theme_change`
- **Event loop** — `Santui::run()` drives tick, key dispatch, and render
- **Palette** — command palette overlay (`Ctrl+P`); items defined in `CMD_ITEMS`, filtering via substring match. "Switch Theme" cycles through built-in themes.
- **Theme** — `santui-core/src/theme.rs` defines `Theme` struct with 10 semantic color keys + `Default` (Santui purple/orange) and `Theme::nord()` (teal-based). Passed to plugins via `PluginContext.theme` during `init()`. Plugins override `on_theme_change()` to react to runtime theme switches.
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
