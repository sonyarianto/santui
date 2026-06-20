# Development Tooling

## Pre-commit hooks (lefthook)

Uses [lefthook](https://github.com/evilmartians/lefthook) — config in `lefthook.yml`.

Checks run in parallel on every commit touching `*.rs` files:

- `cargo fmt --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo check --workspace`

If lefthook is not installed: `cargo install lefthook` or `npm i -g @evilmartians/lefthook`.

## Development Mode (Plugin Registry)

When building plugins, dev mode lets you test installation and native dependencies without GitHub Releases.

### Quick start

```powershell
.\scripts\dev-setup.ps1 ; $env:SANTUI_DEV=1; cargo run -p santui
```

### What dev-setup.ps1 does

1. Builds the workspace (`cargo build --workspace`)
2. Copies native assets from `native/` (e.g. `libmpv-2.dll`) into `target/debug/native/`
3. Scans `target/debug/` for plugin binaries and generates `plugins.json` with real SHA-256 hashes and file sizes

### How dev mode works

When `SANTUI_DEV=1` is set, the app:
- Loads the manifest from `plugins.json` in the current directory (overridable via `SANTUI_DEV_MANIFEST`)
- Copies plugin binaries and native dependencies locally instead of downloading from GitHub
- Shows a `[DEV]` prefix in the registry UI status

Native dependencies are synced from a `native/` folder adjacent to each plugin binary into `~/.santui/plugins/native/` on install and on every registry open.

## Release packaging

Platform-specific scripts in `scripts/`:

| Script | Platform | Format |
|--------|----------|-------|
| `package-release.ps1` | Windows | `.zip` |
| `package-release-macos.sh` | macOS | `.tar.gz` |

Run from the repo root:

```bash
# macOS (requires Homebrew + mpv installed)
./scripts/package-release-macos.sh [version]

# Windows (requires PowerShell)
./scripts/package-release.ps1 [version]
```

The macOS script recursively bundles all transitive Homebrew dylib
 dependencies (`libmpv.2.dylib`, `libavcodec`, etc.) into `native/`
 and rewrites their `LC_LOAD_DYLIB` paths to `@loader_path`-relative
 via `install_name_tool`, making the archive relocatable to machines
 without Homebrew.
