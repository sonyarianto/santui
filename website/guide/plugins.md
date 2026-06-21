# Plugin Development Guide

Plugins extend Santui with new capabilities. They run as **separate processes** and communicate with the host over JSON lines on stdin/stdout — no ratatui dependency, no TUI rendering, just a state machine that outputs render commands.

## How it works

```
santui.exe (host)
  └─ IpcPluginHost (implements Plugin trait)
       ├─ sends HostMsg (Init, Key, Tick, Resize, ...) via plugin's stdin
       └─ reads PluginMsg { commands, hints, request } from plugin's stdout
```

- The host owns all rendering. Your plugin returns a list of `RenderCmd` values — `Text` at (x,y) with colours, or `Clear` a region.
- The host polls your plugin every frame via `Tick`. Response handling is non-blocking — the host never waits on your plugin.
- Blocking calls (`Init`, `Key`, `Resize`, `ThemeChange`) have a 5-second timeout. If your plugin doesn't respond, the host continues without it.

## Quick start

### Prerequisites

- [Rust](https://rustup.rs/) 1.70+
- [cargo-generate](https://github.com/cargo-generate/cargo-generate):
  ```bash
  cargo install cargo-generate
  ```
- Santui source (for the template):

### Scaffold a new plugin

```bash
cargo generate --path /path/to/santui/templates/plugin --name my-plugin
cd my-plugin
```

This creates:

```
my-plugin/
├── Cargo.toml          # depends on santui-ipc + serde
├── README.md           # basic instructions (customise me!)
└── src/
    └── main.rs         # plugin entry point with full IPC loop
```

The generated `main.rs` handles every `HostMsg` variant out of the box. Your job is to fill in the `PluginState` struct with your own data, implement `handle_tick`, `handle_key`, and `render`.

### Build and test locally

1. Build your plugin:

   ```bash
   cargo build --release
   ```

2. Run Santui in **dev mode** so it picks up the local binary:

   ```bash
   # From the Santui workspace root:
   cd /path/to/santui
   .\scripts\dev-setup.ps1   # Windows
   # or
   ./scripts/dev-setup.sh    # macOS / Linux

   $env:SANTUI_DEV=1; cargo run -p santui
   ```

3. Install your plugin from the Plugin Registry (`Ctrl+P` → **Plugin registry**). In dev mode, the registry reads from your local build directory, so your plugin binary appears automatically.

4. Enable it from the registry, then select it from the command palette.

## Understanding the template

### PluginState

The state is created once on `Init` and lives for the plugin's lifetime:

```rust
struct PluginState {
    theme: ThemeData,
    area: Area,
    counter: u64,
}
```

Add your own fields here. The `Init` message provides the current theme and terminal area so you can set up your initial state.

### The event loop

`main()` reads `HostMsg` values from stdin in a loop and dispatches them:

| Message | When it arrives |
|---------|----------------|
| `Init` | Plugin is loaded — create your state |
| `Tick` | Every frame (~60 times/second) — update animations |
| `Key` | User pressed a key while this plugin was focused |
| `Focus` / `Blur` | Plugin gained / lost focus |
| `ThemeChange` | User switched themes |
| `Resize` | Terminal was resized |
| `UserUpdate` | User signed in or out |
| `PaletteCommand` | User selected a palette command from `palette_commands` |
| `PluginMessage` | Another plugin sent a message |
| `Shutdown` | Santui is closing — exit cleanly |

### Responding

Every message expects at least one response on stdout. The template's `respond()` method builds a `PluginMsg`:

```rust
fn respond(&self) {
    let msg = PluginMsg {
        commands: self.render(),
        hints: vec![],
        palette_commands: vec![],
        request: None,
    };
    let json = serde_json::to_string(&msg).expect("serialise PluginMsg");
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{json}");
    let _ = out.flush();
}
```

#### PluginMsg fields

| Field | Type | Purpose |
|-------|------|---------|
| `commands` | `Vec<RenderCmd>` | Things to draw on screen this frame |
| `hints` | `Vec<(String, String)>` | Status bar hints (label, description) |
| `palette_commands` | `Vec<(String, String)>` | Commands shown in `Ctrl+P` palette |
| `request` | `Option<PluginRequest>` | Request auth (`SignIn` / `SignOut`) |

### Render commands

| Command | Purpose |
|---------|---------|
| `Text { x, y, text, fg, bg, bold }` | Draw a string at (x,y) with optional colours and bold |
| `Clear { x, y, w, h }` | Clear a rectangular region |

Colours are `[u8; 3]` RGB arrays (e.g. `[255, 0, 0]` for red). Use `None` for `fg`/`bg` to inherit the terminal default.

The `ThemeData` struct from `Init` and `ThemeChange` provides the current theme colours as RGB arrays. Use them to match Santui's look:

```rust
RenderCmd::Text {
    x: 1,
    y: 3,
    text: "Hello".into(),
    fg: Some(self.theme.accent),
    bg: Some(self.theme.background_panel),
    bold: false,
}
```

## Adding palette commands

Return palette entries and your plugin will appear in `Ctrl+P`:

```rust
fn respond(&self) {
    let msg = PluginMsg {
        commands: self.render(),
        hints: vec![],
        palette_commands: vec![
            ("Do something".into(), "my-plugin-do-something".into()),
            ("Reset".into(),       "my-plugin-reset".into()),
        ],
        request: None,
    };
    // ...serialise and send
}
```

When the user selects one, `HostMsg::PaletteCommand { index }` is sent with the index into your `palette_commands` vector.

## Requesting authentication

Set `request` to `Some(PluginRequest::SignIn { provider })` or `Some(PluginRequest::SignOut)`:

```rust
PluginMsg {
    commands: vec![],
    hints: vec![],
    palette_commands: vec![],
    request: Some(PluginRequest::SignIn {
        provider: "github".into(),
    }),
}
```

When the user completes the flow, `HostMsg::UserUpdate { user: Some(...) }` is sent with the user data.

## Status bar hints

Return hints to show key bindings in the status bar:

```rust
hints: vec![
    ("j/k".into(), "navigate".into()),
    ("Enter".into(), "select".into()),
],
```

> **Radio Streaming Player** requires [libmpv](https://mpv.io/installation/) for audio playback. On Windows it's bundled in the release archive; on macOS/Linux install via `apt`/`brew`/`pacman`.

## Next steps

- Browse the [IPC protocol source](https://github.com/sonyarianto/santui/blob/main/crates/ipc/src/protocol.rs) for the full type definitions
- See the [Radio Streaming Player](https://github.com/sonyarianto/santui/tree/main/crates/plugins/radio-streaming-player) for a production plugin example
- Read [`docs/architecture.md`](https://github.com/sonyarianto/santui/blob/main/docs/architecture.md) for the plugin architecture overview
