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
  ThemeChange, Resize, Shutdown, UserUpdate, PaletteCommand, PluginMessage,
  DbValue)
- **stdout** — sends `PluginMsg` containing `commands` (RenderCmd list),
  `hints` (status-bar hints), `palette_commands` (Ctrl+P entries),
  `consumed` (whether a key was handled internally),
  an optional `request` (SignIn / SignOut / DbGet / DbSet / PluginsChanged / LaunchPlugin),
  and an optional `plugin_message` (plugin-to-plugin messaging)

See the `santui-ipc` crate documentation for the full protocol spec.

## Manifest

Add an entry for this plugin to your `plugins.json` manifest so the registry
can discover and install it:

```json
{
    "id": "{{project-name}}",
    "name": "{{project-name}}",
    "description": "A short description of your plugin",
    "publisher": "Your Name",
    "version": "0.1.0",
    "download_url": "target/debug/{{project-name}}.exe",
    "sha256": "",
    "size": 0,
    "capabilities": []
}
```
