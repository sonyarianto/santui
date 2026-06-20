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

**npm** (recommended) — no admin, no Windows Defender issues, works everywhere:
```bash
npm install -g santui
santui
```

**PowerShell** — ⚠️ Windows may block the downloaded binary:
```powershell
irm https://santuiapp.vercel.app/install.ps1 | iex
```

### macOS / Linux

**npm** (recommended) — works everywhere, no platform-specific setup:
```bash
npm install -g santui
santui
```

**Install script** — downloads binary to `~/.local/share/santui/current`:
```bash
curl -fsSL https://santuiapp.vercel.app/install.sh | sh
```

> **Linux:** Install [libmpv](https://mpv.io/installation/) for the Radio Streaming Player:
> `sudo apt install mpv` / `sudo dnf install mpv` / `sudo pacman -S mpv`
>
> **Prerequisite:** The npm method requires [Node.js](https://nodejs.org/).
