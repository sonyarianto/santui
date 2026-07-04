# Release Process

## 1. Bump versions

Update `version` in **all** `Cargo.toml` files + npm + website. They must all match — CI verifies against `crates/core/Cargo.toml`:

**Crates (all `Cargo.toml` files):**
- `crates/core/Cargo.toml`
- `crates/ipc/Cargo.toml`
- `crates/auth/Cargo.toml`
- `crates/registry/Cargo.toml`
- `crates/db/Cargo.toml`
- `crates/app/Cargo.toml`
- `crates/plugins/radio-stream-player/Cargo.toml`
- `crates/plugins/radio-stream-player/scraper/Cargo.toml`
- `crates/plugins/registry/Cargo.toml`
- `crates/plugins/rss-reader/Cargo.toml`
- `crates/plugins/clipboard-history/Cargo.toml`
- `crates/plugins/system-monitor/Cargo.toml`
- `crates/plugins/world-clock/Cargo.toml`
- `crates/plugins/weather/Cargo.toml`
- `crates/plugins/currency-converter/Cargo.toml`
- `crates/plugins/habit-tracker/Cargo.toml`
- `crates/plugins/hackernews-reader/Cargo.toml`
- `crates/plugins/http-client/Cargo.toml`
- `crates/plugins/music-preview/Cargo.toml`
- `crates/plugins/pomodoro-timer/Cargo.toml`
- `crates/plugins/quick-notes/Cargo.toml`
- `crates/plugins/ssh-manager/Cargo.toml`

**NPM:**
- `packages/npm/package.json`

**Website:**
- `website/.vitepress/config.ts` — nav link + footer
- `website/public/install.ps1` — banner text
- `website/index.md` — tagline (if changed)

**Dev manifest:**
- `plugins.json` — update the `version` field for dev-mode (gitignored — see `AGENTS.md`)

**Dev-setup scripts** auto-detect version from `crates/core/Cargo.toml` — no manual update needed.

## 2. Generate changelog

```bash
git cliff -o CHANGELOG.md
```

Inspect `CHANGELOG.md`, commit any adjustments, then:

## 3. Tag and push

```bash
git add -A && git commit -m "chore: bump version to x.y.z"
git tag vx.y.z && git push --tags
```

## 3. CI does the rest

Pushing a tag triggers the **CI release workflow** (`.github/workflows/release.yml`):

1. Builds all crates in release mode for Windows, macOS, and Linux
2. Generates platform-specific plugin manifests (`plugins-{target}.json`) with SHA-256 hashes
3. Uploads all binaries + manifests as release assets
4. Creates a GitHub Release with auto-generated release notes

The install script at `https://santuiapp.vercel.app/install.ps1` fetches the latest release from GitHub automatically.

## 4. Deploy website

```bash
cd website
npm run build
npx vercel --prod
```
