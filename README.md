# Santui

My terminal home base.

## Features

- **Internet Radio Player** — browse stations by country, search, and stream via libmpv
- **38 Built-in Themes** — switch at runtime via the command palette with live preview
- **Command Palette** — `Ctrl+P` to search and execute commands
- **Plugin Architecture** — headless plugin binaries with JSON IPC, crash-isolated

## Quick Start

```bash
git clone https://github.com/sonyarianto/santui
cd santui
cargo build --workspace && cargo run -p santui
```

Requires Rust 1.70+. The radio streaming player plugin requires [libmpv](https://mpv.io/installation/) (bundled in Windows release archives; install via `apt`/`brew`/`scoop` on other platforms).

## Documentation

Full docs at [santuiapp.vercel.app](https://santuiapp.vercel.app).
