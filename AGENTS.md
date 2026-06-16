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

Watch: `cargo watch -x "run -p santui"`

## Workspace

```
santui-core/     — framework core: App, Plugin trait, event loop, palette
santui-ipc/      — IPC protocol types + host (`IpcPluginHost`) plugin runner
santui-radio-streaming-player/    — radio plugin
  └─ scraper/                     — scrape radio stations into DB
santui/          — binary entry point
website/         — VitePress docs site
```

## Key Conventions

- Rust edition 2021, no nightly
- `ratatui` for rendering; `Theme` semantic colors over hardcoded `Color::*`
- `impl Default` for any type with a `new()` constructor
- `cargo fmt` before commit; clippy must pass with `-D warnings`

## Website

```bash
cd website && npm run dev   # dev server
cd website && npm run build # static build
```

## Docs Index

- `docs/architecture.md` — architecture & IPC plugin model
- `docs/conventions.md` — coding conventions
- `docs/development.md` — tooling setup (lefthook, pre-commit hooks)
