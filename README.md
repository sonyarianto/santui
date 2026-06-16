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

Requires Rust 1.70+ and [libmpv](https://mpv.io/installation/).

## Documentation

Full docs at [santuiapp.vercel.app](https://santuiapp.vercel.app).
