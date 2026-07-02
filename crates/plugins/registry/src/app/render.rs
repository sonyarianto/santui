use santui_ipc::protocol::{RenderCmd, TextStyle};
use santui_ipc::ui;

use super::state::{Action, App};

pub(super) fn max_list_h(content_h: u16) -> u16 {
    content_h.saturating_sub(10).max(3)
}

impl App {
    pub(super) fn fg(&self, color: [u8; 3]) -> Option<[u8; 3]> {
        Some(color)
    }

    pub(super) fn bg(&self) -> Option<[u8; 3]> {
        None
    }

    pub(super) fn hints(&self) -> Vec<(String, String)> {
        if self.detail_idx.is_some() {
            vec![
                ("↑↓".into(), "navigate".into()),
                ("↵".into(), "select".into()),
                ("esc".into(), "back".into()),
            ]
        } else {
            vec![
                ("↑↓".into(), "navigate".into()),
                ("↵".into(), "select".into()),
            ]
        }
    }

    pub(super) fn render_commands(&self) -> Vec<RenderCmd> {
        let mut cmds = Vec::new();

        if let Some(detail_idx) = self.detail_idx {
            self.render_list(&mut cmds);
            self.render_dialog(detail_idx, &mut cmds);
        } else {
            self.render_list(&mut cmds);
        }

        cmds
    }

    fn render_list(&self, cmds: &mut Vec<RenderCmd>) {
        let t = &self.theme;
        let aw = self.area.w;
        let ah = self.area.h;
        if aw < 10 || ah < 3 {
            return;
        }
        let inner_w = (aw.saturating_sub(4)) as usize;

        ui::draw_panel(cmds, t, 0, 0, aw, ah, "Plugins");

        let info = if self.status.is_empty() {
            self.registry.as_ref().map_or_else(String::new, |reg| {
                format!("{} available", reg.available.len())
            })
        } else {
            self.status.clone()
        };
        if !info.is_empty() {
            let info_x = aw.saturating_sub(info.len() as u16 + 2);
            cmds.push(RenderCmd::Text {
                x: info_x,
                y: 1,
                text: info,
                fg: Some(t.text_muted),
                bg: None,
                bold: false,
            });
        }

        if let Some((downloaded, total)) = self.download_progress {
            let bar_w = inner_w.saturating_sub(7).max(10);
            let pct = if total > 0 {
                (downloaded as f64 / total as f64).min(1.0)
            } else {
                0.0
            };
            let filled = (pct * bar_w as f64).round() as usize;
            let empty = bar_w.saturating_sub(filled);
            let pct_display = (pct * 100.0) as u32;
            let bar = format!(
                "[{}{}] {:3}%",
                "=".repeat(filled),
                " ".repeat(empty),
                pct_display
            );
            cmds.push(RenderCmd::Text {
                x: 2,
                y: 2,
                text: bar,
                fg: Some(t.accent),
                bg: None,
                bold: false,
            });
        }

        if let Some(ref reg) = self.registry {
            let list_top = 3u16;
            let list_h = max_list_h(ah) as usize;

            if reg.available.is_empty() {
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: list_top,
                    text: "No plugins available".into(),
                    fg: self.fg(t.text_muted),
                    bg: self.bg(),
                    bold: false,
                });
                return;
            }

            let publisher_w: usize = 10;
            let installed_w: usize = 9;
            let status_w: usize = 9;
            let ver_w: usize = 8;
            let rem = inner_w.saturating_sub(publisher_w + installed_w + status_w + ver_w);
            let name_w = (rem * 4 / 10).max(5);
            let desc_w = rem.saturating_sub(name_w).max(5);

            let visible_count =
                list_h.min(reg.available.len().saturating_sub(self.scroll as usize));

            let mut rows: Vec<Vec<String>> = Vec::with_capacity(visible_count);
            for i in 0..visible_count {
                let idx = self.scroll as usize + i;
                if idx >= reg.available.len() {
                    break;
                }
                let plugin = &reg.available[idx];
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

                let installed = if is_installed { "Yes" } else { "No" };
                let status = if is_enabled {
                    "Enabled"
                } else if is_installed {
                    "Disabled"
                } else {
                    "-"
                };

                let name_s = ui::truncate(&plugin.name, name_w);
                let desc_s = ui::truncate(&plugin.description, desc_w);
                let publisher_s = ui::truncate(&plugin.publisher, publisher_w);
                let ver_s = ui::truncate(&plugin.version, ver_w);

                rows.push(vec![
                    name_s,
                    desc_s,
                    publisher_s,
                    ver_s,
                    installed.into(),
                    status.into(),
                ]);
            }

            let vis_selected = {
                let cursor = self.cursor;
                let scroll = self.scroll as usize;
                if cursor >= scroll && cursor < scroll + visible_count {
                    Some(cursor - scroll)
                } else {
                    None
                }
            };

            cmds.push(RenderCmd::Table {
                x: 2,
                y: list_top,
                w: inner_w as u16,
                h: list_h as u16,
                header: vec![
                    "Name".into(),
                    "Description".into(),
                    "Publisher".into(),
                    "Version".into(),
                    "Installed".into(),
                    "Status".into(),
                ],
                header_style: TextStyle {
                    fg: Some(t.text_muted),
                    bg: None,
                    bold: true,
                },
                rows,
                column_widths: vec![
                    name_w as u16,
                    desc_w as u16,
                    publisher_w as u16,
                    ver_w as u16,
                    installed_w as u16,
                    status_w as u16,
                ],
                selected: vis_selected,
                style: TextStyle {
                    fg: Some(t.text),
                    bg: None,
                    bold: false,
                },
                highlight_style: TextStyle {
                    fg: Some(t.inverted_text),
                    bg: Some(t.highlight),
                    bold: true,
                },
                current_row: None,
                current_style: None,
            });
        }
    }

    fn render_dialog(&self, detail_idx: usize, cmds: &mut Vec<RenderCmd>) {
        let t = &self.theme;
        let aw = self.area.w;
        let ah = self.area.h;

        let actions = self.available_actions(detail_idx);
        if actions.is_empty() {
            return;
        }

        let Some(reg) = &self.registry else { return };
        let Some(plugin) = reg.available.get(detail_idx) else {
            return;
        };

        let has_publisher = !plugin.publisher.is_empty();
        let field_count = if has_publisher { 4 } else { 3 };
        let footer_h = 3u16;
        let content_h = 2u16   // top padding + title
            + 1                 // blank after title
            + field_count       // Name, [Publisher,] Version, Status
            + 1                 // blank after Status
            + actions.len() as u16
            + footer_h;
        let pr = ui::palette_rect(aw, ah, content_h);
        ui::palette_bg(cmds, t, &pr);
        ui::palette_title(cmds, t, &pr, 1, "Plugin Actions");

        let field_fg = Some(t.text_muted);
        let ix = pr.ix;
        let bg = Some(t.background_panel);

        cmds.push(RenderCmd::Text {
            x: ix,
            y: pr.y + 3,
            text: format!("Name:      {}", plugin.name),
            fg: field_fg,
            bg,
            bold: false,
        });

        if has_publisher {
            cmds.push(RenderCmd::Text {
                x: ix,
                y: pr.y + 4,
                text: format!("Publisher: {}", plugin.publisher),
                fg: field_fg,
                bg,
                bold: false,
            });
        }

        let (installed_idx, is_enabled, installed_ver) = reg
            .installed
            .iter()
            .position(|p| {
                p.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.trim_end_matches(".exe"))
                    == Some(&plugin.id)
            })
            .map(|i| {
                (
                    i,
                    reg.installed[i].enabled,
                    reg.installed[i].version.clone(),
                )
            })
            .unwrap_or((usize::MAX, false, String::new()));

        let ver_text = if installed_idx != usize::MAX && installed_ver != plugin.version {
            format!(
                "Version:   {iv}  →  {av}",
                iv = installed_ver,
                av = plugin.version
            )
        } else {
            format!("Version:   {}", plugin.version)
        };
        let ver_y = if has_publisher { 5 } else { 4 };
        cmds.push(RenderCmd::Text {
            x: ix,
            y: pr.y + ver_y,
            text: ver_text,
            fg: field_fg,
            bg,
            bold: false,
        });

        let status_str = if installed_idx != usize::MAX {
            if is_enabled {
                "Enabled"
            } else {
                "Disabled"
            }
        } else {
            "Not installed"
        };
        let status_color = if is_enabled {
            Some(t.success)
        } else {
            field_fg
        };
        let status_y = if has_publisher { 6 } else { 5 };
        cmds.push(RenderCmd::Text {
            x: ix,
            y: pr.y + status_y,
            text: format!("Status:    {}", status_str),
            fg: status_color,
            bg,
            bold: false,
        });

        // Action items
        let action_base = if has_publisher { 8 } else { 7 };
        for (i, action) in actions.iter().enumerate() {
            let focused = i == self.action_cursor;
            let label = match action {
                Action::Enable => "Enable".into(),
                Action::Disable => "Disable".into(),
                Action::Install => "Install".into(),
                Action::Update => format!("Update to v{}", plugin.version),
                Action::Delete => "Delete".into(),
            };
            ui::palette_item(cmds, t, &pr, action_base + i as u16, &label, focused);
        }

        // Footer: key hints
        let footer_y = pr.y + action_base + actions.len() as u16;
        let dim_fg = Some(t.text_muted);
        cmds.push(RenderCmd::Text {
            x: ix,
            y: footer_y,
            text: "".into(),
            fg: dim_fg,
            bg,
            bold: false,
        });
        cmds.push(RenderCmd::Text {
            x: ix,
            y: footer_y + 1,
            text: "↑↓ navigate • ↵ select".into(),
            fg: dim_fg,
            bg,
            bold: false,
        });
        cmds.push(RenderCmd::Text {
            x: ix,
            y: footer_y + 2,
            text: "".into(),
            fg: dim_fg,
            bg,
            bold: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use santui_ipc::protocol::Area;

    #[test]
    fn test_max_list_h_normal() {
        assert_eq!(max_list_h(24), 14);
    }

    #[test]
    fn test_max_list_h_minimum() {
        assert_eq!(max_list_h(12), 3);
    }

    #[test]
    fn test_max_list_h_small() {
        assert_eq!(max_list_h(13), 3);
    }

    #[test]
    fn test_hints_list_mode() {
        let app = App::new();
        let hints = app.hints();
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0], ("↑↓".into(), "navigate".into()));
        assert_eq!(hints[1], ("↵".into(), "select".into()));
    }

    #[test]
    fn test_hints_detail_mode() {
        let mut app = App::new();
        app.detail_idx = Some(0);
        let hints = app.hints();
        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0], ("↑↓".into(), "navigate".into()));
        assert_eq!(hints[1], ("↵".into(), "select".into()));
        assert_eq!(hints[2], ("esc".into(), "back".into()));
    }

    #[test]
    fn test_fg_returns_some_color() {
        let app = App::new();
        assert_eq!(app.fg([100, 200, 50]), Some([100, 200, 50]));
    }

    #[test]
    fn test_bg_returns_none() {
        let app = App::new();
        assert_eq!(app.bg(), None);
    }

    #[test]
    fn test_render_commands_empty_app() {
        let app = App::new();
        let cmds = app.render_commands();
        assert!(!cmds.is_empty());
        assert!(cmds.iter().any(|c| matches!(c, RenderCmd::Border { .. })));
    }

    #[test]
    fn test_render_commands_small_area_returns_empty() {
        let mut app = App::new();
        app.area = Area { w: 5, h: 2 };
        let cmds = app.render_commands();
        assert!(cmds.is_empty());
    }
}
