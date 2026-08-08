# Santui — Agent Guide

## Build & Test

```bash
./scripts/check.sh check    # fast compile check — host + core + stable plugins only
./scripts/check.sh clippy   # lint (same fast set)
./scripts/check.sh test     # tests (core crates + stable plugin tests)
./scripts/check.sh fmt      # formatting check
cargo fmt                   # auto-format

cargo build -p santui       # build just the host app (add specific -p flags for more)
cargo check --workspace     # SLOW — all ~127 crates incl. experimental plugins (CI's job)
cargo clippy --workspace -- -D warnings  # SLOW — only needed when touching experimental plugins
cargo clippy --workspace --all-targets -- -D warnings  # SLOW — ONLY this flag lints plugin #[cfg(test)] code; CI gate
cargo test --workspace      # SLOW — compiles every plugin binary; prefer `./scripts/check.sh test`
```

**Default verification = `scripts/check.sh`** (host + core + builtins + stable plugins from `plugins-manifest.json`; stable set is generated via `santui-dev-setup list-ids`). Full-workspace verification runs in CI on every push — you only need `--workspace` locally when you're working on an experimental plugin. lefthook pre-commit runs `cargo fmt --check` + `./scripts/check.sh clippy` automatically. Install hooks: `lefthook install`.

**Important**: `check.sh clippy` and the pre-commit hook lint non-test code only. Plugin `#[cfg(test)]` code (and any `#[cfg(test)]`-gated items like test modules) is compiled and linted **only** by CI's `cargo clippy --workspace --all-targets -- -D warnings`. When you edit a plugin's tests or reorder code around a `mod tests` block, run that exact command locally — a dropped `#[cfg(test)]` attribute or a bad test-only pattern compiles fine in the fast set but fails CI.

Run: `cargo build -p santui && cargo run -p santui` or `.\target\debug\santui.exe`

Server: `cargo run -p santui-server`

Dev mode (plugin registry + native deps):
  - Windows: `.\scripts\dev-setup.ps1 ; $env:SANTUI_DEV=1; cargo run -p santui`
  - macOS/Linux: `./scripts/dev-setup.sh && SANTUI_DEV=1 cargo run -p santui`

Fast dev (skip building plugins entirely — just run the app):
  - `./scripts/dev-setup.sh --no-build && SANTUI_DEV=1 cargo run -p santui`

Dev-setup now builds only the host binary + builtins (`santui`, `santui-dev-setup`, `santui-registry-plugin`) plus plugins with `"status": "stable"` in `plugins-manifest.json`. Other experimental plugins are compiled only when running `cargo build --workspace` explicitly.

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
├── plugins/           — first-party plugins (see plugins-manifest.json for full list)
│   ├── radio-stream-player/   — radio plugin
│   │   └── scraper/           — scrape radio stations into DB (--db-path, --prune, --help)
│   ├── registry/             — plugin registry UI plugin
│   └── ... (more plugin directories)
├── app/           — binary entry point (main.rs)
└── website/       — VitePress docs site
```

## Key Conventions

- Rust edition 2024, no nightly
- `ratatui` for rendering; `Theme` semantic colors over hardcoded `Color::*`
- `impl Default` for any type with a `new()` constructor
- `cargo fmt` before commit; clippy must pass with `-D warnings` (enforced by lefthook pre-commit)
- Commit messages must be in English
- **Refactoring / non-trivial changes**: work on a feature branch, push for review, then merge to `main`
- **Don't push on every commit** — only push when explicitly asked or when the branch is ready for review/merging
- **Semantic correctness**: before/after each edit, read the full surrounding function to ensure variable names, types, and logic still make sense. The compiler catches type errors but NOT wrong variable names (e.g. `name` vs `id`) or wrong control flow (e.g. `return` vs `continue`). Re-read the diff yourself before staging.
- **Verify claims against code**: never assert facts about the codebase (which plugins persist data, how many tests exist, what a feature does) from memory — memory drifts between sessions. Grep/read the code first, then answer. A confident-sounding but wrong claim (e.g. "radio doesn't persist favorites" when it does via `DbSet "favorites"`) wastes the user's time and erodes trust. If asked to compare features across crates/plugins, verify every side of the comparison.
- **Structural vs semantic filtering**: When filtering a collection (plugins, messages, events), prefer semantic criteria (e.g. "was installed via registry") over structural ones (e.g. "has a binary path"). Built-in plugins can share the same structure as registry-installed ones. A wrong filter compiles fine but causes subtle breakage at runtime (e.g. killing the registry plugin on startup). Add a dedicated tracking field rather than reusing an existing one for a different purpose.
- **No fragile solutions**: every approach must be solid, reliable, and performant. Avoid heuristics (hint-text comparisons, inferred state), silent race windows (timeout + fallback without guarding consumed), unbounded growth (height/width caps), and false positives (marking a plugin as crashed when the channel is merely full). Match on specific error variants rather than `.is_err()` catch-alls.
- **IPC `consumed` protocol**: `PluginMsg.consumed` must be set to `true` when a plugin handles a key event internally (e.g., closing a sub-dialog on Esc). The host uses this to decide whether to fall back to default handling (e.g., closing the plugin on Esc). Every key handler should return a `bool` consumed flag; do NOT rely on heuristics like hint text comparison.
- **plugins-manifest.json + Cargo.toml**: When adding a new plugin you MUST update **both**:
  1. `plugins-manifest.json` — add an entry with `id`, `name`, `description`, `capabilities`. Optionally add `"status": "stable"` for plugins ready to ship in releases (otherwise defaults to `"experimental"` in dev-setup and is excluded from release packaging). This is the source of truth for the registry (read by `dev-setup.sh` and CI `release.yml`). `plugins.json` (gitignored) is auto-generated.
  2. `Cargo.toml` (root workspace) — add `"crates/plugins/{id}"` to the `members` list. Without this, `cargo build` and `dev-setup.sh` will skip the plugin entirely (as happened with 31 orphaned plugins that had manifest entries but no workspace membership).
- **Never delete code unintentionally**: Every `edit` must preserve all existing lines, functions, and logic unless the user explicitly asked for removal. Before applying an edit, verify that `oldString` matches *only* the intended target and that `newString` includes everything that should remain — especially surrounding code, closing braces, and adjacent statements. When in doubt, prefer a more specific `oldString` with extra context lines to avoid matching the wrong block. After each edit, re-read the file to confirm nothing was silently dropped. A single missing brace or removed line can silently break the build and waste a debugging cycle.
- **Architectural skepticism**: If the AI struggles to fix a bug across multiple attempts (patch after patch, each adding complexity without solving it), step back and question the architecture itself. A fragile timing assumption or wrong abstraction is often the root cause — patching around it never works. The correct fix is to eliminate the assumption, not widen the window. No magic-number timeouts; no "should be fast enough" reasoning.
- **Dependency updates**: Use `cargo upgrade --incompatible allow` (from cargo-edit) to bump Cargo.toml version constraints to latest. Do NOT use `cargo outdated` — it is very slow. After upgrading, fix compilation errors in santui's own code only (not in third-party crates), then run `cargo check --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo fmt`, and `cargo test --workspace`.
- **`native/radio_stream_stations.db`** is the scraped radio station database. It is **not** a transient build artifact — it is bundled into releases (`release.yml`, `package-release-*` scripts) and copied to the user's data directory on first run. Always commit changes to this file (the `.gitignore` has `!native/radio_stream_stations.db` for this reason). Do NOT leave it unstaged.

## Website

```bash
cd website && npm run dev   # dev server
cd website && npm run build # static build
```

## Release

```bash
./scripts/release.sh x.y.z
```

Automates the whole flow: pre-flight check (every version-bearing file must be at the
current version), bump of all `Cargo.toml` + `packages/npm/package.json` +
website version strings + `plugins.json` (dev-mode only, never committed), leftover
verification, `cargo check` sanity, `git cliff` changelog (pre- and post-tag),
commit, tag, push, and `gh release create`. CI builds binaries and publishes to npm.

Manual fallback (if you prefer to do it by hand — everything the script does):

```bash
# Update version in all Cargo.toml files + packages/npm/package.json
# They must all match (script verifies; CI only checks tag vs crates/core/Cargo.toml)
#
# IMPORTANT: Every inter-crate path dependency must also have a version
# field matching the new version, e.g.:
#   santui-core = { path = "../core", version = "x.y.z" }
#
# Also update website version strings (grep for the old version — mind hidden dirs):
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
# CI builds binaries and publishes to npm.
# Then create the GitHub Release with only the new section from CHANGELOG.md:
git cliff -o CHANGELOG.md   # regenerate (now tag exists → [unreleased] becomes vx.y.z)
git add -A && git commit -m "chore: update changelog for vx.y.z" && git push
# Extract just the new section (between ## [vx.y.z] and next ## [)
VERSION="$(git describe --tags --abbrev=0 | sed 's/^v//')"
awk 'BEGIN{f=0} /^## \['"$VERSION"'\]/{f=1; next} /^## \[/{if(f) exit} f' CHANGELOG.md | \
  gh release create vx.y.z --notes-file - --title "vx.y.z"
```

Prerequisites:
- `NPM_TOKEN` secret set in GitHub repo Settings → Secrets → Actions

## Docs Index

- `docs/architecture.md` — architecture & IPC plugin model
- `docs/conventions.md` — coding conventions
- `docs/development.md` — tooling setup, pre-commit checks
