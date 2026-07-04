# Getting Started

## Installation

### Windows

**npm (recommended)** — no admin, no Windows Defender issues, works everywhere. After installing, type `santui` to launch:

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

**npm** (recommended) — works everywhere with no extra setup. After installing, type `santui` to launch:

```bash
npm install -g santui
santui
```

**Install script** — downloads binary to `~/.local/share/santui/current` and adds it to your PATH:

```bash
curl -fsSL https://santuiapp.vercel.app/install.sh | sh
```

> **macOS users:** Install [libmpv](https://mpv.io/installation/) for the Radio Stream Player:
> ```bash
> brew install mpv
> ```

### Linux

**npm** (recommended) — works everywhere with no extra setup. After installing, type `santui` to launch:

```bash
npm install -g santui
santui
```

**Install script** — downloads binary to `~/.local/share/santui/current` and adds it to your PATH:

```bash
curl -fsSL https://santuiapp.vercel.app/install.sh | sh
```

> **Linux users:** Install [libmpv](https://mpv.io/installation/) for the Radio Stream Player:
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

### CLI flags

Run `santui --help` for a full reference, or use these flags:

| Flag | Action |
|------|--------|
| `--version` / `-V` | Print version and exit |
| `--list-plugins` / `plugins` | List installed and available plugins, then exit |
| `reset` | Delete all data (config, plugins, database) and start fresh |

## Plugin Registry

Plugins extend Santui with new capabilities. Open `Ctrl+P` → **Plugin registry** to see what's available.

### Available now

| Plugin | What it does | Needs |
|---|---|---|
| **Radio Stream Player** | Browse stations by country, search, stream internet radio, **save favorites** | [libmpv](https://mpv.io/installation/) |
| **RSS Reader** | Subscribe to and read RSS/Atom feeds | — |
| **Clipboard History** | Track and search clipboard history | — |
| **System Monitor** | Monitor CPU, memory, disk, and network | — |
| **World Clock** | Timezone converter and world clock | — |
| **Weather** | Current conditions, hourly & 7-day forecast, location search, auto-refresh | — |
| **Plugin Registry** | Browse, install, and manage plugins | — |
| **Currency Converter** | Convert between currencies with live exchange rates | — |
| **Habit Tracker** | Track daily habits with streaks and heatmap visualization | — |
| **Hacker News Reader** | Browse top, new, and best stories from Hacker News | — |
| **HTTP Client** | Compose and send HTTP requests, view responses | — |
| **Music Preview** | Search and preview tracks from the iTunes catalog | — |
| **Pomodoro Timer** | Focus timer with work/break sessions and daily stats | — |
| **Quick Notes** | Lightweight scratch pad for capturing and searching notes | — |
| **SSH Manager** | Manage and connect to SSH hosts | — |

From the registry you can:

- **Browse** available plugins with descriptions and version info
- **Install** — download and set up a plugin with one Enter press
- **Enable / Disable** — toggle plugins on and off
- **Installed plugins** appear in your command palette under the **Modules** category

To get started, install a plugin from the registry — try **Radio Stream Player** for internet radio, **Hacker News Reader** for tech stories, **Weather** for forecasts, **System Monitor** for resource usage, or **World Clock** for timezone conversion. Then open `Ctrl+P` and select it to start using it.

> **Radio Stream Player** requires [libmpv](https://mpv.io/installation/) for audio playback. On Windows it's bundled in the release archive; on macOS/Linux install via `apt`/`brew`/`pacman`.

## Development

To build Santui from source and test plugins locally without a GitHub release:

### Prerequisites

- [Rust](https://rustup.rs/) 1.70+
- For the Radio Stream Player: [libmpv](https://mpv.io/installation/) (`apt install mpv`, `brew install mpv`, or bundled on Windows)

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
