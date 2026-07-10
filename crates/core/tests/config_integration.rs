use santui_core::config::{Config, ConfigManager, CustomThemeColors};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

/// Helper: create a temp dir that cleans itself up on drop.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut p = std::env::temp_dir();
        p.push(format!("santui_test_cfg_{}", id));
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

// ─── Config::load_from ──────────────────────────────────────────────

#[test]
fn config_load_from_missing_dir_returns_default() {
    let tmp = TempDir::new();
    let cfg = Config::load_from(tmp.path());
    assert!(cfg.theme.is_none());
    assert!(cfg.custom_theme.is_none());
}

#[test]
fn config_load_from_valid_toml() {
    let tmp = TempDir::new();
    let p = tmp.path().join("config.toml");
    fs::write(
        &p,
        r##"
theme = "Nord"

[custom_theme]
accent = "#ff8800"
"##,
    )
    .unwrap();

    let cfg = Config::load_from(tmp.path());
    assert_eq!(cfg.theme.as_deref(), Some("Nord"));
    assert_eq!(
        cfg.custom_theme.as_ref().unwrap().accent.as_deref(),
        Some("#ff8800")
    );
}

#[test]
fn config_load_from_malformed_toml_returns_default() {
    let tmp = TempDir::new();
    let p = tmp.path().join("config.toml");
    fs::write(&p, "this is not valid toml {{{").unwrap();
    let cfg = Config::load_from(tmp.path());
    // Should fall back to default without panicking.
    assert!(cfg.theme.is_none());
}

// ─── Config::save_to ────────────────────────────────────────────────

#[test]
fn config_save_roundtrip() {
    let tmp = TempDir::new();
    let cfg = Config {
        theme: Some("Dracula".into()),
        custom_theme: Some(CustomThemeColors {
            name: None,
            accent: Some("#ff0000".into()),
            highlight: None,
            logo: None,
            text: None,
            text_muted: None,
            background: None,
            background_panel: None,
            background_overlay: None,
            border: None,
            success: None,
            error: None,
            inverted_text: None,
        }),
        keybindings: santui_core::config::KeyBindings::default(),
        plugins: None,
        server: None,
    };
    cfg.save_to(tmp.path()).unwrap();

    let loaded = Config::load_from(tmp.path());
    assert_eq!(loaded.theme, Some("Dracula".into()));
    assert_eq!(loaded.custom_theme.unwrap().accent, Some("#ff0000".into()));
}

// ─── ConfigManager ─────────────────────────────────────────────────

#[test]
fn config_manager_new_with_missing_file() {
    let tmp = TempDir::new();
    let mut mgr = ConfigManager::new(tmp.path().to_path_buf());
    assert!(!mgr.dirty);
    assert!(mgr.config().theme.is_none());
    // Poll on missing file should not set dirty.
    mgr.poll();
    assert!(!mgr.dirty);
}

#[test]
fn config_manager_poll_detects_external_change() {
    let tmp = TempDir::new();
    let p = tmp.path().join("config.toml");
    fs::write(&p, r#"theme = "Nord""#).unwrap();

    let mut mgr = ConfigManager::new(tmp.path().to_path_buf());
    assert_eq!(mgr.config().theme.as_deref(), Some("Nord"));
    mgr.ack();

    // Modify config externally.
    std::thread::sleep(Duration::from_millis(50)); // ensure different mtime
    fs::write(&p, r#"theme = "Dracula""#).unwrap();

    mgr.poll();
    assert!(mgr.dirty);
    assert_eq!(mgr.config().theme.as_deref(), Some("Dracula"));
}

#[test]
fn config_manager_ack_clears_dirty() {
    let tmp = TempDir::new();
    let p = tmp.path().join("config.toml");
    fs::write(&p, r#"theme = "Nord""#).unwrap();

    let mut mgr = ConfigManager::new(tmp.path().to_path_buf());
    mgr.ack();

    std::thread::sleep(Duration::from_millis(50));
    fs::write(&p, r#"theme = "Dracula""#).unwrap();
    mgr.poll();
    assert!(mgr.dirty);

    mgr.ack();
    assert!(!mgr.dirty);
}

#[test]
fn config_manager_save_theme() {
    let tmp = TempDir::new();
    let mut mgr = ConfigManager::new(tmp.path().to_path_buf());
    assert!(mgr.config().theme.is_none());
    mgr.ack();

    mgr.save_theme("Nord");
    assert_eq!(mgr.config().theme.as_deref(), Some("Nord"));
    // Clears custom_theme
    assert!(mgr.config().custom_theme.is_none());

    // Verify it was persisted to disk.
    let cfg = Config::load_from(tmp.path());
    assert_eq!(cfg.theme.as_deref(), Some("Nord"));
}

#[test]
fn config_manager_save_theme_clears_custom() {
    let tmp = TempDir::new();
    let mut mgr = ConfigManager::new(tmp.path().to_path_buf());

    // First set custom theme.
    let colors = CustomThemeColors {
        name: None,
        accent: Some("#ff8800".into()),
        highlight: None,
        logo: None,
        text: None,
        text_muted: None,
        background: None,
        background_panel: None,
        background_overlay: None,
        border: None,
        success: None,
        error: None,
        inverted_text: None,
    };
    mgr.save_custom_theme(colors);
    assert!(mgr.config().custom_theme.is_some());

    // Now save a built-in theme — custom_theme should be cleared.
    mgr.save_theme("Nord");
    assert!(mgr.config().custom_theme.is_none());
}

#[test]
fn config_manager_save_custom_theme() {
    let tmp = TempDir::new();
    let mut mgr = ConfigManager::new(tmp.path().to_path_buf());

    let colors = CustomThemeColors {
        name: Some("My Custom".into()),
        accent: Some("#ff8800".into()),
        highlight: Some("#00ff88".into()),
        logo: None,
        text: None,
        text_muted: None,
        background: None,
        background_panel: None,
        background_overlay: None,
        border: None,
        success: None,
        error: None,
        inverted_text: None,
    };
    mgr.save_custom_theme(colors);

    let cc = mgr.config().custom_theme.as_ref().unwrap();
    assert_eq!(cc.name.as_deref(), Some("My Custom"));
    assert_eq!(cc.accent.as_deref(), Some("#ff8800"));
    assert_eq!(cc.highlight.as_deref(), Some("#00ff88"));

    // Verify persisted.
    let cfg = Config::load_from(tmp.path());
    let loaded = cfg.custom_theme.unwrap();
    assert_eq!(loaded.accent, Some("#ff8800".into()));
}

#[test]
fn config_manager_clear_custom_theme() {
    let tmp = TempDir::new();
    let mut mgr = ConfigManager::new(tmp.path().to_path_buf());

    let colors = CustomThemeColors {
        name: None,
        accent: Some("#ff8800".into()),
        highlight: None,
        logo: None,
        text: None,
        text_muted: None,
        background: None,
        background_panel: None,
        background_overlay: None,
        border: None,
        success: None,
        error: None,
        inverted_text: None,
    };
    mgr.save_custom_theme(colors);
    assert!(mgr.config().custom_theme.is_some());

    mgr.clear_custom_theme();
    assert!(mgr.config().custom_theme.is_none());

    // Verify persisted.
    let cfg = Config::load_from(tmp.path());
    assert!(cfg.custom_theme.is_none());
}

#[test]
fn config_manager_save_does_not_trigger_poll() {
    // save_theme should update last_modified so poll() doesn't re-detect.
    let tmp = TempDir::new();
    let mut mgr = ConfigManager::new(tmp.path().to_path_buf());
    mgr.ack();

    mgr.save_theme("Nord");
    assert!(!mgr.dirty);

    mgr.poll();
    // Should NOT set dirty because persist() synced last_modified.
    assert!(!mgr.dirty);
}
