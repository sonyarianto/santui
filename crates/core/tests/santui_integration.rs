use santui_core::plugin::{Plugin, PluginContext};
use santui_core::Santui;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

// ─── Mock plugin for testing ────────────────────────────────────────

struct MockPlugin {
    id: String,
    name: String,
    init_called: bool,
    theme_changed: bool,
}

impl MockPlugin {
    fn new(id: &str, name: &str) -> Self {
        MockPlugin {
            id: id.into(),
            name: name.into(),
            init_called: false,
            theme_changed: false,
        }
    }
}

impl Plugin for MockPlugin {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }

    fn init(&mut self, _ctx: &mut PluginContext) -> Result<(), Box<dyn std::error::Error>> {
        self.init_called = true;
        Ok(())
    }

    fn on_theme_change(&mut self, _theme: &santui_core::Theme) {
        self.theme_changed = true;
    }
}

// ─── TempDir helper ────────────────────────────────────────────────

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut p = std::env::temp_dir();
        p.push(format!("santui_test_app_{}", id));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        TempDir { path: p }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

// ─── Tests ─────────────────────────────────────────────────────────

#[test]
fn santui_new_creates_default_instance() {
    let _app = Santui::new();
    // Default theme should be "Santui".
    let themes = santui_core::Theme::all();
    assert!(themes.len() >= 36);
}

#[test]
fn santui_default_impl() {
    let app = Santui::default();
    let explicit = Santui::new();
    // Both should produce the same result.
    assert_eq!(app.current_theme_name(), explicit.current_theme_name());
}

#[test]
fn santui_default_theme_name() {
    let app = Santui::new();
    assert_eq!(app.current_theme_name(), "Santui");
}

#[test]
fn santui_register_plugin() {
    let mut app = Santui::new();
    let plugin = MockPlugin::new("test_plugin", "Test Plugin");
    app.register(Box::new(plugin));
    // Should not panic.
}

#[test]
fn santui_set_config_dir_changes_theme() {
    let tmp = TempDir::new();
    let config_path = tmp.path().join("config.toml");
    fs::write(&config_path, r#"theme = "Nord""#).unwrap();

    let mut app = Santui::new();
    // Before set_config_dir, theme is Santui (default).
    assert_eq!(app.current_theme_name(), "Santui");

    app.set_config_dir(tmp.path().to_path_buf());
    assert_eq!(app.current_theme_name(), "Nord");
}

#[test]
fn santui_set_config_dir_with_missing_config() {
    let tmp = TempDir::new();
    // No config.toml in tmp — should not panic.
    let mut app = Santui::new();
    app.set_config_dir(tmp.path().to_path_buf());
    // Default theme should still be Santui.
    assert_eq!(app.current_theme_name(), "Santui");
}

#[test]
fn santui_set_config_dir_invalid_config() {
    let tmp = TempDir::new();
    let config_path = tmp.path().join("config.toml");
    fs::write(&config_path, "invalid toml {{{").unwrap();

    let mut app = Santui::new();
    // Should not panic — falls back to defaults.
    app.set_config_dir(tmp.path().to_path_buf());
    assert_eq!(app.current_theme_name(), "Santui");
}
