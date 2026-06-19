# Development Tooling

## Pre-commit hooks (lefthook)

Uses [lefthook](https://github.com/evilmartians/lefthook) — config in `lefthook.yml`.

Checks run in parallel on every commit touching `*.rs` files:

- `cargo fmt --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo check --workspace`

If lefthook is not installed: `cargo install lefthook` or `npm i -g @evilmartians/lefthook`.

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
