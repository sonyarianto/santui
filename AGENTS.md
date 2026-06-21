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

Dev mode (plugin registry + native deps):
  - Windows: `.\scripts\dev-setup.ps1 ; $env:SANTUI_DEV=1; cargo run -p santui`
  - macOS/Linux: `./scripts/dev-setup.sh && SANTUI_DEV=1 cargo run -p santui`

Watch: `cargo watch -x "run -p santui"`

## Workspace

```
crates/
├── core/          — framework: App, Plugin trait, event loop, palette
├── ipc/           — IPC protocol types + host (`IpcPluginHost`) plugin runner
├── auth/          — GitHub OAuth + auth handle/client
├── registry/      — plugin registry: manifest fetch, install, config
├── plugins/
│   └── radio-streaming-player/   — radio plugin
│       └── scraper/              — scrape radio stations into DB
└── app/           — binary entry point (main.rs)
website/           — VitePress docs site
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

## Release

```bash
# Update version in all Cargo.toml files + packages/npm/package.json
# They must all match (CI verifies against crates/core/Cargo.toml)
git add -A && git commit -m "chore: bump version to x.y.z"
git tag vx.y.z && git push --tags
# CI builds binaries, creates GitHub Release, and publishes to npm
```

Prerequisites:
- `NPM_TOKEN` secret set in GitHub repo Settings → Secrets → Actions

## Docs Index

- `docs/architecture.md` — architecture & IPC plugin model
- `docs/conventions.md` — coding conventions
- `docs/development.md` — tooling setup (lefthook, pre-commit hooks)
