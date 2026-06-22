use santui_ipc::protocol::RenderCmd;

use super::state::App;

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
                ("Enter".into(), "Install/Toggle".into()),
                ("Esc".into(), "Back".into()),
            ]
        } else {
            vec![
                ("↑↓".into(), "Navigate".into()),
                ("Enter".into(), "Install/Toggle".into()),
                ("d".into(), "Details".into()),
            ]
        }
    }

    pub(super) fn render_commands(&self) -> Vec<RenderCmd> {
        let mut cmds = Vec::new();

        if let Some(detail_idx) = self.detail_idx {
            self.render_detail(detail_idx, &mut cmds);
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
        let inner_w = aw.saturating_sub(4) as usize;

        santui_ipc::ui::draw_panel(cmds, t, 0, 0, aw, ah, "Plugins");

        let status_x = aw.saturating_sub(self.status.len() as u16 + 1);
        cmds.push(RenderCmd::Text {
            x: status_x,
            y: 0,
            text: self.status.clone(),
            fg: Some(t.text_muted),
            bg: Some(t.background_panel),
            bold: false,
        });

        let status_w: usize = 7;
        let ver_w: usize = 10;
        let act_w: usize = 7;
        let rem = inner_w.saturating_sub(status_w + ver_w + act_w + 4);
        let name_w = (rem * 3 / 10).max(5);
        let desc_w = rem.saturating_sub(name_w);
        let sep = " ";

        let hdr = format!(
            "{:<sw$}{sep}{:<nw$}{sep}{:<dw$}{sep}{:>vw$}{sep}{:<aw$}",
            "Status",
            "Name",
            "Description",
            "Version",
            "Action",
            sw = status_w,
            nw = name_w,
            dw = desc_w,
            vw = ver_w,
            aw = act_w,
            sep = sep
        );
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 2,
            text: hdr,
            fg: Some(t.text_muted),
            bg: Some(t.background_panel),
            bold: true,
        });

        let sep_line = format!("{:_<iw$}", "", iw = inner_w.saturating_sub(1));
        cmds.push(RenderCmd::Text {
            x: 2,
            y: 3,
            text: sep_line,
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
                y: 4,
                text: bar,
                fg: Some(t.accent),
                bg: Some(t.background_panel),
                bold: false,
            });
        }

        if let Some(ref reg) = self.registry {
            if reg.available.is_empty() {
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: 5,
                    text: "No plugins available".into(),
                    fg: self.fg(t.text_muted),
                    bg: self.bg(),
                    bold: false,
                });
                return;
            }

            let list_top = if has_progress { 6u16 } else { 5u16 };
            let list_h = max_list_h(ah) as usize;
            for i in 0..list_h.min(reg.available.len()) {
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
                let hovered = idx == self.cursor;

                let status = if is_enabled {
                    "ON"
                } else if is_installed {
                    "OFF"
                } else {
                    "--"
                };

                let name_s = if plugin.name.len() > name_w {
                    format!("{}…", &plugin.name[..name_w.saturating_sub(1)])
                } else {
                    format!("{:<nw$}", plugin.name, nw = name_w)
                };
                let desc_s = if plugin.description.len() > desc_w {
                    format!("{}…", &plugin.description[..desc_w.saturating_sub(1)])
                } else {
                    format!("{:<dw$}", plugin.description, dw = desc_w)
                };
                let ver_s = if plugin.version.len() > ver_w {
                    format!("{}…", &plugin.version[..ver_w.saturating_sub(1)])
                } else {
                    format!("{:>vw$}", plugin.version, vw = ver_w)
                };

                let action = if !is_installed {
                    "Install"
                } else if reg.installed.iter().any(|p| {
                    p.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim_end_matches(".exe"))
                        == Some(&plugin.id)
                        && p.version != plugin.version
                }) {
                    "Update"
                } else {
                    ""
                };
                let act_s = if action.len() > act_w {
                    format!("{}…", &action[..act_w.saturating_sub(1)])
                } else {
                    format!("{:<aw$}", action, aw = act_w)
                };

                let row_y = list_top + i as u16;
                let row_fg = if hovered { t.inverted_text } else { t.text };
                let line = format!(
                    "{:<sw$}{sep}{:<nw$}{sep}{:<dw$}{sep}{:>vw$}{sep}{:<aw$}",
                    status,
                    name_s,
                    desc_s,
                    ver_s,
                    act_s,
                    sw = status_w,
                    nw = name_w,
                    dw = desc_w,
                    vw = ver_w,
                    aw = act_w,
                    sep = sep
                );
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: row_y,
                    text: line,
                    fg: self.fg(row_fg),
                    bg: if hovered {
                        Some(t.highlight)
                    } else {
                        self.bg()
                    },
                    bold: hovered,
                });
            }
        }
    }

    fn render_detail(&self, detail_idx: usize, cmds: &mut Vec<RenderCmd>) {
        let t = &self.theme;
        let aw = self.area.w;
        let ah = self.area.h;
        if aw < 10 || ah < 3 {
            return;
        }

        santui_ipc::ui::draw_panel(cmds, t, 0, 0, aw, ah, "Plugin Details");

        if let Some(ref reg) = self.registry {
            if let Some(plugin) = reg.available.get(detail_idx) {
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

                let status_str = if is_enabled {
                    "Enabled"
                } else if is_installed {
                    "Disabled"
                } else {
                    "Not installed"
                };

                let y_base = 2u16;
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base,
                    text: format!(" {}", &plugin.name),
                    fg: Some(t.text),
                    bg: Some(t.background_panel),
                    bold: true,
                });

                let desc_w = aw.saturating_sub(4) as usize;
                let desc = if plugin.description.len() > desc_w {
                    format!("{}…", &plugin.description[..desc_w.saturating_sub(1)])
                } else {
                    plugin.description.clone()
                };
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 2,
                    text: desc,
                    fg: Some(t.text),
                    bg: Some(t.background_panel),
                    bold: false,
                });

                let field_fg = Some(t.text_muted);
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 4,
                    text: format!(" ID:      {}", &plugin.id),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 5,
                    text: format!(" Version: {}", &plugin.version),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 6,
                    text: format!(" Size:    {} bytes", &plugin.size),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: y_base + 7,
                    text: format!(" Status:  {}", status_str),
                    fg: if is_enabled {
                        Some(t.success)
                    } else {
                        field_fg
                    },
                    bg: Some(t.background_panel),
                    bold: false,
                });

                let hint_y = y_base + 9;
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: hint_y,
                    text: " [Enter] Install / Toggle".into(),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
                cmds.push(RenderCmd::Text {
                    x: 2,
                    y: hint_y + 1,
                    text: " [Esc]   Back to list".into(),
                    fg: field_fg,
                    bg: Some(t.background_panel),
                    bold: false,
                });
            }
        }
    }
}
