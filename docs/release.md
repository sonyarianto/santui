# Release Process

## 1. Bump versions

Update `version` in **all** `Cargo.toml` files + npm + website. They must all match — CI verifies against `crates/core/Cargo.toml`:

**Crates (every `Cargo.toml` under `crates/` that has a `version` field — enumerate with `rg -l '^version = ' crates --glob 'Cargo.toml'`):**

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
3. Builds a Windows Inno Setup installer (`santui-setup.exe`) via `scripts/winget/santui.iss`
4. Uploads all binaries + installer + manifests as release assets
5. Creates a GitHub Release with auto-generated release notes

The install script at `https://santuiapp.vercel.app/install.ps1` fetches the latest release from GitHub automatically.

## 4. Winget

### First-time submission (manual — required once)

The `winget-releaser` action needs an existing version of the package in
`microsoft/winget-pkgs` as a base manifest, so the **first** release must be
submitted manually. After that, subsequent releases are automated.

1. Push a release tag; the `release.yml` workflow builds `santui-setup.exe`
   (Inno Setup) and attaches it to the GitHub Release.
2. On a machine with `wingetcreate` installed:
   ```powershell
   wingetcreate update sonyarianto.Santui -u https://github.com/sonyarianto/santui/releases/download/v0.2.40/santui-setup.exe -v 0.2.40
   ```
3. This creates the manifest locally; review it, then submit a PR to
   `microsoft/winget-pkgs` (or let `wingetcreate` submit it with `--submit`).

### Automatic submissions (subsequent releases)

After the first version lands in winget-pkgs, the `winget.yml` workflow runs on
every **published** release and submits `sonyarianto.Santui` automatically
(creates a PR to `microsoft/winget-pkgs`).

Requires the `WINGET_TOKEN` secret in GitHub repo settings — a *classic* PAT
with `public_repo` scope (fine-grained PATs are NOT supported by the action).
The account owning the token must have a fork of `microsoft/winget-pkgs`
(created automatically if the account has that repo forked).

## 5. Deploy website

```bash
cd website
npm run build
npx vercel --prod
```
