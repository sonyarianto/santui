# Ratatui-Native Migration Plan

## Status

| Phase | Status |
|-------|--------|
| Phase 0 — Widget Library | ✅ Complete |
| Phase 1 — Host UI | ✅ Complete |
| Phase 2 — IPC Bridge | ✅ Complete |
| Phase 3 — Cleanup | ⏳ Pending |
| Phase 4 — Tests | ⏳ Pending |

---

## Motivation

The current codebase has significant drift from ratatui conventions: hand-rolled lists,
custom buffer-level border drawing, manual cell-by-cell dimming, and ad-hoc overlay
positioning. This makes UI development harder, introduces bugs, and raises the bar for
new contributors.

**Goal**: Use ratatui widgets (`List`, `Block`, `Clear`, `Table`, `Layout`, `Widget` trait)
wherever possible. The IPC protocol stays serializable — only host-side rendering changes.

---

## Phase 0 — Widget Component Library

Create `crates/core/src/widgets/` module with reusable ratatui widgets.

```
crates/core/src/
├── widgets/
│   ├── mod.rs              # re-exports
│   ├── popup.rs            # generic overlay container
│   ├── dim_overlay.rs      # dims content area behind an overlay
│   ├── filtered_list.rs    # searchable/filterable List (StatefulWidget)
│   ├── command_palette.rs  # palette orchestrating Popup + FilteredList
│   ├── theme_picker.rs     # theme picker using Popup + FilteredList
│   └── panel.rs            # panel with Block border
```

### `widgets/popup.rs`

Generic vertically-centered overlay with dimmed background, optional title bar, border,
and inner content area. Replaces the manual `Clear` + `Paragraph` stacking and hardcoded
positioning in `palette_widget.rs` and `theme_manager.rs`.

```rust
pub struct Popup<'a> {
    title: Option<Line<'a>>,
    min_width: u16,
    ideal_width: u16,
    min_height: u16,
    max_height: u16,  // computed from content area
    inner: Option<Box<dyn Widget + 'a>>,
}

impl Widget for Popup<'_> { ... }
```

Uses `Layout` internally to center the popup, `Block` for border, `Clear` for the
inner rectangle.

### `widgets/filtered_list.rs`

A stateful widget wrapping `List` + `ListState` with search/filter and category
grouping. Replaces all hand-rolled cursor/scroll/grouping logic.

```rust
pub struct FilteredListState {
    query: String,
    cursor: usize,
    list_state: ListState,
    items: Vec<FilteredItem>,
    groups: Vec<Group>,  // (name, start_idx, count)
    scroll: u16,
}

pub struct FilteredList<'a> {
    query: &'a str,
    items: &'a [FilteredItem],      // all items
    groups: &'a [Group],
    cursor: usize,
    list_state: &'a mut ListState,
    highlight_style: Style,
    group_style: Style,
}
```

Render approach: Build a `Vec<ListItem>` where category headers are non-selectable
`ListItem`s with distinct style. `ListState::selected()` points into the flat list.
Category headers use `Modifier::BOLD` + `theme.accent`; items use `theme.text`;
selected item uses `theme.highlight` + `theme.inverted_text`.

```rust
impl StatefulWidget for FilteredList<'_> {
    type State = ListState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let items: Vec<ListItem> = ...; // build flat list with group headers
        List::new(items)
            .highlight_style(self.highlight_style)
            .render(area, buf, state);
    }
}
```

### `widgets/dim_overlay.rs`

Replaces the cell-by-cell buffer iteration in `app/mod.rs:773-793`.

```rust
pub struct DimOverlay {
    area: Rect,
}

impl Widget for DimOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill area with a dimmed-background style using `Clear` + styled `Paragraph`
        // Or better: just fill each cell's bg with the overlay color
        // Ratatui doesn't have a "dim existing content" primitive, but we can
        // use buf.set_style(area, Style::default().bg(dim_bg)) to overlay.
    }
}
```

Simpler approach — **replace the dim loop with**:

```rust
let dim_style = Style::default().bg(self.app_state.theme.background_overlay);
f.buffer_mut().set_style(chunks[0], dim_style);
```

This avoids the cell-by-cell iteration and gives a uniform dim overlay.
If we want per-cell fg dimming too (preserving text while dimming), we could
use `buf.set_style()` with a combined fg+bg style, but a bg-only overlay is
simpler and often looks fine. We can keep `dim_color` for the fg pass only
if visual quality demands it, but use the buffer's native `set_style` instead
of manual iteration.

---

## Phase 1 — Host UI Refactoring

### 1.1 Replace dim overlay loop

**File**: `crates/core/src/app/mod.rs` (lines 773-793)

**Before**:
```rust
if self.palette_controller.is_open() || self.app_state.theme_picker_open {
    let dim_bg = self.app_state.theme.background_overlay;
    let buf = f.buffer_mut();
    const DIM: f64 = 0.45;
    for y in chunks[0].top()..chunks[0].bottom() {
        for x in chunks[0].left()..chunks[0].right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                let mut style = cell.style();
                if let Some(fg) = style.fg { style.fg = Some(dim_color(fg, DIM)); }
                if let Some(bg) = style.bg { style.bg = Some(dim_color(bg, DIM)); }
                else { style.bg = Some(dim_bg); }
                cell.set_style(style);
            }
        }
    }
}
```

**After**:
```rust
if self.palette_controller.is_open() || self.app_state.theme_picker_open {
    let dim_style = Style::default().bg(self.app_state.theme.background_overlay);
    f.buffer_mut().set_style(chunks[0], dim_style);
}
```

Or use the `DimOverlay` widget if mk. Keep `dim_color` as a private helper if
fg-perception matters, but the loop should still use `buf.set_style()` with
pre-computed colors instead of per-cell branching.

### 1.2 Replace palette with FilteredList + Popup

**Files**: `palette_widget.rs` (339 lines), `palette_controller.rs` (217 lines)

**Current**: ~556 lines of hand-rolled cursor, scroll, grouping, filtering, rendering.

**After**: ~150 lines total. PaletteController becomes thin — it owns a
`FilteredListState` and renders a `Popup` containing a `FilteredList`.

Key changes:
- `PaletteWidget` struct → `FilteredListState` (or inline in PaletteController)
- `render_with_groups()` → `FilteredList` stateful widget inside `Popup`
- `filtered_items()` → O(1) lookup via `FilteredListState` filter
- `ensure_cursor_visible()` → handled by `ListState` natively
- Query input, cursor nav → same (just state management, no rendering)

### 1.3 Replace theme picker with FilteredList + Popup

**File**: `theme_manager.rs` (281 lines)

**Current**: ~160 lines of hand-rolled list rendering + state management.

**After**: Theme picker state uses `FilteredListState`; rendering is a `Popup`
containing a `FilteredList` with a live-preview `Paragraph` below.

### 1.4 Refactor splash screen / about screen

**File**: `screens.rs`

- Use `Block::default().borders(Borders::ALL)` or `.borders(Borders::NONE)` with
  `style(fg=theme.border)` instead of raw `Paragraph::new(...)` without borders.
- If borders are desired, add them. If not, at least use `block: Some(Block::default()
  .style(...))` for consistency.

### 1.5 Clean up status bar

**File**: `status_bar.rs`

- Already fairly clean, but consider using `Layout::horizontal` with `Constraint::Length`
  for the right-aligned overlays instead of manual `Line` + `Span` position calculation
  with `x = area.right().saturating_sub(...)`.

---

## Phase 2 — IPC Bridge Refactoring

**Key insight**: The IPC protocol (`RenderCmd` enum) stays the same — it must be
serializable. Only the host-side translation in `render_commands()` changes.

### 2.1 `RenderCmd::Border` → `Block`

**File**: `crates/ipc/src/render.rs` (lines 145-172)

**Before**: Draws `┌─┐│└─┘` manually via `buf.set_string()`.

**After**:
```rust
RenderCmd::Border { x, y, w, h, fg } => {
    let area = Rect { x, y, width: w, height: h };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(fg));
    block.render(area, buf);
}
```

### 2.2 `RenderCmd::Rect` → `Clear` + `Block` (or just `Clear` + style)

**File**: `crates/ipc/src/render.rs` (lines 111-123)

**Before**: Fills area with spaces and bg color via `buf.set_stringn()`.

**After**:
```rust
RenderCmd::Rect { x, y, w, h, bg } => {
    let area = Rect { x, y, width: w, height: h };
    Clear.render(area, buf);
    Block::default().style(Style::default().bg(bg)).render(area, buf);
}
```

### 2.3 `RenderCmd::Clear` → `Clear` widget

**File**: `crates/ipc/src/render.rs` (lines 99-109)

**Before**: Manual space-fill loop.

**After**:
```rust
RenderCmd::Clear { x, y, w, h } => {
    Clear.render(Rect { x, y, width: w, height: h }, buf);
}
```

### 2.4 `RenderCmd::Text` → `Paragraph` (or `Span`)

**File**: `crates/ipc/src/render.rs`

**Before**: `buf.set_string()` with manual style.

**After**:
```rust
RenderCmd::Text { x, y, text, fg, bg, bold } => {
    let area = Rect { x, y, width: text.len() as u16, height: 1 };
    let style = Style::default().fg(fg).bg(bg);
    let style = if bold { style.add_modifier(Modifier::BOLD) } else { style };
    Paragraph::new(Span::styled(text, style)).render(area, buf);
}
```

### 2.5 `RenderCmd::Paragraph` — already fine, just use `Paragraph` widget

Already maps to `Paragraph`. Clean up if needed.

### 2.6 `RenderCmd::List` — already uses `List`, may need cleanup

Already maps to `List` + `ListState`. Verify `highlight_style` uses semantic colors.

### 2.7 `RenderCmd::Table` — already uses `Table`, may need cleanup

Already maps to `Table` + `TableState`. Verify styling.

---

## Phase 3 — Deprecation & Cleanup

### 3.1 Remove dead code

- `dim_color()` and `parse_hex()` in `mod.rs` — keep if used for config, remove if dead
- `pal_w()`, `max_list_h()`, `PAD_*`, `HEADER_H`, `PAL_*` constants — no longer needed
  after Popup handles layout automatically

### 3.2 Remove old palette widget file

`palette_widget.rs` → deleted (functionality absorbed into `widgets/filtered_list.rs`
and `widgets/command_palette.rs`)

### 3.3 Trim palette_controller.rs

Becomes a thin state wrapper delegating to `FilteredListState` and `Popup`.

### 3.4 Trim theme_manager.rs

Picker rendering moves to `widgets/theme_picker.rs`; manager keeps theme list + load/save.

### 3.5 Remove manual IPC buffer writes

`ipc/src/render.rs` should have no direct `buf.set_string()` or `buf.set_style()` calls
— everything goes through ratatui widgets.

---

## Phase 4 — Testing

- Existing tests for filtering, dim_color, parse_hex should still pass (or be removed
  if the function is removed)
- Add tests for `FilteredList` render output (snapshot or property-based)
- Add tests for `Popup` layout (correct centering, border rendering)
- Ensure `cargo test --workspace` + `cargo clippy --workspace -- -D warnings` pass

---

## Impact Summary

| File | Before (approx) | After (approx) | Delta |
|------|----------------|----------------|-------|
| `app/mod.rs` (dim overlay) | ~20 lines manual loop | ~3 lines | -17 |
| `palette_widget.rs` | 339 lines | deleted | -339 |
| `palette_controller.rs` | 217 lines | ~80 lines | -137 |
| `theme_manager.rs` (picker render) | ~160 lines | ~40 lines | -120 |
| `ipc/src/render.rs` (Border/Rect/Clear/Text) | ~80 lines | ~30 lines | -50 |
| `status_bar.rs` (right-aligned spans) | ~10 lines manual calc | ~5 lines Layout | -5 |
| **New files**: `widgets/` | — | ~400 lines | +400 |
| **Net change** | | | **~-268 lines** |

Deeper ratatui integration means **less code, more predictable behavior, free features**
(scrolling, highlighting, focus management, composability).

---

## Migration Order

```
Phase 0: widgets/ module (foundation)
    ↓
Phase 1: Host UI (mod.rs, palette, theme picker, screens)
    ↓
Phase 2: IPC bridge (render.rs — no protocol change)
    ↓
Phase 3: Cleanup (remove dead code, trim old files)
    ↓
Phase 4: Tests + lint
```

Each phase is self-contained and can be PR'd independently.
