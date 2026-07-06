# Santui Conventions

- Rust edition 2021, no nightly features
- Use `ratatui` for all terminal rendering (no direct terminal writes except crossterm for raw mode)
- Use `Color::Rgb(r, g, b)` for custom colors
- All widgets use ratatui's `Frame`, `Layout`, `Rect`, `Style`, `Span`, `Line`, `Paragraph`
- Use `Theme` semantic colors instead of hardcoded `Color::*` — add new fields to `Theme` if needed
- Add `impl Default` for any type with a `new()` constructor (clippy rule)
- `cargo fmt` before commit; clippy must pass with `-D warnings` (enforced by lefthook pre-commit)
