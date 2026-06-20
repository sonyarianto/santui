# Getting Started

## Installation

### Windows

```powershell
irm https://santuiapp.vercel.app/install.ps1 | iex
```

This downloads the latest release, extracts it to `%LOCALAPPDATA%\santui\current`, and adds it to your PATH. Then run `santui` from any terminal.

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

## Themes

Press `Ctrl+P`, select **Switch theme**, and browse 38 OpenCode themes with live preview.
