use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData};
use santui_ipc::ui::{self, PanelOpts};

use crate::sampler::fmt_bytes;
use crate::state::{Screen, SortBy, SysMonState};

fn auto_color(pct: f32, theme: &ThemeData) -> [u8; 3] {
    if pct > 80.0 {
        theme.error
    } else if pct > 60.0 {
        theme.highlight
    } else {
        theme.success
    }
}

fn duration_str(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn load_avg_str(load: &[f64; 3]) -> String {
    if load[0] == 0.0 && load[1] == 0.0 && load[2] == 0.0 {
        "N/A".to_string()
    } else {
        format!("{:.2} {:.2} {:.2}", load[0], load[1], load[2])
    }
}

fn overview_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let snap = &state.snapshot;

    // ── Full-window panel with title (standard stable-plugin look) ──
    ui::draw_panel(
        &mut cmds,
        theme,
        0,
        0,
        w,
        h,
        Some("System Monitor"),
        PanelOpts::default(),
    );

    let mx: u16 = 1;
    let inner_w = w.saturating_sub(2);
    let content_x = mx + 2;
    let content_w = inner_w.saturating_sub(4);

    // ── System card: host · OS · shell · uptime · battery (2-up) ──
    let comp_y: u16 = 1;
    let host = if snap.hostname.is_empty() {
        "unknown".to_string()
    } else {
        snap.hostname.clone()
    };
    let os = if snap.os_name.is_empty() {
        "unknown OS".to_string()
    } else {
        snap.os_name.clone()
    };
    let up = duration_str(snap.uptime_secs);
    let mut rows: Vec<(String, String)> = vec![
        ("Host".into(), host),
        ("OS".into(), os),
        ("Uptime".into(), up),
    ];
    if !snap.shell.is_empty() {
        rows.push(("Shell".into(), snap.shell.clone()));
    }
    if let Some(b) = &snap.battery {
        rows.push(("Battery".into(), format!("{:.0}% ({})", b.pct, b.state)));
    }
    let comp_h: u16 = rows.len().div_ceil(2) as u16 + 2;
    ui::draw_panel(
        &mut cmds,
        theme,
        mx,
        comp_y,
        inner_w,
        comp_h,
        Some("System"),
        PanelOpts::default(),
    );
    let half_cw = content_w / 2;
    for (i, (label, value)) in rows.iter().enumerate() {
        let base_x = content_x + (i % 2) as u16 * half_cw;
        let line = (i / 2) as u16;
        let value_x = base_x + label.len() as u16 + 2;
        let value_w = half_cw.saturating_sub(value_x - base_x);
        ui::push_text(
            &mut cmds,
            base_x,
            comp_y + 1 + line,
            format!("{label}:"),
            theme.text_muted,
            false,
        );
        ui::text_at(
            &mut cmds,
            value_x,
            comp_y + 1 + line,
            value,
            theme.text,
            None,
            value_w,
        );
    }

    // ── CPU card (full width) ──
    let cpu_y: u16 = comp_y + comp_h;
    let cpu_h: u16 = 5;
    ui::draw_panel(
        &mut cmds,
        theme,
        mx,
        cpu_y,
        inner_w,
        cpu_h,
        Some("CPU"),
        PanelOpts::default(),
    );
    let half_cw = content_w / 2;
    let pairs: [(u16, &str, String); 4] = [
        (0, "CPU", format!("{:.1}%", snap.cpu.global_pct)),
        (1, "Cores", snap.cpu.core_count.to_string()),
        (0, "MHz", snap.cpu.frequency_mhz.to_string()),
        (1, "Brand", snap.cpu.brand.clone()),
    ];
    for (i, (col, label, value)) in pairs.into_iter().enumerate() {
        let base_x = if col == 0 {
            content_x
        } else {
            content_x + half_cw
        };
        let value_x = base_x + label.len() as u16 + 2;
        let value_w = half_cw.saturating_sub(value_x - base_x);
        ui::push_text(
            &mut cmds,
            base_x,
            cpu_y + 1 + (i / 2) as u16,
            format!("{label}:"),
            theme.text_muted,
            false,
        );
        ui::text_at(
            &mut cmds,
            value_x,
            cpu_y + 1 + (i / 2) as u16,
            &value,
            theme.text,
            None,
            value_w,
        );
    }
    let load_x = content_x + "Load (1/5/15m)".len() as u16 + 2;
    ui::push_text(
        &mut cmds,
        content_x,
        cpu_y + 3,
        "Load (1/5/15m):",
        theme.text_muted,
        false,
    );
    ui::text_at(
        &mut cmds,
        load_x,
        cpu_y + 3,
        &load_avg_str(&snap.load_avg),
        theme.text,
        None,
        content_w.saturating_sub(load_x - content_x),
    );

    // ── 2x2 grid below: Memory | Disk, then Network | Processes ──
    let grid_y = cpu_y + cpu_h;
    let avail = (h - 1).saturating_sub(grid_y);
    let bottom_h = (avail.saturating_sub(6)).clamp(3, 14).min(avail);
    let grid_h = avail.saturating_sub(bottom_h);
    let left_w = inner_w * 30 / 100;
    let right_w = inner_w.saturating_sub(left_w);
    let right_x = mx + left_w;
    let left_iw = left_w.saturating_sub(4);
    let right_iw = right_w.saturating_sub(4);

    // Memory card
    ui::draw_panel(
        &mut cmds,
        theme,
        mx,
        grid_y,
        left_w,
        grid_h,
        Some("Memory"),
        PanelOpts::default(),
    );
    let ram_pct = if snap.mem.ram_total > 0 {
        snap.mem.ram_used as f32 / snap.mem.ram_total as f32 * 100.0
    } else {
        0.0
    };
    let swap_pct = if snap.mem.swap_total > 0 {
        snap.mem.swap_used as f32 / snap.mem.swap_total as f32 * 100.0
    } else {
        0.0
    };
    // ── Auto-width table (proportional columns, radio-player style) ──
    if grid_h >= 4 {
        let name_w = ((left_iw - 2) * 30 / 100).max(4);
        let usage_w = ((left_iw - 2) * 15 / 100).max(6);
        let total_w = left_iw.saturating_sub(2 + name_w + usage_w);
        let header_style = TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
            modifiers: 0,
        };
        let rows: Vec<Vec<String>> = vec![
            vec![
                "RAM".into(),
                format!(
                    "{:>width$}",
                    ui::truncate(&format!("{ram_pct:.1}%"), usage_w as usize),
                    width = usage_w as usize
                ),
                format!(
                    "{:>width$}",
                    ui::truncate(
                        &format!(
                            "{} / {}",
                            fmt_bytes(snap.mem.ram_used),
                            fmt_bytes(snap.mem.ram_total)
                        ),
                        total_w as usize
                    ),
                    width = total_w as usize
                ),
            ],
            vec![
                "SWAP".into(),
                format!(
                    "{:>width$}",
                    ui::truncate(&format!("{swap_pct:.1}%"), usage_w as usize),
                    width = usage_w as usize
                ),
                format!(
                    "{:>width$}",
                    ui::truncate(
                        &format!(
                            "{} / {}",
                            fmt_bytes(snap.mem.swap_used),
                            fmt_bytes(snap.mem.swap_total)
                        ),
                        total_w as usize
                    ),
                    width = total_w as usize
                ),
            ],
        ];
        let cell_styles: Vec<Vec<Option<TextStyle>>> = vec![
            vec![
                None,
                Some(TextStyle {
                    fg: Some(auto_color(ram_pct, theme)),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                }),
                None,
            ],
            vec![
                None,
                Some(TextStyle {
                    fg: Some(auto_color(swap_pct, theme)),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                }),
                None,
            ],
        ];
        cmds.push(RenderCmd::Table {
            x: mx + 2,
            y: grid_y + 1,
            w: left_iw,
            h: grid_h.saturating_sub(2),
            header: vec![
                "Name".into(),
                format!(
                    "{:>width$}",
                    ui::truncate("Usage", usage_w as usize),
                    width = usage_w as usize
                ),
                format!(
                    "{:>width$}",
                    ui::truncate("Used / Total", total_w as usize),
                    width = total_w as usize
                ),
            ],
            header_style,
            rows,
            column_widths: vec![name_w, usage_w, total_w],
            selected: None,
            style: TextStyle {
                fg: Some(theme.text),
                bg: None,
                bold: false,
                modifiers: 0,
            },
            highlight_style: TextStyle {
                fg: Some(theme.inverted_text),
                bg: Some(theme.highlight),
                bold: false,
                modifiers: 0,
            },
            current_row: None,
            current_style: None,
            cell_styles: Some(cell_styles),
        });
    }

    // Disk card
    ui::draw_panel(
        &mut cmds,
        theme,
        right_x,
        grid_y,
        right_w,
        grid_h,
        Some("Disk"),
        PanelOpts::default(),
    );
    // ── Auto-width table (proportional columns, radio-player style) ──
    if grid_h >= 4 {
        let mount_w = ((right_iw - 2) * 45 / 100).max(6);
        let usage_w = ((right_iw - 2) * 15 / 100).max(6);
        let total_w = right_iw.saturating_sub(2 + mount_w + usage_w);
        let header_style = TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
            modifiers: 0,
        };
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut cell_styles: Vec<Vec<Option<TextStyle>>> = Vec::new();
        for disk in snap.disks.iter() {
            let pct = if disk.total > 0 {
                disk.used as f32 / disk.total as f32 * 100.0
            } else {
                0.0
            };
            let color = if pct > 90.0 {
                theme.error
            } else if pct > 70.0 {
                theme.highlight
            } else {
                theme.text
            };
            rows.push(vec![
                format!(
                    "{:<width$}",
                    ui::truncate(&disk.mount, mount_w as usize),
                    width = mount_w as usize
                ),
                format!(
                    "{:>width$}",
                    ui::truncate(&format!("{:.1}%", pct), usage_w as usize),
                    width = usage_w as usize
                ),
                format!(
                    "{:>width$}",
                    ui::truncate(
                        &format!(
                            "{} / {}",
                            fmt_size_short(disk.used),
                            fmt_size_short(disk.total)
                        ),
                        total_w as usize
                    ),
                    width = total_w as usize
                ),
            ]);
            cell_styles.push(vec![
                None,
                Some(TextStyle {
                    fg: Some(color),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                }),
                None,
            ]);
        }
        cmds.push(RenderCmd::Table {
            x: right_x + 2,
            y: grid_y + 1,
            w: right_iw,
            h: grid_h.saturating_sub(2),
            header: vec![
                "Mount".into(),
                format!(
                    "{:>width$}",
                    ui::truncate("Usage", usage_w as usize),
                    width = usage_w as usize
                ),
                format!(
                    "{:>width$}",
                    ui::truncate("Used / Total", total_w as usize),
                    width = total_w as usize
                ),
            ],
            header_style,
            rows,
            column_widths: vec![mount_w, usage_w, total_w],
            selected: None,
            style: TextStyle {
                fg: Some(theme.text),
                bg: None,
                bold: false,
                modifiers: 0,
            },
            highlight_style: TextStyle {
                fg: Some(theme.inverted_text),
                bg: Some(theme.highlight),
                bold: false,
                modifiers: 0,
            },
            current_row: None,
            current_style: None,
            cell_styles: Some(cell_styles),
        });
    } else if snap.disks.is_empty() && grid_h >= 3 {
        ui::push_text(
            &mut cmds,
            right_x + 2,
            grid_y + 1,
            "No disks",
            theme.text_muted,
            false,
        );
    }

    // Network card
    let bottom_y = grid_y + grid_h;
    ui::draw_panel(
        &mut cmds,
        theme,
        mx,
        bottom_y,
        left_w,
        bottom_h,
        Some("Network"),
        PanelOpts::default(),
    );
    if bottom_h >= 4 {
        if snap.net.is_empty() {
            ui::push_text(
                &mut cmds,
                mx + 2,
                bottom_y + 1,
                "No interfaces",
                theme.text_muted,
                false,
            );
        } else {
            let iface_w = ((left_iw - 2) * 45 / 100).max(6);
            let down_w = ((left_iw - 2) * 20 / 100).max(7);
            let up_w = left_iw.saturating_sub(2 + iface_w + down_w);
            let header_style = TextStyle {
                fg: Some(theme.text_muted),
                bg: None,
                bold: true,
                modifiers: 0,
            };
            let rows: Vec<Vec<String>> = snap
                .net
                .iter()
                .map(|iface| {
                    vec![
                        format!(
                            "{:<width$}",
                            ui::truncate(&iface.name, iface_w as usize),
                            width = iface_w as usize
                        ),
                        format!(
                            "{:>width$}",
                            ui::truncate(&fmt_bytes(iface.rx_bytes_sec), down_w as usize),
                            width = down_w as usize
                        ),
                        format!(
                            "{:>width$}",
                            ui::truncate(&fmt_bytes(iface.tx_bytes_sec), up_w as usize),
                            width = up_w as usize
                        ),
                    ]
                })
                .collect();
            cmds.push(RenderCmd::Table {
                x: mx + 2,
                y: bottom_y + 1,
                w: left_iw,
                h: bottom_h.saturating_sub(2),
                header: vec![
                    "Interface".into(),
                    format!(
                        "{:>width$}",
                        ui::truncate("↓ Speed", down_w as usize),
                        width = down_w as usize
                    ),
                    format!(
                        "{:>width$}",
                        ui::truncate("↑ Speed", up_w as usize),
                        width = up_w as usize
                    ),
                ],
                header_style,
                rows,
                column_widths: vec![iface_w, down_w, up_w],
                selected: None,
                style: TextStyle {
                    fg: Some(theme.text),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                },
                highlight_style: TextStyle {
                    fg: Some(theme.inverted_text),
                    bg: Some(theme.highlight),
                    bold: false,
                    modifiers: 0,
                },
                current_row: None,
                current_style: None,
                cell_styles: None,
            });
        }
    }

    // Processes card
    ui::draw_panel(
        &mut cmds,
        theme,
        right_x,
        bottom_y,
        right_w,
        bottom_h,
        Some(&format!("Processes · {} total", snap.total_processes)),
        PanelOpts::default(),
    );
    if bottom_h >= 3 {
        let fixed_w = 20u16;
        let show_path = right_iw.saturating_sub(fixed_w + 3) >= 20;
        let (name_w, path_w): (usize, usize) = if show_path {
            let np = right_iw.saturating_sub(fixed_w + 4);
            let n = (np * 35 / 100).max(4);
            (n as usize, np.saturating_sub(n) as usize)
        } else {
            (right_iw.saturating_sub(fixed_w + 3) as usize, 0)
        };
        let rows: Vec<Vec<String>> = snap
            .top_processes
            .iter()
            .map(|proc| {
                let mut row = vec![
                    format!("{:>6}", proc.pid),
                    format!(
                        "{:<width$}",
                        ui::truncate(&proc.name, name_w),
                        width = name_w
                    ),
                    format!("{:>5.1}%", proc.cpu_pct),
                    format!("{:>8}", fmt_bytes(proc.mem_bytes)),
                ];
                if show_path {
                    row.insert(
                        2,
                        format!(
                            "{:<width$}",
                            ui::truncate(&proc.path, path_w),
                            width = path_w
                        ),
                    );
                }
                row
            })
            .collect();
        let mut header = vec![
            format!("{:>6}", "PID"),
            format!("{:<width$}", "Name", width = name_w),
            format!("{:>6}", "CPU"),
            format!("{:>8}", "Mem"),
        ];
        let mut column_widths: Vec<u16> = vec![6, name_w as u16, 6, 8];
        if show_path {
            header.insert(2, format!("{:<width$}", "Path", width = path_w));
            column_widths.insert(2, path_w as u16);
        }
        cmds.push(RenderCmd::Table {
            x: right_x + 2,
            y: bottom_y + 1,
            w: right_iw,
            h: bottom_h.saturating_sub(2),
            header,
            header_style: TextStyle {
                fg: Some(theme.text_muted),
                bg: None,
                bold: true,
                modifiers: 0,
            },
            rows,
            column_widths,
            selected: None,
            style: TextStyle {
                fg: Some(theme.text),
                bg: None,
                bold: false,
                modifiers: 0,
            },
            highlight_style: TextStyle {
                fg: Some(theme.inverted_text),
                bg: Some(theme.highlight),
                bold: false,
                modifiers: 0,
            },
            current_row: None,
            current_style: None,
            cell_styles: None,
        });
    }

    cmds
}

fn cpu_detail_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let snap = &state.snapshot;

    ui::draw_panel(
        &mut cmds,
        theme,
        0,
        0,
        w,
        h,
        Some("CPU Detail"),
        PanelOpts::default(),
    );

    let iw = w.saturating_sub(4);
    let half_cw = iw / 2;

    let cpu_pct = format!("{:.1}%", snap.cpu.global_pct);
    let la = load_avg_str(&snap.load_avg);
    let pairs: [(u16, &str, String); 4] = [
        (0, "CPU", cpu_pct),
        (1, "Load (1/5/15m)", la),
        (0, "Cores", snap.cpu.core_count.to_string()),
        (1, "MHz", snap.cpu.frequency_mhz.to_string()),
    ];
    for (i, (col, label, value)) in pairs.into_iter().enumerate() {
        let base_x = 2 + col * half_cw;
        let value_x = base_x + label.len() as u16 + 2;
        let value_w = half_cw.saturating_sub(value_x - base_x);
        ui::push_text(
            &mut cmds,
            base_x,
            1 + (i / 2) as u16,
            format!("{label}:"),
            theme.text_muted,
            false,
        );
        ui::text_at(
            &mut cmds,
            value_x,
            1 + (i / 2) as u16,
            &value,
            theme.text,
            None,
            value_w,
        );
    }
    let brand_x = 2 + "Brand".len() as u16 + 2;
    ui::push_text(&mut cmds, 2, 3, "Brand:", theme.text_muted, false);
    ui::text_at(
        &mut cmds,
        brand_x,
        3,
        &snap.cpu.brand,
        theme.text,
        None,
        iw.saturating_sub(brand_x - 2),
    );

    // Per-core table
    let t_y = 5;
    let t_h = h.saturating_sub(t_y + 2);
    if t_h >= 1 {
        let core_w = snap
            .cpu
            .per_core
            .iter()
            .enumerate()
            .map(|(i, _)| format!("Core {i}").len())
            .max()
            .unwrap_or(6);
        let pct_w = snap
            .cpu
            .per_core
            .iter()
            .map(|p| format!("{p:.1}%").len())
            .max()
            .unwrap_or(5)
            .max("CPU %".len());
        let header_style = TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
            modifiers: 0,
        };
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut cell_styles: Vec<Vec<Option<TextStyle>>> = Vec::new();
        for (i, pct) in snap.cpu.per_core.iter().enumerate() {
            rows.push(vec![
                format!(
                    "{:<width$}",
                    ui::truncate(&format!("Core {i}"), core_w),
                    width = core_w
                ),
                format!(
                    "{:>width$}",
                    ui::truncate(&format!("{pct:.1}%"), pct_w),
                    width = pct_w
                ),
            ]);
            cell_styles.push(vec![
                None,
                Some(TextStyle {
                    fg: Some(auto_color(*pct, theme)),
                    bg: None,
                    bold: false,
                    modifiers: 0,
                }),
            ]);
        }
        if rows.is_empty() {
            rows.push(vec![
                format!("{:<width$}", "No cores", width = core_w),
                format!("{:>width$}", ui::truncate("0.0%", pct_w), width = pct_w),
            ]);
            cell_styles.push(vec![None, None]);
        }
        cmds.push(RenderCmd::Table {
            x: 2,
            y: t_y,
            w: iw,
            h: t_h,
            header: vec![
                format!("{:<width$}", "Core", width = core_w),
                format!("{:>width$}", ui::truncate("CPU %", pct_w), width = pct_w),
            ],
            header_style,
            rows,
            column_widths: vec![core_w as u16, pct_w as u16],
            selected: None,
            style: TextStyle {
                fg: Some(theme.text),
                bg: None,
                bold: false,
                modifiers: 0,
            },
            highlight_style: TextStyle {
                fg: Some(theme.inverted_text),
                bg: Some(theme.highlight),
                bold: false,
                modifiers: 0,
            },
            current_row: None,
            current_style: None,
            cell_styles: Some(cell_styles),
        });
    }

    cmds
}

fn mem_detail_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let snap = &state.snapshot;

    ui::draw_panel(
        &mut cmds,
        theme,
        0,
        0,
        w,
        h,
        Some("Memory Detail"),
        PanelOpts::default(),
    );

    let iw = w.saturating_sub(4);
    let ram_pct = if snap.mem.ram_total > 0 {
        snap.mem.ram_used as f32 / snap.mem.ram_total as f32 * 100.0
    } else {
        0.0
    };
    let swap_pct = if snap.mem.swap_total > 0 {
        snap.mem.swap_used as f32 / snap.mem.swap_total as f32 * 100.0
    } else {
        0.0
    };

    let ram_used_s = fmt_bytes(snap.mem.ram_used);
    let ram_total_s = fmt_bytes(snap.mem.ram_total);
    let swap_used_s = fmt_bytes(snap.mem.swap_used);
    let swap_total_s = fmt_bytes(snap.mem.swap_total);
    let ram_pct_s = format!("{ram_pct:.1}%");
    let swap_pct_s = format!("{swap_pct:.1}%");
    let name_w = "RAM".len().max("SWAP".len());
    let usage_w = ram_pct_s.len().max(swap_pct_s.len()).max("Usage".len());
    let used_w = ram_used_s.len().max(swap_used_s.len()).max("Used".len());
    let total_w = ram_total_s.len().max(swap_total_s.len()).max("Total".len());
    let header_style = TextStyle {
        fg: Some(theme.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    };
    let rows: Vec<Vec<String>> = vec![
        vec![
            format!("{:<width$}", "RAM", width = name_w),
            format!(
                "{:>width$}",
                ui::truncate(&ram_pct_s, usage_w),
                width = usage_w
            ),
            format!(
                "{:>width$}",
                ui::truncate(&ram_used_s, used_w),
                width = used_w
            ),
            format!(
                "{:>width$}",
                ui::truncate(&ram_total_s, total_w),
                width = total_w
            ),
        ],
        vec![
            format!("{:<width$}", "SWAP", width = name_w),
            format!(
                "{:>width$}",
                ui::truncate(&swap_pct_s, usage_w),
                width = usage_w
            ),
            format!(
                "{:>width$}",
                ui::truncate(&swap_used_s, used_w),
                width = used_w
            ),
            format!(
                "{:>width$}",
                ui::truncate(&swap_total_s, total_w),
                width = total_w
            ),
        ],
    ];
    let cell_styles: Vec<Vec<Option<TextStyle>>> = vec![
        vec![
            None,
            Some(TextStyle {
                fg: Some(auto_color(ram_pct, theme)),
                bg: None,
                bold: false,
                modifiers: 0,
            }),
            None,
            None,
        ],
        vec![
            None,
            Some(TextStyle {
                fg: Some(auto_color(swap_pct, theme)),
                bg: None,
                bold: false,
                modifiers: 0,
            }),
            None,
            None,
        ],
    ];
    cmds.push(RenderCmd::Table {
        x: 2,
        y: 1,
        w: iw,
        h: h.saturating_sub(3),
        header: vec![
            format!("{:<width$}", "Memory", width = name_w),
            format!(
                "{:>width$}",
                ui::truncate("Usage", usage_w),
                width = usage_w
            ),
            format!("{:>width$}", ui::truncate("Used", used_w), width = used_w),
            format!(
                "{:>width$}",
                ui::truncate("Total", total_w),
                width = total_w
            ),
        ],
        header_style,
        rows,
        column_widths: vec![name_w as u16, usage_w as u16, used_w as u16, total_w as u16],
        selected: None,
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: false,
            modifiers: 0,
        },
        current_row: None,
        current_style: None,
        cell_styles: Some(cell_styles),
    });

    cmds
}

fn fmt_size_short(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{bytes}{}", UNITS[unit_idx])
    } else {
        format!("{:.1}{}", size, UNITS[unit_idx])
    }
}

fn disk_detail_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    ui::draw_panel(
        &mut cmds,
        theme,
        0,
        0,
        w,
        h,
        Some("Disk Detail"),
        PanelOpts::default(),
    );

    let iw = w.saturating_sub(4);
    let header_style = TextStyle {
        fg: Some(theme.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    };

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut cell_styles: Vec<Vec<Option<TextStyle>>> = Vec::new();
    let mut mount_w = "Mount".len();
    let mut dev_w = "Device".len();
    let mut fs_w = "FS".len();
    let mut used_w = "Used".len();
    let mut total_w = "Total".len();
    let mut usage_w = "Usage".len();
    for d in &state.snapshot.disks {
        mount_w = mount_w.max(d.mount.len());
        dev_w = dev_w.max(d.name.len());
        fs_w = fs_w.max(d.fs.len());
        used_w = used_w.max(fmt_size_short(d.used).len());
        total_w = total_w.max(fmt_size_short(d.total).len());
        usage_w = usage_w.max(
            format!(
                "{:.1}%",
                if d.total > 0 {
                    d.used as f32 / d.total as f32 * 100.0
                } else {
                    0.0
                }
            )
            .len(),
        );
    }
    for d in &state.snapshot.disks {
        let pct = if d.total > 0 {
            d.used as f32 / d.total as f32 * 100.0
        } else {
            0.0
        };
        let color = if pct > 90.0 {
            theme.error
        } else if pct > 70.0 {
            theme.highlight
        } else {
            theme.success
        };
        let mount_s = d.mount.clone();
        let name_s = d.name.clone();
        let fs_s = d.fs.clone();
        let used_s = fmt_size_short(d.used);
        let total_s = fmt_size_short(d.total);
        let usage_s = format!("{pct:.1}%");
        mount_w = mount_w.max(mount_s.len());
        dev_w = dev_w.max(name_s.len());
        fs_w = fs_w.max(fs_s.len());
        used_w = used_w.max(used_s.len());
        total_w = total_w.max(total_s.len());
        usage_w = usage_w.max(usage_s.len());
        rows.push(vec![
            format!(
                "{:<width$}",
                ui::truncate(&mount_s, mount_w),
                width = mount_w
            ),
            format!("{:<width$}", ui::truncate(&name_s, dev_w), width = dev_w),
            format!("{:<width$}", ui::truncate(&fs_s, fs_w), width = fs_w),
            format!("{:>width$}", ui::truncate(&used_s, used_w), width = used_w),
            format!(
                "{:>width$}",
                ui::truncate(&total_s, total_w),
                width = total_w
            ),
            format!(
                "{:>width$}",
                ui::truncate(&usage_s, usage_w),
                width = usage_w
            ),
        ]);
        cell_styles.push(vec![
            None,
            None,
            None,
            None,
            None,
            Some(TextStyle {
                fg: Some(color),
                bg: None,
                bold: false,
                modifiers: 0,
            }),
        ]);
    }
    if rows.is_empty() {
        rows.push(vec![
            format!("{:<width$}", "No disks", width = mount_w),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ]);
        cell_styles.push(vec![None; 6]);
    }

    cmds.push(RenderCmd::Table {
        x: 2,
        y: 1,
        w: iw,
        h: h.saturating_sub(3),
        header: vec![
            format!("{:<width$}", "Mount", width = mount_w),
            format!("{:<width$}", "Device", width = dev_w),
            format!("{:<width$}", "FS", width = fs_w),
            format!("{:>width$}", ui::truncate("Used", used_w), width = used_w),
            format!(
                "{:>width$}",
                ui::truncate("Total", total_w),
                width = total_w
            ),
            format!(
                "{:>width$}",
                ui::truncate("Usage", usage_w),
                width = usage_w
            ),
        ],
        header_style,
        rows,
        column_widths: vec![
            mount_w as u16,
            dev_w as u16,
            fs_w as u16,
            used_w as u16,
            total_w as u16,
            usage_w as u16,
        ],
        selected: None,
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: false,
            modifiers: 0,
        },
        current_row: None,
        current_style: None,
        cell_styles: Some(cell_styles),
    });

    cmds
}

fn net_detail_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    ui::draw_panel(
        &mut cmds,
        theme,
        0,
        0,
        w,
        h,
        Some("Network Detail"),
        PanelOpts::default(),
    );

    let iw = w.saturating_sub(4);
    let header_style = TextStyle {
        fg: Some(theme.text_muted),
        bg: None,
        bold: true,
        modifiers: 0,
    };

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut iface_w = "Interface".len();
    let mut down_w = "↓ Speed".len();
    let mut up_w = "↑ Speed".len();
    let mut rx_w = "Total ↓".len();
    let mut tx_w = "Total ↑".len();
    for n in &state.snapshot.net {
        iface_w = iface_w.max(n.name.len());
        down_w = down_w.max(format!("{}/s", fmt_bytes(n.rx_bytes_sec)).len());
        up_w = up_w.max(format!("{}/s", fmt_bytes(n.tx_bytes_sec)).len());
        rx_w = rx_w.max(fmt_bytes(n.total_rx).len());
        tx_w = tx_w.max(fmt_bytes(n.total_tx).len());
    }
    for n in &state.snapshot.net {
        let down_s = format!("{}/s", fmt_bytes(n.rx_bytes_sec));
        let up_s = format!("{}/s", fmt_bytes(n.tx_bytes_sec));
        let rx_s = fmt_bytes(n.total_rx);
        let tx_s = fmt_bytes(n.total_tx);
        iface_w = iface_w.max(n.name.len());
        down_w = down_w.max(down_s.len());
        up_w = up_w.max(up_s.len());
        rx_w = rx_w.max(rx_s.len());
        tx_w = tx_w.max(tx_s.len());
        rows.push(vec![
            format!(
                "{:<width$}",
                ui::truncate(&n.name, iface_w),
                width = iface_w
            ),
            format!("{:>width$}", ui::truncate(&down_s, down_w), width = down_w),
            format!("{:>width$}", ui::truncate(&up_s, up_w), width = up_w),
            format!("{:>width$}", ui::truncate(&rx_s, rx_w), width = rx_w),
            format!("{:>width$}", ui::truncate(&tx_s, tx_w), width = tx_w),
        ]);
    }
    if rows.is_empty() {
        rows.push(vec![
            format!("{:<width$}", "No interfaces", width = iface_w),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ]);
    }

    cmds.push(RenderCmd::Table {
        x: 2,
        y: 1,
        w: iw,
        h: h.saturating_sub(3),
        header: vec![
            format!("{:<width$}", "Interface", width = iface_w),
            format!(
                "{:>width$}",
                ui::truncate("↓ Speed", down_w),
                width = down_w
            ),
            format!("{:>width$}", ui::truncate("↑ Speed", up_w), width = up_w),
            format!("{:>width$}", ui::truncate("Total ↓", rx_w), width = rx_w),
            format!("{:>width$}", ui::truncate("Total ↑", tx_w), width = tx_w),
        ],
        header_style,
        rows,
        column_widths: vec![
            iface_w as u16,
            down_w as u16,
            up_w as u16,
            rx_w as u16,
            tx_w as u16,
        ],
        selected: None,
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: false,
            modifiers: 0,
        },
        current_row: None,
        current_style: None,
        cell_styles: None,
    });

    cmds
}

fn process_list_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    let sort_label = match state.process_sort {
        SortBy::Cpu => "CPU % ▼",
        SortBy::Memory => "Memory ▼",
        SortBy::Name => "Name ▼",
    };

    let title = format!(
        " Processes ({} total · top {} by {}) ",
        state.snapshot.total_processes,
        crate::state::TOP_PROCESSES,
        sort_label
    );

    ui::draw_panel(
        &mut cmds,
        theme,
        0,
        0,
        w,
        h,
        Some(&title),
        PanelOpts::default(),
    );

    let iw = w.saturating_sub(2);
    let fixed_w = 8 + 7 + 12 + 4; // PID + CPU % + Memory + 4 column spacings
    let name_path_w = iw.saturating_sub(fixed_w);
    let name_w = (name_path_w * 30 / 100).max(8) as usize;
    let path_w = name_path_w.saturating_sub(name_w as u16) as usize;

    let header = vec![
        format!("{:>8}", "PID"),
        format!("{:<width$}", "Name", width = name_w),
        format!("{:<width$}", "Path", width = path_w),
        format!("{:>7}", "CPU %"),
        format!("{:>12}", "Memory"),
    ];

    let rows: Vec<Vec<String>> = state
        .snapshot
        .top_processes
        .iter()
        .map(|p| {
            vec![
                format!("{:>8}", p.pid),
                format!("{:<width$}", ui::truncate(&p.name, name_w), width = name_w),
                format!("{:<width$}", ui::truncate(&p.path, path_w), width = path_w),
                format!("{:>7}", format!("{:.1}%", p.cpu_pct)),
                format!("{:>12}", fmt_bytes(p.mem_bytes)),
            ]
        })
        .collect();

    let selected = (!rows.is_empty()).then_some(state.selected_process.min(rows.len() - 1));

    cmds.push(RenderCmd::Table {
        x: 1,
        y: 1,
        w: iw,
        h: h.saturating_sub(2),
        header,
        header_style: TextStyle {
            fg: Some(theme.text_muted),
            bg: None,
            bold: true,
            modifiers: 0,
        },
        rows,
        column_widths: vec![8, name_w as u16, path_w as u16, 7, 12],
        selected,
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
            modifiers: 0,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: false,
            modifiers: 0,
        },
        current_row: None,
        current_style: None,
        cell_styles: None,
    });

    cmds
}

pub fn render_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = vec![RenderCmd::Clear {
        x: 0,
        y: 0,
        w: 4096,
        h: 4096,
    }];

    let screen_cmds = match state.screen {
        Screen::Overview => overview_ui(state, theme, w, h),
        Screen::CpuDetail => cpu_detail_ui(state, theme, w, h),
        Screen::MemDetail => mem_detail_ui(state, theme, w, h),
        Screen::DiskDetail => disk_detail_ui(state, theme, w, h),
        Screen::NetDetail => net_detail_ui(state, theme, w, h),
        Screen::ProcessList => process_list_ui(state, theme, w, h),
    };
    cmds.extend(screen_cmds);

    cmds
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_theme() -> ThemeData {
        ThemeData {
            text: [200; 3],
            text_muted: [100; 3],
            accent: [180; 3],
            highlight: [220; 3],
            logo: [255; 3],
            background: [0; 3],
            background_panel: [20; 3],
            background_overlay: [10; 3],
            border: [150; 3],
            success: [0; 3],
            error: [255; 3],
            inverted_text: [255; 3],
        }
    }

    #[test]
    fn overview_renders_full_window_title() {
        let state = SysMonState::default();
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        let has_title = cmds.iter().any(|c| match c {
            RenderCmd::Border { title, .. } => title.as_deref() == Some("System Monitor"),
            _ => false,
        });
        assert!(has_title);
    }

    #[test]
    fn processes_table_fills_panel_interior_at_tall_window() {
        let mut state = SysMonState::default();
        state.snapshot.total_processes = 123;
        for i in 0..crate::state::TOP_PROCESSES {
            state
                .snapshot
                .top_processes
                .push(crate::state::ProcessSnapshot {
                    pid: 1000 + i as u32,
                    name: format!("proc-{i}"),
                    path: String::new(),
                    cpu_pct: 1.0,
                    mem_bytes: 1024,
                });
        }
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 34);
        let processes_panel = cmds
            .iter()
            .find_map(|c| match c {
                RenderCmd::Border {
                    title, x, y, w, h, ..
                } if title.as_deref().is_some_and(|t| t.starts_with("Processes")) => {
                    Some((*x, *y, *w, *h))
                }
                _ => None,
            })
            .expect("Processes panel");
        let table = cmds
            .iter()
            .find_map(|c| match c {
                RenderCmd::Table {
                    y, h, rows, header, ..
                } if *y == processes_panel.1 + 1 => Some((*y, *h, rows.len(), header.len())),
                _ => None,
            })
            .expect("Processes table");
        let interior = processes_panel.3 - 2;
        assert_eq!(
            table.1, interior,
            "table height should fill the panel interior"
        );
        assert!(
            table.2 + table.3 >= table.1 as usize,
            "rows + header should cover the full interior ({} rows + {} header < {} interior)",
            table.2,
            table.3,
            table.1
        );
    }

    #[test]
    fn processes_card_shows_path_column_at_wide_window() {
        let mut state = SysMonState::default();
        state
            .snapshot
            .top_processes
            .push(crate::state::ProcessSnapshot {
                pid: 12345,
                name: "firefox-esr".into(),
                path: "/usr/lib/firefox-esr/firefox-esr".into(),
                cpu_pct: 12.3,
                mem_bytes: 1_200_000_000,
            });
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 120, 30);
        let table = cmds
            .iter()
            .find_map(|c| match c {
                RenderCmd::Table {
                    header,
                    rows,
                    column_widths,
                    ..
                } if header.iter().any(|h| h.trim() == "Path") => {
                    Some((header, rows, column_widths))
                }
                _ => None,
            })
            .expect("Processes table with Path column");
        assert_eq!(table.0.len(), 5, "expected 5 columns with Path");
        assert_eq!(table.2.len(), 5, "expected 5 column widths");
        let path_cell = &table.1[0][2];
        assert!(
            path_cell.trim().starts_with("/usr/lib"),
            "path cell should contain the truncated path, got {path_cell:?}"
        );
        let widths_sum: u16 = table.2.iter().sum();
        assert!(
            widths_sum + 4 <= 79,
            "columns + 4 spacings should fit the card width, sum={widths_sum}"
        );
    }

    #[test]
    fn processes_card_hides_path_column_at_narrow_window() {
        let mut state = SysMonState::default();
        state
            .snapshot
            .top_processes
            .push(crate::state::ProcessSnapshot {
                pid: 12345,
                name: "firefox-esr".into(),
                path: "/usr/lib/firefox-esr/firefox-esr".into(),
                cpu_pct: 12.3,
                mem_bytes: 1_200_000_000,
            });
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 60, 24);
        let has_path_header = cmds.iter().any(|c| match c {
            RenderCmd::Table { header, .. } => header.iter().any(|h| h.trim() == "Path"),
            _ => false,
        });
        assert!(
            !has_path_header,
            "Path column should be hidden at narrow width"
        );
    }

    #[test]
    fn process_list_detail_has_path_column() {
        let mut state = SysMonState::default();
        state
            .snapshot
            .top_processes
            .push(crate::state::ProcessSnapshot {
                pid: 12345,
                name: "firefox-esr".into(),
                path: "/usr/lib/firefox-esr/firefox-esr".into(),
                cpu_pct: 12.3,
                mem_bytes: 1_200_000_000,
            });
        let theme = test_theme();
        let cmds = process_list_ui(&state, &theme, 100, 24);
        let table = cmds
            .iter()
            .find_map(|c| match c {
                RenderCmd::Table { header, rows, .. } => Some((header, rows)),
                _ => None,
            })
            .expect("Process list table");
        assert_eq!(table.0.len(), 5, "expected 5 columns with Path");
        assert!(
            table.0.iter().any(|h| h.trim() == "Path"),
            "missing Path header"
        );
        assert_eq!(table.1[0][2].trim(), "/usr/lib/firefox-esr/firefox-esr");
    }

    #[test]
    fn overview_renders_cpu_pct_row() {
        let mut state = SysMonState::default();
        state.snapshot.cpu.brand = "Test CPU".into();
        state.snapshot.cpu.core_count = 4;
        state.snapshot.cpu.global_pct = 50.0;
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        let has_row = cmds.iter().any(|c| match c {
            RenderCmd::Text { text, .. } => text == "CPU:",
            _ => false,
        });
        let has_pct = cmds.iter().any(|c| match c {
            RenderCmd::Text { text, .. } => text == "50.0%",
            _ => false,
        });
        assert!(has_row, "missing CPU row label");
        assert!(has_pct, "missing CPU percentage value");
    }

    #[test]
    fn overview_renders_hostname() {
        let mut state = SysMonState::default();
        state.snapshot.hostname = "testhost".into();
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        let has_hostname = cmds.iter().any(|c| match c {
            RenderCmd::Text { text, .. } => text.contains("testhost"),
            _ => false,
        });
        assert!(has_hostname);
    }

    #[test]
    fn overview_renders_battery_row_when_present() {
        let mut state = SysMonState::default();
        state.snapshot.battery = Some(crate::state::BatterySnapshot {
            pct: 78.4,
            state: "Discharging".into(),
        });
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        let has_label = cmds.iter().any(|c| match c {
            RenderCmd::Text { text, .. } => text == "Battery:",
            _ => false,
        });
        let has_value = cmds.iter().any(|c| match c {
            RenderCmd::Text { text, .. } => text == "78% (Discharging)",
            _ => false,
        });
        assert!(has_label, "missing Battery label");
        assert!(has_value, "missing battery value");
    }

    #[test]
    fn overview_hides_battery_row_when_absent() {
        let state = SysMonState::default();
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        let has_label = cmds.iter().any(|c| match c {
            RenderCmd::Text { text, .. } => text == "Battery:",
            _ => false,
        });
        assert!(!has_label, "Battery row should be hidden without battery");
    }

    #[test]
    fn overview_renders_shell_row_when_present() {
        let mut state = SysMonState::default();
        state.snapshot.shell = "zsh".into();
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        let has_label = cmds.iter().any(|c| match c {
            RenderCmd::Text { text, .. } => text == "Shell:",
            _ => false,
        });
        let has_value = cmds.iter().any(|c| match c {
            RenderCmd::Text { text, .. } => text == "zsh",
            _ => false,
        });
        assert!(has_label, "missing Shell label");
        assert!(has_value, "missing shell value");
    }

    #[test]
    fn overview_hides_shell_row_when_absent() {
        let state = SysMonState::default();
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        let has_label = cmds.iter().any(|c| match c {
            RenderCmd::Text { text, .. } => text == "Shell:",
            _ => false,
        });
        assert!(!has_label, "Shell row should be hidden without shell");
    }

    #[test]
    fn overview_renders_inner_panels() {
        let state = SysMonState::default();
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        for title in [
            "System",
            "CPU",
            "Memory",
            "Disk",
            "Network",
            "Processes · 0 total",
        ] {
            let has = cmds.iter().any(|c| match c {
                RenderCmd::Border { title: t, .. } => t.as_deref() == Some(title),
                _ => false,
            });
            assert!(has, "missing inner panel {title}");
        }
    }

    #[test]
    fn overview_panels_fit_in_window() {
        let state = SysMonState::default();
        let theme = test_theme();
        for (w, h) in [(80, 24), (120, 30), (60, 20), (100, 14)] {
            let cmds = overview_ui(&state, &theme, w, h);
            let panels: Vec<&RenderCmd> = cmds
                .iter()
                .filter(|c| matches!(c, RenderCmd::Border { title: Some(_), .. }))
                .collect();
            for c in &panels {
                if let RenderCmd::Border {
                    x, y, w: pw, h: ph, ..
                } = c
                {
                    assert!(
                        *y + *ph <= h && *x + *pw <= w,
                        "panel out of bounds at {w}x{h}: y={y} h={ph} x={x} w={pw}"
                    );
                }
            }
        }
        let cmds = overview_ui(&state, &theme, 80, 24);
        let count = cmds
            .iter()
            .filter(|c| matches!(c, RenderCmd::Border { title: Some(_), .. }))
            .count();
        assert_eq!(count, 7, "expected 7 titled panels at 80x24");
    }

    #[test]
    fn overview_content_stays_inside_panels() {
        let mut state = SysMonState::default();
        state.snapshot.cpu.core_count = 16;
        state.snapshot.cpu.frequency_mhz = 3600;
        state.snapshot.cpu.brand = "AMD Ryzen 9 7950X 16-Core Processor".into();
        state.snapshot.cpu.global_pct = 62.3;
        state.snapshot.mem.ram_total = 16_000_000_000;
        state.snapshot.mem.ram_used = 8_000_000_000;
        state.snapshot.mem.swap_total = 8_000_000_000;
        state.snapshot.mem.swap_used = 2_000_000_000;
        state.snapshot.hostname = "testhost".into();
        state.snapshot.os_name = "Ubuntu 24.04 LTS".into();
        state.snapshot.uptime_secs = 3 * 86400 + 4 * 3600 + 12 * 60;
        state.snapshot.load_avg = [2.1, 1.8, 1.6];
        state.snapshot.disks.push(crate::state::DiskSnapshot {
            name: "/dev/nvme0n1p2".into(),
            mount: "/".into(),
            used: 100,
            total: 200,
            fs: "ext4".into(),
        });
        state.snapshot.net.push(crate::state::NetSnapshot {
            name: "eth0".into(),
            rx_bytes_sec: 1_200_000,
            tx_bytes_sec: 340_000,
            total_rx: 10_000,
            total_tx: 5_000,
        });
        state
            .snapshot
            .top_processes
            .push(crate::state::ProcessSnapshot {
                pid: 12345,
                name: "firefox-esr".into(),
                path: String::new(),
                cpu_pct: 12.3,
                mem_bytes: 1_200_000_000,
            });
        let theme = test_theme();
        for (w, h) in [(80, 24), (120, 30), (60, 20), (100, 14), (80, 10)] {
            let cmds = overview_ui(&state, &theme, w, h);
            let panels: Vec<(u16, u16, u16, u16)> = cmds
                .iter()
                .filter_map(|c| match c {
                    RenderCmd::Border {
                        x, y, w: pw, h: ph, ..
                    } => Some((*x, *y, *pw, *ph)),
                    _ => None,
                })
                .collect();
            for c in &cmds {
                if let RenderCmd::Text { x, y, text, .. } = c {
                    let x_end = x + text.chars().count() as u16;
                    let inside = panels.iter().any(|(px, py, pw, ph)| {
                        *x > *px && x_end < px + pw && *y > *py && *y < *py + *ph - 1
                    });
                    assert!(
                        inside,
                        "text {text:?} ({len}) at ({x},{y}) x_end={x_end} crosses panel boundary at {w}x{h}; panels: {panels:?}",
                        len = text.chars().count(),
                    );
                }
            }
        }
    }

    #[test]
    fn overview_values_not_truncated_at_130x23() {
        let mut state = SysMonState::default();
        state.snapshot.cpu.core_count = 8;
        state.snapshot.cpu.frequency_mhz = 3600;
        state.snapshot.cpu.brand = "AMD Ryzen 9 7950X 16-Core Processor".into();
        state.snapshot.cpu.global_pct = 62.3;
        state.snapshot.mem.ram_total = 16_000_000_000;
        state.snapshot.mem.ram_used = 8_000_000_000;
        state.snapshot.mem.swap_total = 8_000_000_000;
        state.snapshot.mem.swap_used = 2_000_000_000;
        state.snapshot.hostname = "testhost".into();
        state.snapshot.os_name = "Ubuntu 24.04 LTS".into();
        state.snapshot.uptime_secs = 3 * 86400 + 4 * 3600 + 12 * 60;
        state.snapshot.load_avg = [2.1, 1.8, 1.6];
        state.snapshot.disks.push(crate::state::DiskSnapshot {
            name: "/dev/nvme0n1p2".into(),
            mount: "/".into(),
            used: 100,
            total: 200,
            fs: "ext4".into(),
        });
        state.snapshot.net.push(crate::state::NetSnapshot {
            name: "eth0".into(),
            rx_bytes_sec: 1_200_000,
            tx_bytes_sec: 340_000,
            total_rx: 10_000,
            total_tx: 5_000,
        });
        state
            .snapshot
            .top_processes
            .push(crate::state::ProcessSnapshot {
                pid: 12345,
                name: "firefox-esr".into(),
                path: String::new(),
                cpu_pct: 12.3,
                mem_bytes: 1_200_000_000,
            });
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 130, 23);
        let text_all: String = cmds
            .iter()
            .flat_map(|c| match c {
                RenderCmd::Text { text, .. } => vec![text.as_str()],
                RenderCmd::Table { header, rows, .. } => {
                    let mut out: Vec<&str> = header.iter().map(String::as_str).collect();
                    out.extend(rows.iter().flat_map(|r| r.iter().map(String::as_str)));
                    out
                }
                _ => Vec::new(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text_all.contains("testhost"), "hostname truncated");
        assert!(text_all.contains("3d 4h 12m"), "uptime truncated");
        let host_label_x = cmds
            .iter()
            .find_map(|c| match c {
                RenderCmd::Text { x, y, text, .. } if text == "Host:" => Some(*x),
                _ => None,
            })
            .expect("Host label");
        let host_value_x = cmds
            .iter()
            .find_map(|c| match c {
                RenderCmd::Text { x, text, .. } if text == "testhost" => Some(*x),
                _ => None,
            })
            .expect("Host value");
        assert_eq!(
            host_value_x,
            host_label_x + "Host:".len() as u16 + 1,
            "one blank after colon"
        );
        assert!(text_all.contains("7.5 GB / 14.9 GB"), "ram sizes truncated");
        assert!(text_all.contains("1.9 GB / 7.5 GB"), "swap sizes truncated");
        assert!(text_all.contains("62.3%"), "cpu percent truncated");
        assert!(text_all.contains("2.10 1.80 1.60"), "load avg truncated");
        assert!(text_all.contains("firefox-esr"), "process name truncated");
    }

    #[test]
    fn cpu_detail_renders_per_core_rows() {
        let mut state = SysMonState::default();
        state.snapshot.cpu.core_count = 4;
        state.snapshot.cpu.per_core = vec![10.0, 20.0, 30.0, 40.0];
        let theme = test_theme();
        let cmds = cpu_detail_ui(&state, &theme, 80, 24);
        let has_table = cmds.iter().any(|c| matches!(c, RenderCmd::Table { .. }));
        let core_row = cmds.iter().any(|c| {
            if let RenderCmd::Table { rows, .. } = c {
                rows.iter().any(|row| row[0].contains("Core 0"))
            } else {
                false
            }
        });
        assert!(has_table);
        assert!(core_row);
    }

    #[test]
    fn disk_detail_renders_table() {
        let mut state = SysMonState::default();
        state.snapshot.disks.push(crate::state::DiskSnapshot {
            name: "/dev/sda1".into(),
            mount: "/".into(),
            used: 100,
            total: 200,
            fs: "ext4".into(),
        });
        let theme = test_theme();
        let cmds = disk_detail_ui(&state, &theme, 80, 24);
        let has_table = cmds.iter().any(|c| matches!(c, RenderCmd::Table { .. }));
        assert!(has_table);
    }

    #[test]
    fn net_detail_renders_table() {
        let mut state = SysMonState::default();
        state.snapshot.net.push(crate::state::NetSnapshot {
            name: "en0".into(),
            rx_bytes_sec: 1000,
            tx_bytes_sec: 500,
            total_rx: 10000,
            total_tx: 5000,
        });
        let theme = test_theme();
        let cmds = net_detail_ui(&state, &theme, 80, 24);
        let has_table = cmds.iter().any(|c| matches!(c, RenderCmd::Table { .. }));
        assert!(has_table);
    }

    #[test]
    fn process_list_renders_table() {
        let mut state = SysMonState::default();
        state
            .snapshot
            .top_processes
            .push(crate::state::ProcessSnapshot {
                pid: 1234,
                name: "test".into(),
                path: String::new(),
                cpu_pct: 10.0,
                mem_bytes: 1000,
            });
        let theme = test_theme();
        let cmds = process_list_ui(&state, &theme, 80, 24);
        let has_table = cmds.iter().any(|c| matches!(c, RenderCmd::Table { .. }));
        assert!(has_table);
    }
}
