# {{project-name}}

A Santui IPC plugin, scaffolded from the official plugin template.

## Quick Start

```bash
cargo build --release
```

Copy the resulting binary to `~/.santui/plugins/{{project-name}}` (or the
Santui plugins directory for your platform), then launch Santui and select
the plugin from the command palette (`Ctrl+P`).

## Development

For local development, run Santui from the workspace root with `SANTUI_DEV=1`
(set in `scripts/dev-setup.ps1` or `scripts/dev-setup.sh`). This tells Santui
to use the local build directory as the plugin source.

## Protocol

Your plugin communicates with the Santui host over JSON lines on stdin/stdout:

- **stdin** — receives `HostMsg` variants (Init, Key, Tick, Focus, Blur,
  ThemeChange, Resize, Shutdown, UserUpdate, PaletteCommand, PluginMessage)
- **stdout** — sends `PluginMsg` containing `commands` (RenderCmd list),
  `hints` (status-bar hints), `palette_commands` (Ctrl+P entries), and
  an optional `request` (SignIn / SignOut)

See the `santui-ipc` crate documentation for the full protocol spec.

## Manifest

Add an entry for this plugin to your `plugins.json` manifest so the registry
can discover and install it:

```json
{
    "id": "{{project-name}}",
    "name": "{{project-name}}",
    "size": 0,
    "sha256": "",
    "download_url": "target/debug/{{project-name}}.exe",
    "version": "0.1.0"
}
```
