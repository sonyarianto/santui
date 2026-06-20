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
| `Enter` | Select / play station |
| `Esc` | Back / close panel |

## Radio Player

Browse and stream internet radio stations:

- **Browse** — filtered by genre or country
- **Search** — type to filter by name, country, or genre
- **Play** — press Enter to start streaming
- **Volume** — `+` / `-` to adjust
- **Reload** — press `r` to reload stations from the database

Stations are pre-populated in the bundled database — ready to use out of the box.

## Updating Stations

To refresh or expand the station database, run the scraper:

```bash
cargo run -p santui-radio-streaming-scraper
```

This fetches currently-playing stations from onlineradiobox.com and inserts them into the local SQLite database at `%APPDATA%\santui\radio_streaming_stations.db`.

## Themes

Press `Ctrl+P`, select **Switch theme**, and browse 38 OpenCode themes with live preview.
