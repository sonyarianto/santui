# Santui

Your terminal home base.

Santui is a keyboard-driven TUI app that lives in your terminal. Think of it as a **launcher for plugins** — the core is lightweight, and everything extra comes through the Plugin Registry.

## What's built in

Out of the box, Santui gives you:

- **Command Palette** — `Ctrl+P` to search and run commands
- **38 Themes** — switch anytime with live preview
- **Plugin Registry** — browse, install, and manage plugins from inside the app
- **Auth** — sign in with Google or GitHub

That's it. No bloat. You add what you need.

## Plugins

Plugins are **standalone binaries** distributed through the Plugin Registry. Open `Ctrl+P` → **Plugin registry**, install what you want, enable it, and it appears in your palette — ready to use.

### Available now

| Plugin | What it does | Needs |
|---|---|---|
| **Radio Streaming Player** | Browse 50,000+ stations by country, search, stream | [libmpv](https://mpv.io/installation/) |

*More coming. Want to build one? See [docs/architecture.md](docs/architecture.md).*

## Quick start

### Windows (Scoop)

```powershell
scoop bucket add santui https://github.com/sonyarianto/scoop-santui
scoop install santui
santui
```

### From source

```bash
git clone https://github.com/sonyarianto/santui
cd santui
cargo build --workspace && cargo run -p santui
```

Requires Rust 1.70+. No plugins included — install them from the Plugin Registry after launching.

### Dev mode (testing plugins locally)

```bash
# Windows
.\scripts\dev-setup.ps1 ; $env:SANTUI_DEV=1; cargo run -p santui

# macOS / Linux
./scripts/dev-setup.sh && SANTUI_DEV=1 cargo run -p santui
```

This builds everything, generates a local plugin manifest, and runs Santui in dev mode — identical flow to production, no release needed.

## Documentation

Full docs at [santuiapp.vercel.app](https://santuiapp.vercel.app).
