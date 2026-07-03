# Configuration

Santui stores its configuration in a **TOML** file. The file is created automatically on first launch with default values, but you can edit it to customise the theme or set persistent overrides.

## Location

| Platform | Path |
|----------|------|
| Linux / macOS | `~/.local/share/santui/config.toml` |
| Windows | `%APPDATA%\santui\config.toml` |

## Schema

```toml
# Theme to apply on startup. Must match a built-in theme name
# (e.g. "Nord", "Dracula", "Catppuccin"). Case-sensitive.
# If omitted or invalid, the default theme (Santui) is used.
theme = "Nord"

# Custom theme colour overrides. When present, these values
# take precedence over the selected theme's colours.
# Each field is an optional hex colour string.
[custom_theme]
name = "My Custom Theme"
accent = "#ff8800"
highlight = "#00ff88"
logo = "#ff3366"
text = "#e0e0e0"
text_muted = "#888888"
background = "#111111"
background_panel = "#1a1a1a"
background_overlay = "#0d0d0d"
border = "#333333"
success = "#44cc66"
error = "#ff4444"
inverted_text = "#222222"

# Key-binding overrides.
[keybindings]
# open_palette = "ctrl+p"
# quit = "q"
# about = "?"

# Plugin-specific settings (reserved for future use).
[plugins]
```

## Fields

### `theme` (optional)

A string matching any of the 38 built-in theme names. Switch themes at any time from the command palette (`Ctrl+P` → **Switch theme**). Setting it here makes the choice persist across restarts.

### `[custom_theme]` (optional)

Override individual colour keys. Any field not specified falls back to the selected theme's value. All colour values are hex strings with or without the `#` prefix (e.g. `"#ff8800"` or `"ff8800"`).

Available colour keys:

| Key | Usage |
|-----|-------|
| `name` | Display name in the theme picker |
| `accent` | Accent text, active item markers |
| `highlight` | Highlight bar, logo accent, borders |
| `logo` | Logo colour |
| `text` | Primary text colour |
| `text_muted` | Muted / secondary text |
| `background` | Main background |
| `background_panel` | Panel and dialog backgrounds |
| `background_overlay` | Overlay / dimmed background |
| `border` | Border colour |
| `success` | Success indicators |
| `error` | Error indicators |
| `inverted_text` | Text on dark backgrounds |

### `[keybindings]`

Override default key bindings. Each value is a string in the format `"ctrl+key"`, `"alt+key"`, or `"shift+key"` (or a bare key for unmodified binds). Supported fields:

| Field | Default | Description |
|-------|---------|-------------|
| `open_palette` | `"ctrl+p"` | Open the command palette |
| `quit` | `"q"` | Quit the application |
| `about` | `"?"` | Show the about screen |

### `[plugins]` (reserved)

Schema defined for future plugin-specific settings. Currently unused.

## Hot-reload

The configuration file is monitored for changes. Edit it with any text editor while Santui is running and the changes are applied automatically on the next frame — no restart required.
