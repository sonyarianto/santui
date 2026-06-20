use ratatui::style::Color;
use santui_core::Theme;

#[test]
fn theme_all_contains_santui() {
    let themes = Theme::all();
    assert!(themes.iter().any(|(name, _)| *name == "Santui"));
}

#[test]
fn theme_all_contains_nord() {
    let themes = Theme::all();
    assert!(themes.iter().any(|(name, _)| *name == "Nord"));
}

#[test]
fn theme_all_contains_catppuccin() {
    let themes = Theme::all();
    assert!(themes.iter().any(|(name, _)| *name == "Catppuccin"));
}

#[test]
fn theme_all_contains_dracula() {
    let themes = Theme::all();
    assert!(themes.iter().any(|(name, _)| *name == "Dracula"));
}

#[test]
fn theme_all_contains_expected_count() {
    let themes = Theme::all();
    // We expect at least 35+ themes.
    assert!(
        themes.len() >= 36,
        "expected at least 36 themes, got {}",
        themes.len()
    );
}

#[test]
fn theme_default_is_santui_style() {
    let default = Theme::default();
    let themes = Theme::all();
    let santui = themes
        .iter()
        .find(|(n, _)| *n == "Santui")
        .unwrap()
        .1
        .clone();

    // Default should match Santui theme colors.
    assert_eq!(default.accent, santui.accent);
    assert_eq!(default.highlight, santui.highlight);
    assert_eq!(default.text, santui.text);
    assert_eq!(default.background, santui.background);
}

#[test]
fn theme_all_each_has_unique_name() {
    let themes = Theme::all();
    let mut names: Vec<&str> = themes.iter().map(|(n, _)| *n).collect();
    names.sort();
    names.dedup();
    assert_eq!(
        names.len(),
        themes.len(),
        "some themes have duplicate names"
    );
}

#[test]
fn theme_santui_specific_colors() {
    let themes = Theme::all();
    let santui = themes
        .iter()
        .find(|(n, _)| *n == "Santui")
        .unwrap()
        .1
        .clone();

    assert_eq!(santui.accent, Color::Rgb(157, 124, 216));
    assert_eq!(santui.highlight, Color::Rgb(255, 185, 0));
    assert_eq!(santui.text, Color::Rgb(255, 255, 255));
    assert_eq!(santui.background, Color::Reset);
}
