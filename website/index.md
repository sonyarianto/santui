---
layout: home

hero:
  name: Santui
  tagline: My terminal home base
  actions:
    - theme: brand
      text: Get Started
      link: /guide/
    - theme: alt
      text: View on GitHub
      link: https://github.com/sonyarianto/santui

features:
  - title: Internet Radio Player
    details: Browse stations by country, search, and stream internet radio via the built-in audio plugin with libmpv.
  - title: 38 Built-in Themes
    details: Switch between any OpenCode theme at runtime via the command palette. Live preview before you commit.
  - title: Command Palette
    details: Press Ctrl+P to search and execute commands — switch themes, reload plugins, and more without leaving the keyboard.
  - title: Plugin Architecture
    details: Plugins run as separate processes with JSON IPC. Hot-reloadable, crash-isolated, and easy to write in Rust.
---

## Install

### Windows
```powershell
irm https://santuiapp.vercel.app/install.ps1 | iex
```

### macOS
```bash
brew install mpv
git clone https://github.com/sonyarianto/santui
cd santui
cargo build --workspace && cargo run -p santui
```

### Linux
```bash
# Debian/Ubuntu
sudo apt install libmpv-dev

# Fedora
sudo dnf install mpv-libs-devel

git clone https://github.com/sonyarianto/santui
cd santui
cargo build --workspace && cargo run -p santui
```
