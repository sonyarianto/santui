use crossterm::event::{KeyCode, KeyEvent};

impl super::Santui {
    pub(super) fn handle_key(&mut self, key: KeyEvent) {
        if self.palette.is_some() {
            return self.handle_key_palette(key);
        }
        if self.app_state.theme_picker_open {
            return self.handle_key_theme_picker(key);
        }
        if self.app_state.show_about {
            return self.handle_key_about(key);
        }
        if self.app_state.registry_open {
            return self.handle_key_registry(key);
        }
        self.handle_key_normal(key);
    }

    fn handle_key_palette(&mut self, key: KeyEvent) {
        let cmds = self.plugin_manager.commands();
        let bi = &self.app_state.builtin_items;
        let filtered = self
            .palette
            .as_ref()
            .map(|p| p.filtered_items(bi, &self.dynamic_items, cmds))
            .unwrap_or_default();

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
                if let Some(ref mut p) = self.palette {
                    p.query.push(c);
                    p.cursor = 0;
                    p.scroll = 0;
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut p) = self.palette {
                    p.query.pop();
                    p.cursor = 0;
                    p.scroll = 0;
                }
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
                    self.execute_palette_selection(idx);
                }
                self.palette = None;
                return;
            }
            KeyCode::Esc => {
                self.palette = None;
                return;
            }
            _ => {}
        }

        if let Some(ref mut p) = self.palette {
            let (_, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
            let cmds = self.plugin_manager.commands();
            let bi = &self.app_state.builtin_items;
            p.ensure_cursor_visible(term_h.saturating_sub(1), bi, &self.dynamic_items, cmds);
        }
    }

    fn execute_palette_selection(&mut self, idx: super::ItemIndex) {
        match idx {
            super::ItemIndex::Builtin(bi) => {
                let id = self.app_state.builtin_items[bi].0;
                match id {
                    super::BuiltinId::SignInGoogle => {
                        if let Some(ref auth) = self.auth {
                            match auth.sign_in("google") {
                                Ok(user) => {
                                    self.plugin_manager.on_user_update_all(Some(&user));
                                    self.event_bus.emit(crate::event::Event::UserUpdated);
                                }
                                Err(e) => eprintln!("[auth] Google sign-in error: {e}"),
                            }
                        }
                    }
                    super::BuiltinId::SignInGitHub => {
                        if let Some(ref auth) = self.auth {
                            if let Err(e) = auth.start_sign_in("github") {
                                eprintln!("[auth] GitHub sign-in error: {e}");
                            }
                        }
                    }
                    super::BuiltinId::SignOut => {
                        if let Some(ref auth) = self.auth {
                            auth.sign_out();
                            self.plugin_manager.on_user_update_all(None);
                            self.event_bus.emit(crate::event::Event::UserUpdated);
                        }
                    }
                    super::BuiltinId::SwitchTheme => {
                        self.app_state.theme_picker_open = true;
                        let tm = &mut self.theme_manager;
                        tm.picker_query.clear();
                        tm.picker_cursor = tm.current_idx;
                        tm.picker_scroll = 0;
                        tm.picker_orig_idx = tm.current_idx;
                    }
                    super::BuiltinId::About => {
                        self.app_state.show_about = true;
                    }
                    super::BuiltinId::PluginRegistry => {
                        self.open_registry();
                    }
                }
            }
            super::ItemIndex::PluginCmd(pci) => {
                let (plugin_idx, local_idx, _cmd) = self.plugin_manager.commands()[pci].clone();
                if plugin_idx < self.plugin_manager.len() {
                    self.plugin_manager.set_active(Some(plugin_idx));
                    self.plugin_manager
                        .handle_palette_command(plugin_idx, local_idx);
                }
            }
            super::ItemIndex::Dynamic(di) => {
                if let Some((_cat, id, name)) = self.dynamic_items.get(di).cloned() {
                    if let Some(existing) = self.plugin_manager.find_by_id(&id) {
                        self.plugin_manager.set_active(Some(existing));
                    } else if let Some(ref reg) = self.registry {
                        if let Some(installed) = reg.installed.iter().find(|p| {
                            p.path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .map(|s| s.trim_end_matches(".exe"))
                                == Some(id.as_str())
                        }) {
                            if let Some(ref factory) = self.plugin_factory {
                                let plugin = factory(&id, &name, &installed.path);
                                let mut ctx = crate::plugin::PluginContext {
                                    theme: self.app_state.theme.clone(),
                                    auth: self.auth.clone(),
                                };
                                if let Ok(idx) = self.plugin_manager.push_and_init(plugin, &mut ctx)
                                {
                                    self.plugin_manager.set_active(Some(idx));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_key_theme_picker(&mut self, key: KeyEvent) {
        let mut filtered = self.theme_manager.filtered();
        match key.code {
            KeyCode::Char(c)
                if c == 'p'
                    && key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.select_theme(self.theme_manager.picker_orig_idx);
                self.app_state.theme_picker_open = false;
            }
            KeyCode::Char(_) if !key.modifiers.is_empty() => {}
            KeyCode::Char(c) => {
                self.theme_manager.picker_query.push(c);
                self.theme_manager.picker_cursor = 0;
                if let Some(&idx) = filtered.first() {
                    self.preview_theme(idx);
                }
            }
            KeyCode::Backspace => {
                self.theme_manager.picker_query.pop();
                self.theme_manager.picker_cursor = 0;
                filtered = self.theme_manager.filtered();
                if let Some(&idx) = filtered.first() {
                    self.preview_theme(idx);
                }
            }
            KeyCode::Up => {
                if !filtered.is_empty() {
                    self.theme_manager.picker_cursor = if self.theme_manager.picker_cursor == 0 {
                        filtered.len() - 1
                    } else {
                        self.theme_manager.picker_cursor - 1
                    };
                    if let Some(&idx) = filtered.get(self.theme_manager.picker_cursor) {
                        self.preview_theme(idx);
                    }
                }
            }
            KeyCode::Down => {
                if !filtered.is_empty() {
                    self.theme_manager.picker_cursor =
                        if self.theme_manager.picker_cursor + 1 >= filtered.len() {
                            0
                        } else {
                            self.theme_manager.picker_cursor + 1
                        };
                    if let Some(&idx) = filtered.get(self.theme_manager.picker_cursor) {
                        self.preview_theme(idx);
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(&idx) = filtered.get(self.theme_manager.picker_cursor) {
                    self.select_theme(idx);
                }
                self.app_state.theme_picker_open = false;
            }
            KeyCode::Esc => {
                self.select_theme(self.theme_manager.picker_orig_idx);
                self.app_state.theme_picker_open = false;
            }
            _ => {}
        }
        if self.app_state.theme_picker_open {
            let (_, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
            self.theme_manager
                .ensure_cursor_visible(term_h.saturating_sub(1));
        }
    }

    fn handle_key_about(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc) {
            self.app_state.show_about = false;
        }
    }

    fn handle_key_registry(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.app_state.registry_open = false;
            }
            KeyCode::Down => {
                if let Some(ref reg) = self.registry {
                    if !reg.available.is_empty() {
                        let max = reg.available.len().saturating_sub(1);
                        self.registry_screen.cursor = (self.registry_screen.cursor + 1).min(max);
                        self.ensure_registry_scroll_visible();
                    }
                }
            }
            KeyCode::Up => {
                if self.registry_screen.cursor > 0 {
                    self.registry_screen.cursor -= 1;
                }
                self.ensure_registry_scroll_visible();
            }
            KeyCode::Enter => {
                let plugin = self
                    .registry
                    .as_ref()
                    .and_then(|reg| reg.available.get(self.registry_screen.cursor).cloned());
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
                            self.registry_screen.status = if !current {
                                format!("{} enabled", plugin.name)
                            } else {
                                format!("{} disabled", plugin.name)
                            };
                        } else {
                            self.registry_screen.status = format!("Downloading {}…", plugin.name);
                            match reg.install(&plugin) {
                                Ok(()) => {
                                    self.registry_screen.status =
                                        format!("{} installed and enabled", plugin.name);
                                }
                                Err(e) => {
                                    self.registry_screen.status = format!("Error: {e}");
                                }
                            }
                        }
                        self.refresh_dynamic_items();
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_key_normal(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Char('p') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
        {
            self.palette = Some(super::palette_widget::PaletteWidget::new());
            return;
        }

        match self.plugin_manager.active() {
            None => match key.code {
                KeyCode::Char('q') => self.app_state.running = false,
                KeyCode::Char('?') => self.app_state.show_about = true,
                _ => {}
            },
            Some(idx) => match key.code {
                KeyCode::Esc => {
                    self.plugin_manager.on_blur(idx);
                    self.plugin_manager.set_active(None);
                }
                KeyCode::Char('q') => self.app_state.running = false,
                KeyCode::Char('?') => self.app_state.show_about = true,
                _ => {
                    self.plugin_manager.handle_key(idx, key);
                }
            },
        }
    }
}
