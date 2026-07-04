# Development Tooling

## Pre-commit hooks (lefthook)

Uses [lefthook](https://github.com/evilmartians/lefthook) — config in `lefthook.yml`.

Checks run in parallel on every commit touching `*.rs` files:

- `cargo fmt --check`
- `cargo clippy --workspace -- -D warnings`

If lefthook is not installed: `cargo install lefthook` or `npm i -g @evilmartians/lefthook`.

## Development Mode (Plugin Registry)

When building plugins, dev mode lets you test installation and native dependencies without GitHub Releases.

### Quick start

```powershell
.\scripts\dev-setup.ps1 ; $env:SANTUI_DEV=1; cargo run -p santui
```

```bash
./scripts/dev-setup.sh && SANTUI_DEV=1 cargo run -p santui
```

### What dev-setup does

1. Builds the workspace (`cargo build --workspace`)
2. Copies native assets from `native/` (e.g. `libmpv-2.dll`) into `target/debug/native/`
3. Scans `target/debug/` for plugin binaries and generates `plugins.json` with real SHA-256 hashes, file sizes, and declared capabilities (e.g. `"capabilities": ["background"]`)

### How dev mode works

When `SANTUI_DEV=1` is set, the app:
- Loads the manifest from `plugins.json` in the current directory (overridable via `SANTUI_DEV_MANIFEST`)
- Copies plugin binaries and native dependencies locally instead of downloading from GitHub
- Shows a `[DEV]` prefix in the registry UI status

Native dependencies are synced from a `native/` folder adjacent to each plugin binary into `~/.santui/plugins/native/` on install and on every registry open.

### Project structure

```
santui/
├── crates/
│   ├── core/           — framework: App, Plugin trait, event loop, palette
│   ├── ipc/            — IPC protocol types + IpcPluginHost runner
│   ├── auth/           — GitHub OAuth client
│   ├── registry/       — plugin registry: manifest fetch, install, config
│   ├── db/             — central SQLite database for per-user plugin data
│   ├── plugins/
│   │   ├── radio-stream-player/   — radio player plugin
│   │   │   └── scraper/              — radio station scraper
│   │   ├── registry/                 — plugin registry UI plugin
│   │   ├── rss-reader/               — RSS feed reader
│   │   ├── clipboard-history/        — clipboard history manager
│   │   ├── system-monitor/           — system resource monitor
│   │   ├── world-clock/              — world clock / timezone converter
│   │   ├── weather/                  — weather forecaster
│   │   ├── currency-converter/       — real-time currency converter
│   │   ├── habit-tracker/            — habit tracker with heatmaps
│   │   ├── hackernews-reader/        — Hacker News story reader
│   │   ├── http-client/              — HTTP request composer
│   │   ├── music-preview/            — iTunes track search and preview
│   │   ├── pomodoro-timer/           — focus timer
│   │   ├── quick-notes/              — scratch pad and note manager
│   │   └── ssh-manager/              — SSH connection manager
│   └── app/            — binary entry point (main.rs)
├── website/            — VitePress docs site
├── docs/               — architecture & dev docs
├── scripts/            — dev setup & release packaging
├── native/             — runtime native dependencies (mpv DLLs, station DB)
└── Cargo.toml          — workspace root
```

## CLI flags

The built binary accepts the following flags:

| Flag | Action |
|------|--------|
| `--version` / `-V` | Print version (`santui vX.Y.Z`) and exit |
| `--list-plugins` / `plugins` | List installed and available plugins, then exit |
| `reset` | Delete all data (config, plugins, database) and start fresh |

## Release packaging

For tagged releases, CI (`.github/workflows/release.yml`) handles cross-platform builds automatically.
Platform-specific packaging scripts in `scripts/` are available for manual testing:

| Script | Platform | Format |
|--------|----------|-------|
| `package-release.ps1` | Windows | `.zip` |
| `package-release-macos.sh` | macOS | `.tar.gz` |

The macOS script recursively bundles all transitive Homebrew dylib
 dependencies (`libmpv.2.dylib`, `libavcodec`, etc.) into `native/`
 and rewrites their `LC_LOAD_DYLIB` paths to `@loader_path`-relative
 via `install_name_tool`, making the archive relocatable to machines
 without Homebrew.
