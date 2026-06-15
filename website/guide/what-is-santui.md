# What is Santui?

Santui is a **modular terminal user interface framework** for Rust, built on top of [Ratatui](https://ratatui.rs).

It provides a lightweight architecture for building interactive TUI applications with:

- **Widget-based composition** — each widget is self-contained with its own state, event handling, and rendering
- **State machine routing** — app modes and transitions are explicit and easy to follow
- **Event-driven** — keyboard input, resize events, and custom events flow through a channel
- **Hot reload** — widgets can be reloaded at runtime without restarting the app

Santui is not a batteries-included framework. It gives you the skeleton — you bring the UI components, business logic, and styling.
