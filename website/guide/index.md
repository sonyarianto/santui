# Getting Started

## Prerequisites

- Rust 1.70+
- [libmpv](https://mpv.io/installation/) (for the radio player plugin)
- A terminal that supports Ratatui (most modern terminals do)

## Installation

### From source

```bash
git clone https://github.com/sonyarianto/santui
cd santui
cargo build --workspace && cargo run -p santui
```

Or install directly:

```bash
cargo install --git https://github.com/sonyarianto/santui
```

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

The radio player plugin lets you browse and stream internet radio stations:

- **Browse by country** — stations are grouped by country
- **Search** — type to filter stations by name
- **Play** — press Enter to start streaming

Press `r` in the radio player to reload stations from the database.

## Scraping Stations

Santui includes a scraper utility to populate the radio station database:

```bash
cargo run -p santui-radio-streaming-scraper
```

This fetches currently-playing stations from onlineradiobox.com for every country and inserts them into the local SQLite database.

## Themes

Press `Ctrl+P`, select **Switch theme**, and browse 38 OpenCode themes with live preview.
