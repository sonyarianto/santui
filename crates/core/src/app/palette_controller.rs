use super::palette_widget::PaletteWidget;
use super::{BuiltinId, ItemIndex};
use crate::plugin::PluginCmdItem;
use crate::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

type CategoryGroups = Vec<(String, Vec<ItemIndex>)>;

/// Owns the command-palette overlay state and processes key events against
/// it, returning actions for the caller to execute.
pub(super) struct PaletteController {
    palette: Option<PaletteWidget>,
    /// Cached result of `filtered_items()` — recomputed only when query changes.
    cached_filtered: Vec<ItemIndex>,
    /// Cached grouped items — recomputed together with `cached_filtered`.
    cached_groups: CategoryGroups,
    /// True when `cached_filtered` is stale (query changed or palette opened).
    filtered_dirty: bool,
}

pub(super) enum PaletteAction {
    Execute(ItemIndex),
    None,
}

impl PaletteController {
    pub fn new() -> Self {
        Self {
            palette: None,
            cached_filtered: Vec::new(),
            cached_groups: Vec::new(),
            filtered_dirty: true,
        }
    }

    pub fn open(&mut self) {
        self.palette = Some(PaletteWidget::new());
        self.filtered_dirty = true;
    }

    pub fn is_open(&self) -> bool {
        self.palette.is_some()
    }

    /// Process a key event while the palette is open.
    /// Returns `PaletteAction::Execute(idx)` when the user presses Enter
    /// on a selected item; the caller should run the selection.  The
    /// palette is closed automatically on Enter, Esc, or Ctrl+P.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        term_h: u16,
        builtin_items: &[(BuiltinId, String, String)],
        dynamic_items: &[(String, String, String)],
        cmds: &[(usize, usize, PluginCmdItem)],
    ) -> PaletteAction {
        if self.filtered_dirty {
            self.cached_filtered = self
                .palette
                .as_ref()
                .map(|p| p.filtered_items(builtin_items, dynamic_items, cmds))
                .unwrap_or_default();
            self.cached_groups =
                build_groups(&self.cached_filtered, builtin_items, dynamic_items, cmds);
            self.filtered_dirty = false;
        }
        let filtered = &self.cached_filtered;

        match key.code {
            KeyCode::Char(c)
                if c == 'p'
                    && key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.palette = None;
                return PaletteAction::None;
            }
            KeyCode::Char(_) if !key.modifiers.is_empty() => {}
            KeyCode::Char(c) => {
                if let Some(ref mut p) = self.palette {
                    p.query.push(c);
                    p.cursor = 0;
                    p.scroll = 0;
                }
                self.filtered_dirty = true;
            }
            KeyCode::Backspace => {
                if let Some(ref mut p) = self.palette {
                    p.query.pop();
                    p.cursor = 0;
                    p.scroll = 0;
                }
                self.filtered_dirty = true;
            }
            KeyCode::Up => {
                if !filtered.is_empty() {
                    if let Some(ref mut p) = self.palette {
                        p.cursor = if p.cursor == 0 {
                            filtered.len() - 1
                        } else {
                            p.cursor - 1
                        };
                    }
                }
            }
            KeyCode::Down => {
                if !filtered.is_empty() {
                    if let Some(ref mut p) = self.palette {
                        p.cursor = if p.cursor + 1 >= filtered.len() {
                            0
                        } else {
                            p.cursor + 1
                        };
                    }
                }
            }
            KeyCode::Enter => {
                let cursor = self.palette.as_ref().map(|p| p.cursor).unwrap_or(0);
                if let Some(&idx) = filtered.get(cursor) {
                    self.palette = None;
                    return PaletteAction::Execute(idx);
                }
                self.palette = None;
                return PaletteAction::None;
            }
            KeyCode::Esc => {
                self.palette = None;
                return PaletteAction::None;
            }
            _ => {}
        }

        if let Some(ref mut p) = self.palette {
            p.ensure_cursor_visible(
                term_h.saturating_sub(1),
                filtered,
                builtin_items,
                dynamic_items,
                cmds,
            );
        }

        PaletteAction::None
    }

    /// Render the palette overlay if it is open.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        f: &mut Frame,
        area: Rect,
        theme: &Theme,
        tick: u64,
        builtin_items: &[(BuiltinId, String, String)],
        dynamic_items: &[(String, String, String)],
        cmds: &[(usize, usize, PluginCmdItem)],
    ) {
        if let Some(ref pal) = self.palette {
            pal.render_with_groups(
                f,
                area,
                theme,
                tick,
                builtin_items,
                dynamic_items,
                cmds,
                &self.cached_groups,
            );
        }
    }
}

impl Default for PaletteController {
    fn default() -> Self {
        Self::new()
    }
}

fn build_groups(
    filtered: &[ItemIndex],
    builtin_items: &[(BuiltinId, String, String)],
    dynamic_items: &[(String, String, String)],
    cmds: &[(usize, usize, PluginCmdItem)],
) -> CategoryGroups {
    let mut current_cat = String::new();
    let mut cat_items: Vec<ItemIndex> = Vec::new();
    let mut groups: CategoryGroups = Vec::new();
    for &idx in filtered {
        let cat = match idx {
            ItemIndex::Builtin(i) => builtin_items[i].1.clone(),
            ItemIndex::Dynamic(i) => dynamic_items[i].0.clone(),
            ItemIndex::PluginCmd(i) => cmds[i].2.category.clone(),
        };
        if cat != current_cat && !cat_items.is_empty() {
            groups.push((current_cat.clone(), std::mem::take(&mut cat_items)));
        }
        current_cat = cat;
        cat_items.push(idx);
    }
    if !cat_items.is_empty() {
        groups.push((current_cat, cat_items));
    }
    groups
}
