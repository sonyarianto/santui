use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl super::Santui {
    pub(super) fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C quits immediately from any screen, even in raw mode where
        // it arrives as a key event rather than a signal.
        if matches!(key.code, KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL)) {
            self.app_state.running = false;
            return;
        }
        if self.palette_controller.is_open() {
            return self.handle_key_palette(key);
        }
        if self.app_state.theme_picker_open {
            return self.handle_key_theme_picker(key);
        }
        if self.app_state.show_about {
            return self.handle_key_about(key);
        }
        self.handle_key_normal(key);
    }

    fn handle_key_palette(&mut self, key: KeyEvent) {
        let cmds = self.plugin_manager.commands();
        let bi = &self.app_state.builtin_items;
        let (_, term_h) = crossterm::terminal::size().unwrap_or((80, 24));
        let action = self.palette_controller.handle_key(
            key,
            term_h,
            bi,
            self.plugin_manager.dynamic_items(),
            cmds,
        );
        if let super::palette_controller::PaletteAction::Execute(idx) = action {
            self.execute_palette_selection(idx);
        }
    }

    fn execute_palette_selection(&mut self, idx: super::ItemIndex) {
        match idx {
            super::ItemIndex::Builtin(bi) => {
                let id = self.app_state.builtin_items[bi].0;
                match id {
                    super::BuiltinId::SignInGoogle => {
                        if let Some(ref auth) = self.auth {
                            if let Err(e) = auth.start_sign_in("google") {
                                log::error!("[auth] Google sign-in error: {e}");
                            }
                        }
                    }
                    super::BuiltinId::SignInGitHub => {
                        if let Some(ref auth) = self.auth {
                            if let Err(e) = auth.start_sign_in("github") {
                                log::error!("[auth] GitHub sign-in error: {e}");
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
                }
            }
            super::ItemIndex::PluginCmd(pci) => {
                let (plugin_idx, local_idx, _) = self.plugin_manager.commands()[pci];
                if plugin_idx < self.plugin_manager.len() {
                    self.plugin_manager.set_active(Some(plugin_idx));
                    self.plugin_manager
                        .handle_palette_command(plugin_idx, local_idx);
                }
            }
            super::ItemIndex::Dynamic(di) => {
                if let Some((_cat, id, name)) = self.plugin_manager.dynamic_items().get(di).cloned()
                {
                    if let Some(existing) = self.plugin_manager.find_by_id(&id) {
                        self.plugin_manager.set_active(Some(existing));
                    } else {
                        // Plugin not loaded yet — read its binary path from registry.toml.
                        let cfg_path = self.plugin_manager.data_dir().join("registry.toml");
                        if let Some(cfg) = super::plugin_manager::RegistryConfig::load(&cfg_path) {
                            if let Some(installed) = cfg.plugins.iter().find(|p| {
                                p.path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .map(|s| s.trim_end_matches(".exe"))
                                    .is_some_and(|stem| stem == name)
                            }) {
                                let mut ctx = crate::plugin::PluginContext {
                                    theme: self.app_state.theme.clone(),
                                    auth: self.auth.clone(),
                                    data_dir: self.plugin_manager.data_dir().to_path_buf(),
                                };
                                if let Ok(idx) = self.plugin_manager.spawn_and_init(
                                    &id,
                                    &name,
                                    &installed.path,
                                    &mut ctx,
                                ) {
                                    self.plugin_manager.set_active(Some(idx));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn activate_carousel_item(&mut self, ci: usize) {
        let carousel = self.plugin_manager.carousel_items();
        let Some(item) = carousel.get(ci) else {
            return;
        };

        if let Some(plugin_idx) = item.plugin_idx {
            // Plugin is already loaded — just activate it.
            self.plugin_manager.set_active(Some(plugin_idx));
        } else if let Some(cfg) = super::plugin_manager::RegistryConfig::load(
            &self.plugin_manager.data_dir().join("registry.toml"),
        ) {
            // Look up the binary path in registry.toml and spawn the plugin.
            if let Some(installed) = cfg.plugins.iter().find(|p| {
                p.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.trim_end_matches(".exe"))
                    == Some(item.id.as_str())
            }) {
                let mut ctx = crate::plugin::PluginContext {
                    theme: self.app_state.theme.clone(),
                    auth: self.auth.clone(),
                    data_dir: self.plugin_manager.data_dir().to_path_buf(),
                };
                if let Ok(idx) = self.plugin_manager.spawn_and_init(
                    &item.id,
                    &item.name,
                    &installed.path,
                    &mut ctx,
                ) {
                    self.plugin_manager.set_active(Some(idx));
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

    fn handle_key_normal(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Char('p') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL))
        {
            self.palette_controller.open();
            return;
        }

        match self.plugin_manager.active() {
            None => match key.code {
                KeyCode::Char('q') => self.app_state.running = false,
                KeyCode::Char('?') => self.app_state.show_about = true,
                KeyCode::Right | KeyCode::Char('l') => {
                    let carousel = self.plugin_manager.carousel_items();
                    let n = carousel.len();
                    if n == 0 {
                        return;
                    }
                    self.app_state.home_selected = Some(match self.app_state.home_selected {
                        None => 0,
                        Some(i) if i + 1 >= n => {
                            self.app_state.home_selected = None;
                            return;
                        }
                        Some(i) => i + 1,
                    });
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    let carousel = self.plugin_manager.carousel_items();
                    let n = carousel.len();
                    if n == 0 {
                        return;
                    }
                    self.app_state.home_selected = Some(match self.app_state.home_selected {
                        None => n - 1,
                        Some(0) => {
                            self.app_state.home_selected = None;
                            return;
                        }
                        Some(i) => i - 1,
                    });
                }
                KeyCode::Enter => {
                    if let Some(ci) = self.app_state.home_selected {
                        self.activate_carousel_item(ci);
                    }
                }
                _ => {}
            },
            Some(idx) => match key.code {
                KeyCode::Esc => {
                    self.plugin_manager.on_blur(idx);
                    self.plugin_manager.set_active(None);
                    self.app_state.home_selected = None;
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
