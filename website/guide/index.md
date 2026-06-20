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

### macOS / Linux

**npm** (recommended) — works everywhere with no extra setup:

```bash
npm install -g santui
santui
```

**Install script** — downloads binary to `~/.local/share/santui/current` and adds it to your PATH:

```bash
curl -fsSL https://santuiapp.vercel.app/install.sh | sh
```

> **Linux users:** Install [libmpv](https://mpv.io/installation/) for the Radio Streaming Player:
> ```bash
> sudo apt install mpv      # Debian / Ubuntu / Pop!_OS
> sudo dnf install mpv      # Fedora
> sudo pacman -S mpv        # Arch
> ```

> **Note:** The npm method requires [Node.js](https://nodejs.org/) to be installed. No plugins included — install them from the Plugin Registry after launching.

## Uninstall

### Windows

| Method | Command |
|---|---|
| **npm** | `npm uninstall -g santui` |
| **PowerShell** | `irm https://santuiapp.vercel.app/uninstall.ps1 \| iex` |

Both remove Santui from your system. The npm method also removes the `santui` command from PATH automatically.

### macOS / Linux

| Method | Command |
|---|---|
| **npm** | `npm uninstall -g santui` |
| **install script** | `curl -fsSL https://santuiapp.vercel.app/uninstall.sh \| sh` |

Both remove Santui from `~/.local/share/santui` and clean up PATH entries.

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
