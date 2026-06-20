use crate::auth::{AuthHandle, User};
use crate::theme::Theme;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;
use std::path::Path;
use std::sync::Arc;

/// Factory that creates a `Box<dyn Plugin>` from an id, name, and binary path.
/// The binary (`santui`) sets this to `IpcPluginHost::new_boxed`.
pub type PluginFactory = Arc<dyn Fn(&str, &str, &Path) -> Box<dyn Plugin> + Send + Sync>;

pub trait Plugin {
    fn id(&self) -> &str;
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
    fn on_user_update(&mut self, _user: Option<&User>) {}
    fn status_hints(&self) -> Vec<(String, String)> {
        vec![]
    }
}

pub struct PluginContext {
    pub theme: Theme,
    pub auth: Option<Arc<dyn AuthHandle>>,
}

impl PluginContext {
    pub fn new() -> Self {
        PluginContext {
            theme: Theme::default(),
            auth: None,
        }
    }
}

impl Default for PluginContext {
    fn default() -> Self {
        Self::new()
    }
}
