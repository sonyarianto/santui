use std::collections::VecDeque;

use santui_ipc::protocol::{RenderCmd, TextStyle, ThemeData, BORDER_ALL};

use crate::sampler::fmt_bytes;
use crate::state::{Screen, SortBy, SysMonState};

const BAR_BLOCK: char = '█';
const BAR_EMPTY: char = '░';

fn auto_color(pct: f32, theme: &ThemeData) -> [u8; 3] {
    if pct > 80.0 {
        theme.error
    } else if pct > 60.0 {
        theme.highlight
    } else {
        theme.success
    }
}

pub fn render_bar(
    label: &str,
    pct: f32,
    width: u16,
    x: u16,
    y: u16,
    theme: &ThemeData,
    color: Option<[u8; 3]>,
) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let bar_color = color.unwrap_or_else(|| auto_color(pct, theme));

    let (label_part, bar_offset) = if label.is_empty() {
        (String::new(), 0)
    } else {
        (format!("{} ", label), (label.len() + 1) as u16)
    };
    let suffix = format!("  {:.1}%", pct);
    let label_w = label_part.len() as u16;
    let suffix_w = suffix.len() as u16;
    let inner_w = width.saturating_sub(label_w + suffix_w);

    if inner_w <= 1 {
        let text = if label.is_empty() {
            format!("{:.1}%", pct)
        } else {
            format!("{} {:.1}%", label, pct)
        };
        cmds.push(RenderCmd::Text {
            x,
            y,
            text,
            fg: Some(theme.text),
            bg: None,
            bold: false,
        });
        return cmds;
    }

    let filled = (pct / 100.0 * inner_w as f32) as u16;
    let empty = inner_w.saturating_sub(filled);

    let bar: String = (0..filled)
        .map(|_| BAR_BLOCK)
        .chain((0..empty).map(|_| BAR_EMPTY))
        .collect();

    let display = format!("{label_part}{bar}{suffix}");

    cmds.push(RenderCmd::Text {
        x,
        y,
        text: display,
        fg: Some(theme.text),
        bg: None,
        bold: false,
    });
    cmds.push(RenderCmd::Text {
        x: x + bar_offset,
        y,
        text: bar,
        fg: Some(bar_color),
        bg: None,
        bold: false,
    });

    cmds
}

pub fn render_sparkline(
    history: &VecDeque<f32>,
    width: u16,
    x: u16,
    y: u16,
    color: [u8; 3],
) -> RenderCmd {
    let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let w = width as usize;

    let mut line = String::new();
    if history.is_empty() || w == 0 {
        line = (0..w).map(|_| '▁').collect();
    } else {
        let max = history.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min = history.iter().cloned().fold(f32::INFINITY, f32::min);
        let range = max - min;

        let step = if history.len() > 1 {
            (history.len() - 1).max(1) as f32 / w.max(1) as f32
        } else {
            1.0
        };

        for i in 0..w {
            let idx = (i as f32 * step).round() as usize;
            let val = history.get(idx).copied().unwrap_or(min);
            let normalized = if range > 0.0 {
                ((val - min) / range * 7.0) as usize
            } else if val > 0.0 {
                7
            } else {
                0
            };
            line.push(chars[normalized.min(7)]);
        }
    }

    RenderCmd::Text {
        x,
        y,
        text: line,
        fg: Some(color),
        bg: None,
        bold: false,
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
        "  N/A  ".to_string()
    } else {
        format!("{:.2}  {:.2}  {:.2}", load[0], load[1], load[2])
    }
}

fn overview_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let snap = &state.snapshot;

    let gap: u16 = 1;
    let mx: u16 = 1;
    let inner_w = w.saturating_sub(2);

    // ── Computer Panel ──
    let comp_h: u16 = 5;
    cmds.push(RenderCmd::Border {
        x: mx,
        y: 0,
        w: inner_w,
        h: comp_h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Computer".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });
    let comp_iy = 1;
    let max_val_w = inner_w.saturating_sub(4);
    let comp_row = |cmds: &mut Vec<RenderCmd>, y: u16, key: &str, val: &str| {
        let label = format!("{}: ", key);
        let label_w = label.len() as u16;
        cmds.push(RenderCmd::Text {
            x: mx + 2,
            y,
            text: label.clone(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
        cmds.push(RenderCmd::Text {
            x: mx + 2 + label_w,
            y,
            text: santui_ipc::ui::truncate(val, max_val_w.saturating_sub(label_w) as usize),
            fg: Some(theme.text),
            bg: None,
            bold: false,
        });
    };
    comp_row(&mut cmds, comp_iy, "Name", &snap.hostname);
    comp_row(&mut cmds, comp_iy + 1, "OS", &snap.os_name);
    comp_row(
        &mut cmds,
        comp_iy + 2,
        "Uptime",
        &duration_str(snap.uptime_secs),
    );

    // Layout: 4 column row (CPU, Memory, Disk, Network) + Processes row
    let mid_y = comp_h;
    let col_w = (inner_w.saturating_sub(gap * 3)) / 4;
    let mid_h = ((h - comp_h - 2) / 2).max(8);
    let procs_y = mid_y + mid_h;
    let procs_h = h.saturating_sub(procs_y + 2).max(4);

    // ── CPU Panel ──
    let cpu_x = mx;
    let cpu_iw = col_w.saturating_sub(4);
    let cpu_iy = mid_y + 1;
    cmds.push(RenderCmd::Border {
        x: cpu_x,
        y: mid_y,
        w: col_w,
        h: mid_h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("CPU".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    let mut cy = cpu_iy;
    let cpu_desc = format!(
        "{}  · {}c  · {}MHz",
        snap.cpu.brand, snap.cpu.core_count, snap.cpu.frequency_mhz
    );
    let desc = santui_ipc::ui::truncate(&cpu_desc, cpu_iw as usize);
    cmds.push(RenderCmd::Text {
        x: cpu_x + 2,
        y: cy,
        text: desc,
        fg: Some(theme.text),
        bg: None,
        bold: true,
    });
    cy += 1;
    cmds.extend(render_bar(
        "",
        snap.cpu.global_pct,
        cpu_iw,
        cpu_x + 2,
        cy,
        theme,
        None,
    ));
    cy += 1;
    let spark = render_sparkline(
        &state.history.cpu,
        cpu_iw.min(50),
        cpu_x + 2,
        cy,
        theme.accent,
    );
    cmds.push(spark);
    let la_raw = load_avg_str(&snap.load_avg);
    let la = santui_ipc::ui::truncate(&la_raw, cpu_iw as usize);
    cmds.push(RenderCmd::Text {
        x: cpu_x + 2 + cpu_iw.saturating_sub(la.len() as u16 + 2).min(cpu_iw),
        y: cy,
        text: la,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    // ── Memory Panel ──
    let mem_x = cpu_x + col_w + gap;
    let mem_iw = col_w.saturating_sub(4);
    let mem_iy = mid_y + 1;
    cmds.push(RenderCmd::Border {
        x: mem_x,
        y: mid_y,
        w: col_w,
        h: mid_h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Memory".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    let mut my = mem_iy;
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
    cmds.extend(render_bar(
        "RAM",
        ram_pct,
        mem_iw,
        mem_x + 2,
        my,
        theme,
        None,
    ));
    let ram_label_raw = format!(
        "{} / {}",
        fmt_bytes(snap.mem.ram_used),
        fmt_bytes(snap.mem.ram_total)
    );
    let ram_label = santui_ipc::ui::truncate(&ram_label_raw, mem_iw as usize);
    cmds.push(RenderCmd::Text {
        x: mem_x
            + 2
            + mem_iw
                .saturating_sub(ram_label.len() as u16 + 2)
                .min(mem_iw),
        y: my,
        text: ram_label,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });
    my += 1;
    cmds.extend(render_bar(
        "SWP",
        swap_pct,
        mem_iw,
        mem_x + 2,
        my,
        theme,
        None,
    ));
    let swap_label_raw = format!(
        "{} / {}",
        fmt_bytes(snap.mem.swap_used),
        fmt_bytes(snap.mem.swap_total)
    );
    let swap_label = santui_ipc::ui::truncate(&swap_label_raw, mem_iw as usize);
    cmds.push(RenderCmd::Text {
        x: mem_x
            + 2
            + mem_iw
                .saturating_sub(swap_label.len() as u16 + 2)
                .min(mem_iw),
        y: my,
        text: swap_label,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    // ── Disk Panel ──
    let disk_x = mem_x + col_w + gap;
    let disk_iw = col_w.saturating_sub(4);
    let disk_iy = mid_y + 1;
    cmds.push(RenderCmd::Border {
        x: disk_x,
        y: mid_y,
        w: col_w,
        h: mid_h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Disk".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    let mut dy = disk_iy;
    for disk in snap.disks.iter() {
        if dy >= disk_iy + mid_h.saturating_sub(2) {
            break;
        }
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
        cmds.extend(render_bar(
            santui_ipc::ui::truncate(&disk.mount, 6).as_str(),
            pct,
            disk_iw,
            disk_x + 2,
            dy,
            theme,
            Some(color),
        ));
        dy += 1;
    }
    if snap.disks.is_empty() {
        cmds.push(RenderCmd::Text {
            x: disk_x + 2,
            y: dy,
            text: "No disks".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
    }

    // ── Network Panel ──
    let net_x = disk_x + col_w + gap;
    let net_iy = mid_y + 1;
    cmds.push(RenderCmd::Border {
        x: net_x,
        y: mid_y,
        w: col_w,
        h: mid_h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Network".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    let mut ny = net_iy;
    for iface in snap.net.iter() {
        if ny >= net_iy + mid_h.saturating_sub(2) {
            break;
        }
        let line_raw = format!(
            "{}  ↓{}  ↑{}",
            santui_ipc::ui::truncate(&iface.name, 12),
            santui_ipc::ui::truncate(&fmt_bytes(iface.rx_bytes_sec), 7),
            santui_ipc::ui::truncate(&fmt_bytes(iface.tx_bytes_sec), 7),
        );
        let line = santui_ipc::ui::truncate(&line_raw, disk_iw as usize);
        cmds.push(RenderCmd::Text {
            x: net_x + 2,
            y: ny,
            text: line,
            fg: Some(theme.text),
            bg: None,
            bold: false,
        });
        ny += 1;
    }
    if snap.net.is_empty() {
        cmds.push(RenderCmd::Text {
            x: net_x + 2,
            y: ny,
            text: "No interfaces".into(),
            fg: Some(theme.text_muted),
            bg: None,
            bold: false,
        });
    }

    // ── Processes Panel ──
    let procs_iw = inner_w.saturating_sub(4);
    cmds.push(RenderCmd::Border {
        x: mx,
        y: procs_y,
        w: inner_w,
        h: procs_h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Processes".into()),
        title_fg: Some(theme.accent),
        title_dash_fg: Some(theme.border),
    });

    let procs_iy = procs_y + 1;
    let proc_count = format!("Total: {}  Showing top 10 by CPU", snap.total_processes);
    cmds.push(RenderCmd::Text {
        x: mx + 2,
        y: procs_iy,
        text: proc_count,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    let name_w = (procs_iw / 2).max(10) as usize;
    let max_pty = procs_y + procs_h.saturating_sub(1);
    for (i, proc) in snap.top_processes.iter().enumerate() {
        let y_pos = procs_iy + 1 + i as u16;
        if y_pos >= max_pty {
            break;
        }
        let cpu_disp = proc.cpu_pct / snap.cpu.core_count.max(1) as f32;
        let line = format!(
            " {:>5}  {:width$}  {:>6.1}%  {:>8}",
            proc.pid,
            santui_ipc::ui::truncate(&proc.name, name_w),
            cpu_disp,
            fmt_bytes(proc.mem_bytes),
            width = name_w,
        );
        cmds.push(RenderCmd::Text {
            x: mx + 2,
            y: y_pos,
            text: line,
            fg: Some(theme.text),
            bg: None,
            bold: false,
        });
    }

    // Bottom hints
    cmds.push(RenderCmd::Text {
        x: 1,
        y: h.saturating_sub(1),
        text: " 1 cpu  2 mem  3 disk  4 net  5 procs  esc back".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    cmds
}

fn cpu_detail_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let snap = &state.snapshot;

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("CPU Detail".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });

    let mut row = 1;
    let iw = w.saturating_sub(4);

    row += 1;
    let header = format!(
        "{}  ·  {} cores  ·  {} MHz          Global: {:.1}%",
        snap.cpu.brand, snap.cpu.core_count, snap.cpu.frequency_mhz, snap.cpu.global_pct
    );
    cmds.push(RenderCmd::Text {
        x: 2,
        y: row,
        text: header,
        fg: Some(theme.text),
        bg: None,
        bold: true,
    });

    row += 1;
    let half = snap.cpu.core_count.div_ceil(2);
    let half_w = iw / 2;
    for col in 0..half {
        let core_idx = col;
        if core_idx < snap.cpu.per_core.len() {
            let pct = snap.cpu.per_core[core_idx] / snap.cpu.core_count.max(1) as f32;
            let bar = render_bar(
                &format!("Core {}", core_idx),
                pct,
                half_w,
                2,
                row,
                theme,
                None,
            );
            cmds.extend(bar);
        }
        let core_idx2 = col + half;
        if core_idx2 < snap.cpu.per_core.len() {
            let pct = snap.cpu.per_core[core_idx2] / snap.cpu.core_count.max(1) as f32;
            let bar = render_bar(
                &format!("Core {}", core_idx2),
                pct,
                half_w,
                2 + half_w,
                row,
                theme,
                None,
            );
            cmds.extend(bar);
        }
        row += 1;
    }

    row += 1;
    let spark = render_sparkline(&state.history.cpu, iw.min(60), 2, row, theme.accent);
    cmds.push(spark);
    let load_text = format!("Load average:  {}", load_avg_str(&snap.load_avg));
    cmds.push(RenderCmd::Text {
        x: 2 + iw.saturating_sub(load_text.len() as u16 + 2).min(iw),
        y: row,
        text: load_text,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    cmds
}

fn mem_detail_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();
    let snap = &state.snapshot;

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Memory Detail".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });

    let mut row = 1;
    let iw = w.saturating_sub(4);

    row += 1;
    let ram_pct = if snap.mem.ram_total > 0 {
        snap.mem.ram_used as f32 / snap.mem.ram_total as f32 * 100.0
    } else {
        0.0
    };
    let ram_line = format!(
        "RAM   {} used  /  {} total  ({:.1}%)",
        fmt_bytes(snap.mem.ram_used),
        fmt_bytes(snap.mem.ram_total),
        ram_pct
    );
    cmds.extend(render_bar("RAM", ram_pct, iw, 2, row, theme, None));
    let rw = ram_line.len() as u16;
    cmds.push(RenderCmd::Text {
        x: 2 + iw.saturating_sub(rw + 2).min(iw),
        y: row,
        text: ram_line,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    row += 1;
    let swap_pct = if snap.mem.swap_total > 0 {
        snap.mem.swap_used as f32 / snap.mem.swap_total as f32 * 100.0
    } else {
        0.0
    };
    let swap_line = format!(
        "SWAP  {} used  /  {} total  ({:.1}%)",
        fmt_bytes(snap.mem.swap_used),
        fmt_bytes(snap.mem.swap_total),
        swap_pct
    );
    cmds.extend(render_bar("SWAP", swap_pct, iw, 2, row, theme, None));
    let sw = swap_line.len() as u16;
    cmds.push(RenderCmd::Text {
        x: 2 + iw.saturating_sub(sw + 2).min(iw),
        y: row,
        text: swap_line,
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    row += 2;
    let rspark = render_sparkline(&state.history.ram, iw.min(60), 2, row, theme.accent);
    cmds.push(rspark);
    cmds.push(RenderCmd::Text {
        x: 2 + (iw / 2).saturating_sub(15),
        y: row,
        text: "RAM history (60s)".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    row += 1;
    let sspark = render_sparkline(
        &state.history.cpu.iter().map(|_| swap_pct).collect(),
        iw.min(60),
        2,
        row,
        theme.text_muted,
    );
    cmds.push(sspark);
    cmds.push(RenderCmd::Text {
        x: 2 + (iw / 2).saturating_sub(15),
        y: row,
        text: "SWAP history (60s)".into(),
        fg: Some(theme.text_muted),
        bg: None,
        bold: false,
    });

    cmds
}

fn disk_detail_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Disk Detail".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });

    let header = vec![
        "Mount".into(),
        "Device".into(),
        "FS".into(),
        "Used".into(),
        "Total".into(),
        "Usage".into(),
    ];
    let col_w = w.saturating_sub(2) / 6;

    let rows: Vec<Vec<String>> = state
        .snapshot
        .disks
        .iter()
        .map(|d| {
            let pct = if d.total > 0 {
                d.used as f64 / d.total as f64 * 100.0
            } else {
                0.0
            };
            vec![
                d.mount.clone(),
                d.name.clone(),
                d.fs.clone(),
                fmt_bytes(d.used),
                fmt_bytes(d.total),
                format!("{:.1}%", pct),
            ]
        })
        .collect();

    cmds.push(RenderCmd::Table {
        x: 1,
        y: 1,
        w: w.saturating_sub(2),
        h: h.saturating_sub(2),
        header,
        header_style: TextStyle {
            fg: Some(theme.accent),
            bg: None,
            bold: true,
        },
        rows,
        column_widths: vec![col_w, col_w, col_w, col_w, col_w, col_w],
        selected: None,
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: false,
        },
        current_row: None,
        current_style: None,
    });

    cmds
}

fn net_detail_ui(state: &SysMonState, theme: &ThemeData, w: u16, h: u16) -> Vec<RenderCmd> {
    let mut cmds = Vec::new();

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some("Network Detail".into()),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });

    let header = vec![
        "Interface".into(),
        "↓ Speed".into(),
        "↑ Speed".into(),
        "Total ↓".into(),
        "Total ↑".into(),
    ];
    let col_w = w.saturating_sub(2) / 5;

    let rows: Vec<Vec<String>> = state
        .snapshot
        .net
        .iter()
        .map(|n| {
            vec![
                n.name.clone(),
                format!("{}/s", fmt_bytes(n.rx_bytes_sec)),
                format!("{}/s", fmt_bytes(n.tx_bytes_sec)),
                fmt_bytes(n.total_rx),
                fmt_bytes(n.total_tx),
            ]
        })
        .collect();

    cmds.push(RenderCmd::Table {
        x: 1,
        y: 1,
        w: w.saturating_sub(2),
        h: h.saturating_sub(2),
        header,
        header_style: TextStyle {
            fg: Some(theme.accent),
            bg: None,
            bold: true,
        },
        rows,
        column_widths: vec![col_w, col_w, col_w, col_w, col_w],
        selected: None,
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: false,
        },
        current_row: None,
        current_style: None,
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

    let title = format!(" Processes (top 10 by {}) ", sort_label);

    cmds.push(RenderCmd::Border {
        x: 0,
        y: 0,
        w,
        h,
        fg: theme.border,
        bg: None,
        borders: BORDER_ALL,
        title: Some(title),
        title_fg: Some(theme.text),
        title_dash_fg: Some(theme.border),
    });

    let core_count = state.snapshot.cpu.core_count.max(1) as f32;
    let header = vec!["PID".into(), "Name".into(), "CPU %".into(), "Memory".into()];
    let col_w = w.saturating_sub(2) / 4;

    let rows: Vec<Vec<String>> = state
        .snapshot
        .top_processes
        .iter()
        .map(|p| {
            let cpu_display = p.cpu_pct / core_count;
            vec![
                format!("{}", p.pid),
                p.name.clone(),
                format!("{:.1}%", cpu_display),
                fmt_bytes(p.mem_bytes),
            ]
        })
        .collect();

    cmds.push(RenderCmd::Table {
        x: 1,
        y: 1,
        w: w.saturating_sub(2),
        h: h.saturating_sub(2),
        header,
        header_style: TextStyle {
            fg: Some(theme.accent),
            bg: None,
            bold: true,
        },
        rows,
        column_widths: vec![col_w, col_w, col_w, col_w],
        selected: Some(state.selected_process),
        style: TextStyle {
            fg: Some(theme.text),
            bg: None,
            bold: false,
        },
        highlight_style: TextStyle {
            fg: Some(theme.inverted_text),
            bg: Some(theme.highlight),
            bold: false,
        },
        current_row: None,
        current_style: None,
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
    fn render_bar_full() {
        let theme = test_theme();
        let cmds = render_bar("CPU", 100.0, 40, 0, 0, &theme, None);
        let has_bar = cmds.iter().any(|c| {
            if let RenderCmd::Text { ref text, .. } = c {
                text.contains(BAR_BLOCK)
            } else {
                false
            }
        });
        assert!(has_bar);
    }

    #[test]
    fn render_bar_empty() {
        let theme = test_theme();
        let cmds = render_bar("CPU", 0.0, 40, 0, 0, &theme, None);
        let empty_bar = cmds.iter().any(|c| {
            if let RenderCmd::Text { ref text, .. } = c {
                !text.contains(BAR_BLOCK) && text.contains(BAR_EMPTY)
            } else {
                false
            }
        });
        assert!(empty_bar);
    }

    #[test]
    fn render_bar_half() {
        let theme = test_theme();
        let cmds = render_bar("CPU", 50.0, 40, 0, 0, &theme, None);
        assert!(!cmds.is_empty());
    }

    #[test]
    fn render_bar_color_success_below_60() {
        let theme = test_theme();
        let cmds = render_bar("CPU", 40.0, 40, 0, 0, &theme, None);
        let bar_cmd = cmds
            .iter()
            .find(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains(BAR_BLOCK)));
        assert!(bar_cmd.is_some());
    }

    #[test]
    fn render_bar_color_warning_at_70() {
        let theme = test_theme();
        let cmds = render_bar("CPU", 70.0, 40, 0, 0, &theme, None);
        assert!(!cmds.is_empty());
    }

    #[test]
    fn render_bar_color_error_above_80() {
        let theme = test_theme();
        let cmds = render_bar("CPU", 90.0, 40, 0, 0, &theme, None);
        assert!(!cmds.is_empty());
    }

    #[test]
    fn render_sparkline_empty_history() {
        let theme = test_theme();
        let history = VecDeque::new();
        let cmd = render_sparkline(&history, 10, 0, 0, theme.text);
        if let RenderCmd::Text { ref text, .. } = cmd {
            assert!(text.chars().all(|c| c == '▁'));
        }
    }

    #[test]
    fn render_sparkline_full() {
        let theme = test_theme();
        let mut history = VecDeque::new();
        for _ in 0..10 {
            history.push_back(100.0);
        }
        let cmd = render_sparkline(&history, 10, 0, 0, theme.text);
        if let RenderCmd::Text { ref text, .. } = cmd {
            assert!(text.chars().all(|c| c == '█'));
        }
    }

    #[test]
    fn overview_renders_cpu_bar() {
        let mut state = SysMonState::default();
        state.snapshot.cpu.brand = "Test CPU".into();
        state.snapshot.cpu.core_count = 4;
        state.snapshot.cpu.global_pct = 50.0;
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        let has_bar = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains(BAR_BLOCK)));
        assert!(has_bar);
    }

    #[test]
    fn overview_renders_hostname() {
        let mut state = SysMonState::default();
        state.snapshot.hostname = "testhost".into();
        let theme = test_theme();
        let cmds = overview_ui(&state, &theme, 80, 24);
        let has_hostname = cmds.iter().any(|c| match c {
            RenderCmd::Text { ref text, .. } => text.contains("testhost"),
            RenderCmd::Border { ref title, .. } => {
                title.as_deref().map_or(false, |t| t.contains("testhost"))
            }
            _ => false,
        });
        assert!(has_hostname);
    }

    #[test]
    fn cpu_detail_renders_per_core_bars() {
        let mut state = SysMonState::default();
        state.snapshot.cpu.core_count = 4;
        state.snapshot.cpu.per_core = vec![10.0, 20.0, 30.0, 40.0];
        let theme = test_theme();
        let cmds = cpu_detail_ui(&state, &theme, 80, 24);
        let core_label = cmds
            .iter()
            .any(|c| matches!(c, RenderCmd::Text { ref text, .. } if text.contains("Core 0")));
        assert!(core_label);
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
            removable: false,
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
                cpu_pct: 10.0,
                mem_bytes: 1000,
            });
        let theme = test_theme();
        let cmds = process_list_ui(&state, &theme, 80, 24);
        let has_table = cmds.iter().any(|c| matches!(c, RenderCmd::Table { .. }));
        assert!(has_table);
    }
}
