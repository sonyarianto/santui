# What is Santui?

Santui is **my terminal home base** — a daily companion TUI app that lives in your terminal.

It's not a framework or library for building other apps. It's the app itself, designed to be:

- **Extensible** — plugins run as separate processes and communicate over JSON IPC. Write a plugin in Rust, register it, and it becomes part of Santui.
- **Themeable** — 38 OpenCode themes you can switch at runtime with live preview.
- **Keyboard-driven** — everything is a key press away, from the command palette to navigation.

## Current features

- **Internet Radio Player** — browse radio stations by country, search by name or genre, and stream audio via libmpv.
- **Command Palette** — `Ctrl+P` to search commands: switch themes, reload plugins, and more.
- **Themes** — 38 built-in themes with instant switching and live preview.
- **Plugin System** — headless plugin binaries that Santui spawns and manages. Crash-isolated and hot-reloadable.
