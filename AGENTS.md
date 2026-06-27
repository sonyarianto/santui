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
├── db/            — central SQLite database for per-user plugin data
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
- Commit messages must be in English
- **Refactoring / non-trivial changes**: work on a feature branch, push for review, then merge to `main`
- **Semantic correctness**: before/after each edit, read the full surrounding function to ensure variable names, types, and logic still make sense. The compiler catches type errors but NOT wrong variable names (e.g. `name` vs `id`) or wrong control flow (e.g. `return` vs `continue`). Re-read the diff yourself before staging.
- **No fragile solutions**: every approach must be solid, reliable, and performant. Avoid heuristics (hint-text comparisons, inferred state), silent race windows (timeout + fallback without guarding consumed), unbounded growth (height/width caps), and false positives (marking a plugin as crashed when the channel is merely full). Match on specific error variants rather than `.is_err()` catch-alls.
- **IPC `consumed` protocol**: `PluginMsg.consumed` must be set to `true` when a plugin handles a key event internally (e.g., closing a sub-dialog on Esc). The host uses this to decide whether to fall back to default handling (e.g., closing the plugin on Esc). Every key handler should return a `bool` consumed flag; do NOT rely on heuristics like hint text comparison.

## Website

```bash
cd website && npm run dev   # dev server
cd website && npm run build # static build
```

## Release

```bash
# Update version in all Cargo.toml files + packages/npm/package.json
# They must all match (CI verifies against crates/core/Cargo.toml)
#
# IMPORTANT: Every inter-crate path dependency must also have a version
# field matching the new version, e.g.:
#   santui-core = { path = "../core", version = "x.y.z" }
#
# Also update website version strings (grep for the old version):
#   website/.vitepress/config.ts      — nav link + footer
#   website/public/install.ps1        — banner text
#   website/index.md                  — tagline (if changed)
git add -A && git commit -m "chore: bump version to x.y.z"
git tag vx.y.z && git push --tags
# CI builds binaries, creates GitHub Release, publishes to npm and crates.io
```

Prerequisites:
- `NPM_TOKEN` secret set in GitHub repo Settings → Secrets → Actions
- `CARGO_REGISTRY_TOKEN` secret set in GitHub repo Settings → Secrets → Actions

## Docs Index

- `docs/architecture.md` — architecture & IPC plugin model
- `docs/conventions.md` — coding conventions
- `docs/development.md` — tooling setup (lefthook, pre-commit hooks)
