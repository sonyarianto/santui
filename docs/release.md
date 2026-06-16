# Release Process

## 1. Bump versions

Update `version` in `Cargo.toml` for each workspace crate that changed:

- `santui-core/Cargo.toml`
- `santui-ipc/Cargo.toml`
- `santui-auth/Cargo.toml`
- `santui-radio-streaming-player/Cargo.toml`
- `santui-radio-streaming-player/scraper/Cargo.toml`
- `santui/Cargo.toml`

## 2. Build and package

```powershell
.\scripts\package-release.ps1 v0.x.x
```

Produces `releases/santui-x86_64-pc-windows-msvc.zip` with:

- `santui.exe`
- `santui-radio-streaming-player.exe`
- `native/libmpv-2.dll`
- `native/radio_streaming_stations.db`

## 3. Create GitHub Release

1. Go to https://github.com/sonyarianto/santui/releases/new
2. Tag: `v0.x.x`
3. Title: `v0.x.x`
4. Attach the zip from `releases/`
5. Publish

## 4. Deploy website

```bash
cd website
npm run build
npx vercel --prod
```

The install script at `https://santuiapp.vercel.app/install.ps1` fetches the latest release from GitHub automatically.
