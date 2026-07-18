# Santui — Agent Guide

## Build & Test

```bash
cargo build              # build all workspace crates (including server + plugin binaries)
cargo check              # fast compile check
cargo clippy --workspace -- -D warnings  # lint
cargo fmt --check        # formatting check
cargo fmt                # auto-format
cargo test --workspace   # run tests (SLOW — compiles all 110+ plugin binaries)
cargo test -p santui-core -p santui-ipc -p santui-registry -p santui-db -p santui-server -p santui-auth  # fast — only crates with tests

When running tests, prefer the short list above over `--workspace` to avoid recompiling all 110+ plugin binaries.
```

lefthook pre-commit runs `cargo fmt --check` + `cargo clippy` automatically. Install hooks: `lefthook install`.

Run: `cargo build --workspace && cargo run -p santui` or `.\target\debug\santui.exe`

Server: `cargo run -p santui-server`

Dev mode (plugin registry + native deps):
  - Windows: `.\scripts\dev-setup.ps1 ; $env:SANTUI_DEV=1; cargo run -p santui`
  - macOS/Linux: `./scripts/dev-setup.sh && SANTUI_DEV=1 cargo run -p santui`

Fast dev (skip rebuilding all plugins — only the host + registry):
  - `./scripts/dev-setup.sh --no-build && SANTUI_DEV=1 cargo run -p santui`
  - After adding/changing plugins, run without `--no-build` to rebuild everything.

Watch: `cargo watch -x "run -p santui"`

## Workspace

```
crates/
├── core/          — framework: App, Plugin trait, event loop, palette, sync client
├── ipc/           — IPC protocol types + host (`IpcPluginHost`) plugin runner
├── auth/          — GitHub OAuth + auth handle/client
├── db/            — central SQLite database for per-user plugin data
├── registry/      — plugin registry: manifest fetch, install, config
├── server/        — optional self-hosted sync server (axum + SQLite + JWT)
├── plugins/           — 110+ first-party plugins (see plugins-manifest.json for full list)
│   ├── radio-stream-player/   — radio plugin
│   │   └── scraper/           — scrape radio stations into DB
│   ├── registry/             — plugin registry UI plugin
│   └── ... (108 more plugin directories)
├── app/           — binary entry point (main.rs)
└── website/       — VitePress docs site
```

## Key Conventions

- Rust edition 2021, no nightly
- `ratatui` for rendering; `Theme` semantic colors over hardcoded `Color::*`
- `impl Default` for any type with a `new()` constructor
- `cargo fmt` before commit; clippy must pass with `-D warnings` (enforced by lefthook pre-commit)
- Commit messages must be in English
- **Refactoring / non-trivial changes**: work on a feature branch, push for review, then merge to `main`
- **Don't push on every commit** — only push when explicitly asked or when the branch is ready for review/merging
- **Semantic correctness**: before/after each edit, read the full surrounding function to ensure variable names, types, and logic still make sense. The compiler catches type errors but NOT wrong variable names (e.g. `name` vs `id`) or wrong control flow (e.g. `return` vs `continue`). Re-read the diff yourself before staging.
- **Structural vs semantic filtering**: When filtering a collection (plugins, messages, events), prefer semantic criteria (e.g. "was installed via registry") over structural ones (e.g. "has a binary path"). Built-in plugins can share the same structure as registry-installed ones. A wrong filter compiles fine but causes subtle breakage at runtime (e.g. killing the registry plugin on startup). Add a dedicated tracking field rather than reusing an existing one for a different purpose.
- **No fragile solutions**: every approach must be solid, reliable, and performant. Avoid heuristics (hint-text comparisons, inferred state), silent race windows (timeout + fallback without guarding consumed), unbounded growth (height/width caps), and false positives (marking a plugin as crashed when the channel is merely full). Match on specific error variants rather than `.is_err()` catch-alls.
- **IPC `consumed` protocol**: `PluginMsg.consumed` must be set to `true` when a plugin handles a key event internally (e.g., closing a sub-dialog on Esc). The host uses this to decide whether to fall back to default handling (e.g., closing the plugin on Esc). Every key handler should return a `bool` consumed flag; do NOT rely on heuristics like hint text comparison.
- **plugins-manifest.json + Cargo.toml**: When adding a new plugin you MUST update **both**:
  1. `plugins-manifest.json` — add an entry with `id`, `name`, `description`, `capabilities`. This is the source of truth for the registry (read by `dev-setup.sh` and CI `release.yml`). `plugins.json` (gitignored) is auto-generated.
  2. `Cargo.toml` (root workspace) — add `"crates/plugins/{id}"` to the `members` list. Without this, `cargo build` and `dev-setup.sh` will skip the plugin entirely (as happened with 31 orphaned plugins that had manifest entries but no workspace membership).
- **Never delete code unintentionally**: Every `edit` must preserve all existing lines, functions, and logic unless the user explicitly asked for removal. Before applying an edit, verify that `oldString` matches *only* the intended target and that `newString` includes everything that should remain — especially surrounding code, closing braces, and adjacent statements. When in doubt, prefer a more specific `oldString` with extra context lines to avoid matching the wrong block. After each edit, re-read the file to confirm nothing was silently dropped. A single missing brace or removed line can silently break the build and waste a debugging cycle.
- **Architectural skepticism**: If the AI struggles to fix a bug across multiple attempts (patch after patch, each adding complexity without solving it), step back and question the architecture itself. A fragile timing assumption or wrong abstraction is often the root cause — patching around it never works. The correct fix is to eliminate the assumption, not widen the window. No magic-number timeouts; no "should be fast enough" reasoning.
- **Dependency updates**: Use `cargo upgrade --incompatible allow` (from cargo-edit) to bump Cargo.toml version constraints to latest. Do NOT use `cargo outdated` — it is very slow. After upgrading, fix compilation errors in santui's own code only (not in third-party crates), then run `cargo check --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo fmt`, and `cargo test --workspace`.

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
#
# IMPORTANT: plugins.json is gitignored — update its version field so
# dev-mode (SANTUI_DEV=1) shows the correct version, but DO NOT commit
# it (git add -A skips it automatically).
git add -A && git commit -m "chore: bump version to x.y.z"
git cliff -o CHANGELOG.md   # auto-generate changelog from conventional commits
git add -A && git commit -m "chore: bump version to x.y.z"
git tag vx.y.z && git push origin main vx.y.z
# CI builds binaries, creates GitHub Release, publishes to npm and crates.io
```

Prerequisites:
- `NPM_TOKEN` secret set in GitHub repo Settings → Secrets → Actions
- `CARGO_REGISTRY_TOKEN` secret set in GitHub repo Settings → Secrets → Actions

## Docs Index

- `docs/architecture.md` — architecture & IPC plugin model
- `docs/conventions.md` — coding conventions
- `docs/development.md` — tooling setup, pre-commit checks
