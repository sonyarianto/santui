use crossterm::event::{KeyCode, KeyEvent};

impl super::Santui {
    pub(super) fn handle_key(&mut self, key: KeyEvent) {
        if self.palette.is_some() {
            let query = &self.palette.as_ref().unwrap().query;
            let filtered = self.filtered_items(query);
            let palette = self.palette.as_mut().unwrap();

            match key.code {
                KeyCode::Char(c)
                    if c == 'p'
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.palette = None;
                    return;
                }
                KeyCode::Char(_c) if !key.modifiers.is_empty() => {}
                KeyCode::Char(c) => {
                    palette.query.push(c);
                    palette.cursor = 0;
                    palette.scroll = 0;
                }
                KeyCode::Backspace => {
                    palette.query.pop();
                    palette.cursor = 0;
                    palette.scroll = 0;
                }
                KeyCode::Up => {
                    if !filtered.is_empty() {
                        palette.cursor = palette.cursor.saturating_sub(1);
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() {
                        palette.cursor = (palette.cursor + 1).min(filtered.len() - 1);
                    }
                }
                KeyCode::Enter => {
                    if let Some(&idx) = filtered.get(palette.cursor) {
                        match super::CMD_ITEMS[idx].label {
                            "Radio Streaming Player" if !self.plugins.is_empty() => {
                                self.plugins[0].on_focus();
                                self.active_plugin = Some(0);
                            }
                            "Switch theme" => {
                                self.show_theme_picker = true;
                                self.theme_picker_query.clear();
                                self.theme_picker_cursor = self.theme_idx;
                                self.theme_picker_scroll = 0;
                                self.theme_picker_orig_idx = self.theme_idx;
                            }
                            "About" => self.show_about = true,
                            _ => {}
                        }
                    }
                    self.palette = None;
                }
                KeyCode::Esc => self.palette = None,
                _ => {}
            }

            if self.palette.is_some() {
                let (_, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
                self.ensure_cursor_visible(term_h.saturating_sub(1));
            }
            return;
        }

        if self.show_theme_picker {
            let filtered = self.filtered_themes();
            match key.code {
                KeyCode::Char(c)
                    if c == 'p'
                        && key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.select_theme(self.theme_picker_orig_idx);
                    self.show_theme_picker = false;
                }
                KeyCode::Char(_) if !key.modifiers.is_empty() => {}
                KeyCode::Char(c) => {
                    self.theme_picker_query.push(c);
                    self.theme_picker_cursor = 0;
                    if let Some(&idx) = filtered.first() {
                        self.preview_theme(idx);
                    }
                }
                KeyCode::Backspace => {
                    self.theme_picker_query.pop();
                    self.theme_picker_cursor = 0;
                    let filtered = self.filtered_themes();
                    if let Some(&idx) = filtered.first() {
                        self.preview_theme(idx);
                    }
                }
                KeyCode::Up => {
                    if !filtered.is_empty() {
                        self.theme_picker_cursor = self.theme_picker_cursor.saturating_sub(1);
                        if let Some(&idx) = filtered.get(self.theme_picker_cursor) {
                            self.preview_theme(idx);
                        }
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() {
                        self.theme_picker_cursor =
                            (self.theme_picker_cursor + 1).min(filtered.len() - 1);
                        if let Some(&idx) = filtered.get(self.theme_picker_cursor) {
                            self.preview_theme(idx);
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(&idx) = filtered.get(self.theme_picker_cursor) {
                        self.select_theme(idx);
                    }
                    self.show_theme_picker = false;
                }
                KeyCode::Esc => {
                    self.select_theme(self.theme_picker_orig_idx);
                    self.show_theme_picker = false;
                }
                _ => {}
            }
            if self.show_theme_picker {
                let (_, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
                self.ensure_theme_cursor_visible(term_h.saturating_sub(1));
            }
            return;
        }

        if self.show_about {
            if matches!(key.code, KeyCode::Esc) {
                self.show_about = false;
            }
            return;
        }

        if matches!(key.code, KeyCode::Char('p') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
        {
            self.palette = Some(super::PaletteState {
                query: String::new(),
                cursor: 0,
                scroll: 0,
            });
            return;
        }

        match self.active_plugin {
            None => match key.code {
                KeyCode::Char('q') => self.running = false,
                KeyCode::Char('?') => self.show_about = true,
                _ => {}
            },
            Some(idx) => match key.code {
                KeyCode::Esc => {
                    self.plugins[idx].on_blur();
                    self.active_plugin = None;
                }
                KeyCode::Char('q') => self.running = false,
                KeyCode::Char('?') => self.show_about = true,
                _ => {
                    self.plugins[idx].handle_key(key);
                }
            },
        }
    }
}
