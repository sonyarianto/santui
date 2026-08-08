# Santui Conventions

- Rust edition 2024, no nightly features
- Use `ratatui` for all terminal rendering (no direct terminal writes except crossterm for raw mode)
- Use `Color::Rgb(r, g, b)` for custom colors
- All widgets use ratatui's `Frame`, `Layout`, `Rect`, `Style`, `Span`, `Line`, `Paragraph`
- Use `Theme` semantic colors instead of hardcoded `Color::*` — add new fields to `Theme` if needed
- Add `impl Default` for any type with a `new()` constructor (clippy rule)
- `cargo fmt` before commit; clippy must pass with `-D warnings` (enforced by lefthook pre-commit)

## Palette & hint footers

The standard palette footer style is `↑↓ navigate • ↵ select`:

- Keys render in `theme.text`, descriptions and ` • ` separators in `theme.text_muted`
- Separator between pairs is a single ` • ` (muted); the last pair has no trailing separator

Implementation:

- Host (core): use `app::palette_controller::render_palette_footer` for any palette footer
- Plugins (IPC): use `ui::hints_row` for a single hint row at an arbitrary position,
  `ui::palette_footer` for the standard 3-row footer anchored to a `PaletteRect`,
  or `PanelOpts.footer` for a `draw_panel` footer — all share the same style
- Do NOT hand-roll hint rows with `RenderCmd::Text` or `text_at`; every footer must
  go through one of the helpers above so colors and separators stay consistent

## Key-value detail rows

Detail panels (e.g. registry Plugin Actions dialog, music-preview Track Details)
render `Label: value` rows as two separate `RenderCmd::Text` commands:

- Key renders in `theme.text_muted`, followed by `:` and a single space
- Value renders in `theme.text` (semantic overrides allowed, e.g. `theme.success` for a status)
- No alignment padding — value starts right after the key, regardless of other rows

