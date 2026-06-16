---
layout: home

hero:
  name: Santui
  text: Modular TUI platform
  tagline: TUI app for your daily companion.
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
