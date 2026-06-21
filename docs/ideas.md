# Ideas

## Home screen plugin widget + scrollability

**Status:** idea

Show a panel of currently installed plugins on the home screen (splash), below the logo.

### Why

The home screen is currently static — ASCII logo + tagline + version. Installed plugins are only accessible via the command palette (`Ctrl+P`). A dedicated panel would make the launcher nature of Santui visible immediately: see what's installed, which are running, and get a quick shortcut hint.

### How

**Current splash layout** (`screens.rs:99-106`):

```
Fill(1)     ← top spacer
Length(8)   ← logo + tagline + version
Fill(1)     ← bottom spacer
```

**Proposed layout:**

```
Fill(1)           ← top spacer
Length(8)         ← logo + tagline + version
Length(N)         ← "Plugins" panel (List widget, scrollable)
Fill(1)           ← bottom spacer
```

**Plugin panel** — a ratatui `List` or `Paragraph` iterating over installed plugins:

| Column | Source |
|--------|--------|
| name | `InstalledPlugin::name` or humanized `id` |
| status | "running" / "stopped" — check `PluginManager::find_by_id()` |
| shortcut | e.g. `1`, `2`, `3` — future hotkey to activate directly |

The data already exists:
- `PluginManager::dynamic_items()` → enabled installed plugins
- `PluginManager::plugins` → currently loaded/running
- `Registry::installed` → full list with `enabled`, `version`, `path`, `id`, `name`

### Scrollability

If the terminal is too short or too many plugins are installed, the panel overflows. Ratatui `Paragraph`/`List` supports `.scroll((y, 0))`. Same pattern as the palette, theme picker, and registry overlays.

Two possible scroll models:

1. **Panel-scroll (recommended start):** Only the plugin list scrolls within its `Length(N)` slot. Logo stays fixed. Simple, same pattern as overlays.

2. **Full-page scroll:** The entire home screen (logo + plugin list) scrolls as one unit. Would need to wrap all content into a single scrollable widget. More refactoring, but useful if we later add more sections below plugins.

### Data flow

```
Registry::installed
       │
       ▼
PluginManager::refresh_dynamic_items()  ← called at startup + on registry change
       │
       ▼
Home screen render reads PluginManager for:
  - list of enabled installed plugins (name + id)
  - which are currently loaded (find_by_id)
  - renders as a scrollable list
```

### Open questions

- Hotkey shortcuts to activate a plugin directly from the home screen? (e.g. `1`–`9`)
- Show enabled + disabled plugins, or only enabled?
- Show plugin version? Description?
- Auto-scroll to newly installed plugin after registry install?
