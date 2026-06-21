impl super::Santui {
    pub(super) fn select_theme(&mut self, idx: usize) {
        let theme = self.theme_manager.select(idx);
        self.event_bus
            .emit(crate::event::Event::ThemeChanged(theme));
        // Persist the chosen theme to config.toml so it survives restarts.
        let name = &self.theme_manager.themes[idx].0;
        self.config_manager.save_theme(name);
    }

    pub(super) fn preview_theme(&mut self, idx: usize) {
        self.app_state.theme = self.theme_manager.preview(idx);
    }
}
