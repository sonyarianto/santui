use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::plugin::PluginFactory;
use santui_registry::Registry as PluginRegistry;
use std::path::PathBuf;

use super::Santui;

impl Santui {
    /// Set the registry directory (called from main.rs before run()).
    pub fn set_registry_dir(&mut self, dir: PathBuf) {
        self.registry = Some(PluginRegistry::new(dir));
    }

    /// Set the plugin factory (called from main.rs before run()).
    pub fn set_plugin_factory(&mut self, factory: PluginFactory) {
        self.plugin_factory = Some(factory);
    }

    pub(super) fn ensure_registry_scroll_visible(&mut self) {
        if self.registry.is_some() {
            let content_h = 40u16;
            let list_h = super::max_list_h(content_h);
            let cursor = self.registry_cursor as u16;
            if cursor < self.registry_scroll {
                self.registry_scroll = cursor;
            } else if cursor >= self.registry_scroll + list_h {
                self.registry_scroll = cursor.saturating_sub(list_h.saturating_sub(1));
            }
        }
    }
    /// Fetch plugin manifest and prepare the registry screen.
    pub(super) fn open_registry(&mut self) {
        self.show_registry = true;
        self.registry_status = "Fetching plugins…".to_string();
        self.registry_cursor = 0;
        self.registry_scroll = 0;

        // Try to fetch manifest in a blocking manner (the event loop is sync).
        if let Some(ref mut reg) = self.registry {
            match reg.fetch_manifest() {
                Ok(()) => {
                    self.registry_status = reg.status.clone();
                }
                Err(e) => {
                    self.registry_status = format!("Error: {e}");
                }
            }
        }
    }

    pub(super) fn render_registry(&self, f: &mut Frame, area: Rect) {
        let t = &self.theme;

        // Overlay dimming is handled by the caller (render() in mod.rs).

        let pw = super::pal_w(area.width);
        let inner_w = pw.saturating_sub(super::PAD_L * 2);

        // Calculate palette height
        let list_h = if let Some(ref reg) = self.registry {
            (reg.available.len() as u16).min(super::max_list_h(area.height))
        } else {
            3
        };
        let ideal_pal = super::PAD_T + super::HEADER_H + list_h + super::PAD_B + 2;
        let pal_h = ideal_pal.min(area.height);
        let x = (area.width.saturating_sub(pw)) / 2;
        let y = area.y + (area.height.saturating_sub(pal_h)) / 2;
        let pal_area = Rect {
            x,
            y,
            width: pw,
            height: pal_h,
        };

        f.render_widget(Clear, pal_area);
        f.render_widget(
            Paragraph::new(vec![]).style(Style::default().bg(t.background_panel)),
            pal_area,
        );

        // Title
        let pad_w = inner_w.saturating_sub(12);
        let mut title_spans = vec![Span::styled(
            "Plugin Registry",
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        )];
        if pad_w > 0 {
            title_spans.push(Span::styled(" ".repeat(pad_w as usize), Style::default()));
        }
        title_spans.push(Span::styled("esc", Style::default().fg(t.text_muted)));

        let header_lines = vec![
            Line::from(title_spans),
            Line::from(""),
            Line::from(Span::styled(
                &self.registry_status,
                Style::default().fg(t.text_muted),
            )),
            Line::from(""),
        ];

        let header_area = Rect {
            x: pal_area.x + super::PAD_L,
            y: pal_area.y + super::PAD_T,
            width: inner_w,
            height: super::HEADER_H,
        };
        f.render_widget(Paragraph::new(header_lines), header_area);

        // Plugin list
        let mut list_lines: Vec<Line> = Vec::new();
        if let Some(ref reg) = self.registry {
            if reg.available.is_empty() {
                list_lines.push(Line::from(Span::styled(
                    "No plugins available",
                    Style::default().fg(t.text_muted),
                )));
            } else {
                for (i, plugin) in reg.available.iter().enumerate() {
                    let is_installed = reg.installed.iter().any(|p| {
                        p.path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.trim_end_matches(".exe"))
                            == Some(&plugin.id)
                    });
                    let is_enabled = reg.installed.iter().any(|p| {
                        p.path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.trim_end_matches(".exe"))
                            == Some(&plugin.id)
                            && p.enabled
                    });
                    let hovered = i == self.registry_cursor;
                    let status = if is_enabled {
                        " ON "
                    } else if is_installed {
                        " OFF"
                    } else {
                        "    "
                    };
                    let prefix = if hovered { " ▸ " } else { "   " };
                    let text_fg = if hovered { t.inverted_text } else { t.text };
                    let mut style = Style::default().fg(text_fg);
                    if hovered {
                        style = style.bg(t.highlight).add_modifier(Modifier::BOLD);
                    }
                    let name_display = format!("{}{}  {}", prefix, plugin.name, status);
                    list_lines.push(Line::from(Span::styled(
                        format!("{:<width$}", name_display, width = inner_w as usize),
                        style,
                    )));
                    if hovered {
                        // Show description on hovered line
                        let desc_style = Style::default().fg(t.text_muted);
                        list_lines.push(Line::from(Span::styled(
                            format!("  {}", plugin.description),
                            desc_style,
                        )));
                    }
                }
            }
        }

        let list_top = pal_area.y + super::PAD_T + super::HEADER_H;
        let list_area = Rect {
            x: pal_area.x + super::PAD_L,
            y: list_top,
            width: inner_w,
            height: list_h,
        };
        f.render_widget(
            Paragraph::new(list_lines).scroll((self.registry_scroll, 0)),
            list_area,
        );
    }
}
