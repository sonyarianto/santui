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
                        palette.cursor = if palette.cursor == 0 {
                            filtered.len() - 1
                        } else {
                            palette.cursor - 1
                        };
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() {
                        palette.cursor = if palette.cursor + 1 >= filtered.len() {
                            0
                        } else {
                            palette.cursor + 1
                        };
                    }
                }
                KeyCode::Enter => {
                    if let Some(&idx) = filtered.get(palette.cursor) {
                        match idx {
                            super::ItemIndex::Builtin(bi) => match super::CMD_ITEMS[bi].label {
                                "Sign in with Google" => {
                                    if let Some(ref auth) = self.ctx.auth {
                                        if let Ok(user) = auth.sign_in("google") {
                                            for p in &mut self.plugins {
                                                p.on_user_update(Some(&user));
                                            }
                                        }
                                    }
                                }
                                "Sign in with GitHub" => {
                                    if let Some(ref auth) = self.ctx.auth {
                                        if let Ok(user) = auth.sign_in("github") {
                                            for p in &mut self.plugins {
                                                p.on_user_update(Some(&user));
                                            }
                                        }
                                    }
                                }
                                "Sign out" => {
                                    if let Some(ref auth) = self.ctx.auth {
                                        auth.sign_out();
                                        for p in &mut self.plugins {
                                            p.on_user_update(None);
                                        }
                                    }
                                }
                                "Switch theme" => {
                                    self.show_theme_picker = true;
                                    self.theme_picker_query.clear();
                                    self.theme_picker_cursor = self.theme_idx;
                                    self.theme_picker_scroll = 0;
                                    self.theme_picker_orig_idx = self.theme_idx;
                                }
                                "About" => self.show_about = true,
                                "Plugin registry" => self.open_registry(),
                                _ => {}
                            },
                            super::ItemIndex::PluginCmd(pci) => {
                                // Dispatch to the plugin's registered palette command.
                                let (plugin_idx, local_idx, _cmd) =
                                    self.plugin_commands[pci].clone();
                                if plugin_idx < self.plugins.len() {
                                    self.active_plugin = Some(plugin_idx);
                                    self.plugins[plugin_idx].handle_palette_command(local_idx);
                                }
                            }
                            super::ItemIndex::Dynamic(di) => {
                                // Launch a registry-installed plugin via factory.
                                if let Some((_cat, id, name)) = self.dynamic_items.get(di).cloned()
                                {
                                    // Re-use already-running instance if one exists.
                                    if let Some(existing) =
                                        self.plugins.iter().position(|p| p.id() == id)
                                    {
                                        self.active_plugin = Some(existing);
                                    } else if let Some(ref reg) = self.registry {
                                        if let Some(installed) = reg.installed.iter().find(|p| {
                                            p.path
                                                .file_stem()
                                                .and_then(|s| s.to_str())
                                                .map(|s| s.trim_end_matches(".exe"))
                                                == Some(id.as_str())
                                        }) {
                                            if let Some(ref factory) = self.plugin_factory {
                                                let mut plugin =
                                                    factory(&id, &name, &installed.path);
                                                let mut ctx = crate::plugin::PluginContext {
                                                    theme: self.theme.clone(),
                                                    auth: self.ctx.auth.clone(),
                                                };
                                                if plugin.init(&mut ctx).is_ok() {
                                                    let idx = self.plugins.len();
                                                    self.plugins.push(plugin);
                                                    self.active_plugin = Some(idx);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
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
            let mut filtered = self.filtered_themes();
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
                    filtered = self.filtered_themes();
                    if let Some(&idx) = filtered.first() {
                        self.preview_theme(idx);
                    }
                }
                KeyCode::Up => {
                    if !filtered.is_empty() {
                        self.theme_picker_cursor = if self.theme_picker_cursor == 0 {
                            filtered.len() - 1
                        } else {
                            self.theme_picker_cursor - 1
                        };
                        if let Some(&idx) = filtered.get(self.theme_picker_cursor) {
                            self.preview_theme(idx);
                        }
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() {
                        self.theme_picker_cursor = if self.theme_picker_cursor + 1 >= filtered.len()
                        {
                            0
                        } else {
                            self.theme_picker_cursor + 1
                        };
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

        // ---- Registry screen ----
        if self.show_registry {
            match key.code {
                KeyCode::Esc => {
                    self.show_registry = false;
                }
                KeyCode::Down => {
                    if let Some(ref reg) = self.registry {
                        if !reg.available.is_empty() {
                            let max = reg.available.len().saturating_sub(1);
                            self.registry_cursor = (self.registry_cursor + 1).min(max);
                            self.ensure_registry_scroll_visible();
                        }
                    }
                }
                KeyCode::Up => {
                    if self.registry_cursor > 0 {
                        self.registry_cursor -= 1;
                    }
                    self.ensure_registry_scroll_visible();
                }
                KeyCode::Enter => {
                    // Toggle install/enable/disable
                    let plugin = self
                        .registry
                        .as_ref()
                        .and_then(|reg| reg.available.get(self.registry_cursor).cloned());
                    if let Some(plugin) = plugin {
                        if let Some(ref mut reg) = self.registry {
                            let installed_idx = reg.installed.iter().position(|p| {
                                p.path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .map(|s| s.trim_end_matches(".exe"))
                                    == Some(&plugin.id)
                            });
                            if let Some(idx) = installed_idx {
                                let current = reg.installed[idx].enabled;
                                let _ = reg.set_enabled(idx, !current);
                                self.registry_status = if !current {
                                    format!("{} enabled", plugin.name)
                                } else {
                                    format!("{} disabled", plugin.name)
                                };
                            } else {
                                self.registry_status = format!("Downloading {}…", plugin.name);
                                match reg.install(&plugin) {
                                    Ok(()) => {
                                        self.registry_status =
                                            format!("{} installed and enabled", plugin.name);
                                    }
                                    Err(e) => {
                                        self.registry_status = format!("Error: {e}");
                                    }
                                }
                            }
                            self.refresh_dynamic_items();
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        if matches!(key.code, KeyCode::Char('p') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
        {
            // Reset palette to include dynamic items from registry plugins
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
