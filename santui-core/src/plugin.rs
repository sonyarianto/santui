use crate::theme::Theme;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

pub trait Plugin {
    fn id(&self) -> &'static str;
    fn name(&self) -> &str;
    fn init(&mut self, ctx: &mut PluginContext) -> Result<(), Box<dyn std::error::Error>>;
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn render(&self, f: &mut Frame, area: Rect);
    fn tick(&mut self);
    fn on_focus(&mut self) {}
    fn on_blur(&mut self) {}
    fn on_theme_change(&mut self, theme: &Theme) {
        let _ = theme;
    }
    fn status_hints(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }
}

pub struct PluginContext {
    pub theme: Theme,
}

impl PluginContext {
    pub fn new() -> Self {
        PluginContext {
            theme: Theme::default(),
        }
    }
}

impl Default for PluginContext {
    fn default() -> Self {
        Self::new()
    }
}
