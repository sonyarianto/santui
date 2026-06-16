# Getting Started

## Installation

### Windows

```powershell
irm https://santuiapp.vercel.app/install.ps1 | iex
```

This downloads the latest release, extracts it to `%LOCALAPPDATA%\santui\current`, and adds it to your PATH. Then run `santui` from any terminal.

### macOS / Linux

```bash
curl -fsSL https://santuiapp.vercel.app/install.sh | sh
```

Installs mpv (macOS via Homebrew, checks for libmpv on Linux), downloads the latest release to `~/.local/share/santui/current`, and adds it to your PATH. Run `santui` from any terminal.

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
