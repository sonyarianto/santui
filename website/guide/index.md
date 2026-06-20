# Getting Started

## Installation

### Windows

**npm (recommended)** — no admin, no Windows Defender issues, works everywhere:

```bash
npm install -g santui
santui
```

**PowerShell** — ⚠️ Windows may block the downloaded binary:

```powershell
irm https://santuiapp.vercel.app/install.ps1 | iex
```

This extracts to `%LOCALAPPDATA%\santui\current` and adds to your PATH.

### macOS

```bash
curl -fsSL https://santuiapp.vercel.app/install.sh | sh
```

### Linux

```bash
# Install libmpv first
sudo apt install mpv      # Debian / Ubuntu / Pop!_OS
sudo dnf install mpv      # Fedora
sudo pacman -S mpv        # Arch

curl -fsSL https://santuiapp.vercel.app/install.sh | sh
```

Downloads the latest release to `~/.local/share/santui/current` and adds it to your PATH. libmpv must be installed via your package manager — it's not bundled in the archive.

## Uninstall

### Windows

```powershell
irm https://santuiapp.vercel.app/uninstall.ps1 | iex
```

Removes the installation folder from `%LOCALAPPDATA%\santui` and cleans up the User PATH.

### macOS / Linux

```bash
curl -fsSL https://santuiapp.vercel.app/uninstall.sh | sh
```

Removes `~/.local/share/santui` and removes the PATH entry from `.bashrc` / `.zshrc`.

## Usage

Santui is keyboard-driven. Here are the keybindings:

| Key | Action |
|-----|--------|
| `?` | About screen |
| `Ctrl+P` | Command palette |
| `↑` / `↓` | Navigate lists |
| `Enter` | Select item |
| `Esc` | Back / close panel |

## Plugin Registry

Plugins extend Santui with new capabilities. Open `Ctrl+P` → **Plugin registry** to see what's available.

From the registry you can:

- **Browse** available plugins with descriptions and version info
- **Install** — download and set up a plugin with one Enter press
- **Enable / Disable** — toggle plugins on and off
- **Installed plugins** appear in your command palette under the **Modules** category

To get started, install the **Radio Streaming Player** plugin from the registry, enable it, then open `Ctrl+P` and select it to start browsing thousands of radio stations.

> **Radio Streaming Player** requires [libmpv](https://mpv.io/installation/) for audio playback. On Windows it's bundled in the release archive; on macOS/Linux install via `apt`/`brew`/`pacman`.

## Development

To build Santui from source and test plugins locally without a GitHub release:

### Prerequisites

- [Rust](https://rustup.rs/) 1.70+
- For the Radio Streaming Player: [libmpv](https://mpv.io/installation/) (`apt install mpv`, `brew install mpv`, or bundled on Windows)

### Build & run

```bash
git clone https://github.com/sonyarianto/santui
cd santui
cargo build --workspace && cargo run -p santui
```

### Dev mode (test plugin registry locally)

By default, the Plugin Registry fetches manifests from GitHub Releases. In development, use **dev mode** to test the full install flow without publishing anything:

**Windows (PowerShell):**
```powershell
.\scripts\dev-setup.ps1 ; $env:SANTUI_DEV=1; cargo run -p santui
```

**macOS / Linux:**
```bash
./scripts/dev-setup.sh && SANTUI_DEV=1 cargo run -p santui
```

What `dev-setup` does:
1. Builds the workspace (`cargo build --workspace`)
2. Copies native assets into `target/debug/native/`
3. Scans for plugin binaries and generates `plugins.json` with real SHA-256 hashes

When `SANTUI_DEV=1`, the app:
- Loads plugins from the local `plugins.json` instead of GitHub
- Copies binaries from your build directory instead of downloading
- Shows a `[DEV]` badge in the registry UI so you know you're in dev mode

> See [`docs/development.md`](https://github.com/sonyarianto/santui/blob/main/docs/development.md) for detailed tooling info.

## Themes

Press `Ctrl+P`, select **Switch theme**, and browse 38 OpenCode themes with live preview.
