impl super::Santui {
    pub(super) fn select_theme(&mut self, idx: usize) {
        self.theme = self.theme_manager.select(idx).clone();
        self.ctx.theme = self.theme.clone();
        self.plugin_manager.on_theme_change_all(&self.theme);
        self.event_bus.emit(crate::event::Event::ThemeChanged);
    }

    pub(super) fn preview_theme(&mut self, idx: usize) {
        self.theme = self.theme_manager.preview(idx).clone();
        self.ctx.theme = self.theme.clone();
    }
}
