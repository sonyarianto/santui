use santui_ipc::protocol::{RenderCmd, TextStyle};
use santui_ipc::ui;

use super::state::{Action, App};

pub(super) fn max_list_h(content_h: u16) -> u16 {
    content_h.saturating_sub(8).max(3)
}

impl App {
    pub(super) fn fg(&self, color: [u8; 3]) -> Option<[u8; 3]> {
        Some(color)
    }

    pub(super) fn bg(&self) -> Option<[u8; 3]> {
        Some(self.theme.background_panel)
    }

    pub(super) fn hints(&self) -> Vec<(String, String)> {
        if self.detail_idx.is_some() {
            vec![
                ("↑↓".into(), "Navigate".into()),
                ("Enter".into(), "Select action".into()),
                ("Esc".into(), "Back".into()),
            ]
        } else {
            vec![
                ("↑↓".into(), "Navigate".into()),
                ("Enter".into(), "Actions".into()),
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
        let inner_w = (aw.saturating_sub(3)) as usize;

        ui::draw_panel(cmds, t, 0, 0, aw, ah, "Plugins");

        let status_x = aw.saturating_sub(self.status.len() as u16 + 1);
        cmds.push(RenderCmd::Text {
            x: status_x,
            y: 0,
            text: self.status.clone(),
            fg: Some(t.text_muted),
            bg: Some(t.background_panel),
            bold: false,
        });

        let has_progress = self.download_progress.is_some();
        if let Some((downloaded, total)) = self.download_progress {
            let bar_w = inner_w.saturating_sub(6).max(10);
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
                bg: Some(t.background_panel),
                bold: false,
            });
        }

        if let Some(ref reg) = self.registry {
            let list_top = if has_progress { 4u16 } else { 2u16 };
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

            let status_w: usize = 9;
            let ver_w: usize = 10;
            let rem = inner_w.saturating_sub(status_w + ver_w);
            let name_w = (rem * 3 / 10).max(5);
            let desc_w = rem.saturating_sub(name_w);

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

                let status = if is_enabled {
                    "Enabled"
                } else if is_installed {
                    "Disabled"
                } else {
                    "--"
                };

                let name_s = ui::truncate(&plugin.name, name_w);
                let desc_s = ui::truncate(&plugin.description, desc_w);
                let ver_s = ui::truncate(&plugin.version, ver_w);

                rows.push(vec![status.into(), name_s, desc_s, ver_s]);
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
                    "Status".into(),
                    "Name".into(),
                    "Description".into(),
                    "Version".into(),
                ],
                header_style: TextStyle {
                    fg: Some(t.text_muted),
                    bg: Some(t.background_panel),
                    bold: true,
                },
                rows,
                column_widths: vec![status_w as u16, name_w as u16, desc_w as u16, ver_w as u16],
                selected: vis_selected,
                style: TextStyle {
                    fg: Some(t.text),
                    bg: Some(t.background_panel),
                    bold: false,
                },
                highlight_style: TextStyle {
                    fg: Some(t.inverted_text),
                    bg: Some(t.highlight),
                    bold: true,
                },
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

        let content_h = 7u16 + actions.len() as u16;
        let pr = ui::palette_rect(aw, ah, content_h);
        ui::palette_bg(cmds, t, &pr);
        // PAD_T=1 (blank), title, then Name/Version/Status info fields
        ui::palette_title(cmds, t, &pr, 1, "Plugin Actions");

        let field_fg = Some(t.text_muted);
        let ix = pr.ix;
        let bg = Some(t.background_panel);

        cmds.push(RenderCmd::Text {
            x: ix,
            y: pr.y + 2,
            text: format!(" Name:    {}", plugin.name),
            fg: field_fg,
            bg,
            bold: false,
        });

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
                " Version: {iv}  →  {av}",
                iv = installed_ver,
                av = plugin.version
            )
        } else {
            format!(" Version: {}", plugin.version)
        };
        cmds.push(RenderCmd::Text {
            x: ix,
            y: pr.y + 3,
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
        cmds.push(RenderCmd::Text {
            x: ix,
            y: pr.y + 4,
            text: format!(" Status:  {}", status_str),
            fg: status_color,
            bg,
            bold: false,
        });

        // Action items
        for (i, action) in actions.iter().enumerate() {
            let focused = i == self.action_cursor;
            let label = match action {
                Action::Enable => "Enable".into(),
                Action::Disable => "Disable".into(),
                Action::Install => "Install".into(),
                Action::Update => format!("Update to v{}", plugin.version),
                Action::Delete => "Delete".into(),
            };
            ui::palette_item(cmds, t, &pr, 6 + i as u16, &label, focused);
        }
    }
}
