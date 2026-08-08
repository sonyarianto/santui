use std::collections::HashMap;

use sysinfo::{Disks, Networks, ProcessesToUpdate, RefreshKind, System};

use crate::state::{
    BatterySnapshot, CpuSnapshot, DiskSnapshot, MemSnapshot, NetSnapshot, ProcessSnapshot,
    SystemSnapshot,
};

pub struct Sampler {
    sys: System,
    disks: Disks,
    networks: Networks,
    prev_net_rx: HashMap<String, u64>,
    prev_net_tx: HashMap<String, u64>,
    prev_sample_time: std::time::Instant,
    total_processes: u32,
    top_processes: Vec<ProcessSnapshot>,
}

impl Default for Sampler {
    fn default() -> Self {
        let mut sys = System::new_with_specifics(RefreshKind::everything());
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_all();
        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            prev_net_rx: HashMap::new(),
            prev_net_tx: HashMap::new(),
            prev_sample_time: std::time::Instant::now(),
            total_processes: 0,
            top_processes: Vec::new(),
        }
    }
}

impl Sampler {
    pub fn sample(&mut self, refresh_processes: bool) -> SystemSnapshot {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        if refresh_processes {
            self.sys.refresh_processes(ProcessesToUpdate::All, false);
        }
        self.disks.refresh(true);
        self.networks.refresh(true);

        let elapsed = self.prev_sample_time.elapsed().as_secs_f64().max(0.001);
        self.prev_sample_time = std::time::Instant::now();

        let global_pct = self.sys.global_cpu_usage();
        let per_core: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        let brand = self
            .sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default();
        let frequency_mhz = self.sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);

        let mem = MemSnapshot {
            ram_used: self.sys.used_memory(),
            ram_total: self.sys.total_memory(),
            swap_used: self.sys.used_swap(),
            swap_total: self.sys.total_swap(),
        };

        let disks = self
            .disks
            .iter()
            .map(|d| {
                let total = d.total_space();
                let available = d.available_space();
                DiskSnapshot {
                    name: d.name().to_string_lossy().into(),
                    mount: d.mount_point().to_string_lossy().into(),
                    used: total.saturating_sub(available),
                    total,
                    fs: d.file_system().to_string_lossy().into(),
                }
            })
            .collect();

        let mut net = Vec::new();
        for (name, data) in self.networks.iter() {
            let rx_total = data.total_received();
            let tx_total = data.total_transmitted();
            let prev_rx = *self.prev_net_rx.get(name).unwrap_or(&rx_total);
            let prev_tx = *self.prev_net_tx.get(name).unwrap_or(&tx_total);
            let rx_sec = ((rx_total.saturating_sub(prev_rx)) as f64 / elapsed) as u64;
            let tx_sec = ((tx_total.saturating_sub(prev_tx)) as f64 / elapsed) as u64;
            self.prev_net_rx.insert(name.clone(), rx_total);
            self.prev_net_tx.insert(name.clone(), tx_total);
            if rx_total > 0 || tx_total > 0 {
                net.push(NetSnapshot {
                    name: name.clone(),
                    rx_bytes_sec: rx_sec,
                    tx_bytes_sec: tx_sec,
                    total_rx: rx_total,
                    total_tx: tx_total,
                });
            }
        }
        net.sort_by_key(|b| std::cmp::Reverse(b.rx_bytes_sec));

        if refresh_processes {
            let mut all_procs: Vec<ProcessSnapshot> = self
                .sys
                .processes()
                .values()
                .map(|p| {
                    let full_name = p
                        .cmd()
                        .first()
                        .and_then(|c| std::path::Path::new(c).file_name())
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.name().to_string_lossy().into_owned());
                    ProcessSnapshot {
                        pid: p.pid().as_u32(),
                        name: full_name,
                        path: p
                            .exe()
                            .map(|e| e.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        cpu_pct: p.cpu_usage(),
                        mem_bytes: p.memory(),
                    }
                })
                .collect();
            self.total_processes = all_procs.len() as u32;
            all_procs.sort_by(|a, b| {
                b.cpu_pct
                    .partial_cmp(&a.cpu_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            all_procs.truncate(crate::state::TOP_PROCESSES);
            self.top_processes = all_procs;
        }
        let top_processes = self.top_processes.clone();

        let load_avg = System::load_average();

        SystemSnapshot {
            cpu: CpuSnapshot {
                global_pct,
                per_core,
                core_count: self.sys.cpus().len(),
                brand,
                frequency_mhz,
            },
            mem,
            disks,
            net,
            total_processes: self.total_processes,
            top_processes,
            hostname: System::host_name().unwrap_or_default(),
            os_name: System::long_os_version().unwrap_or_default(),
            uptime_secs: System::uptime(),
            load_avg: [load_avg.one, load_avg.five, load_avg.fifteen],
            battery: read_battery(),
            shell: std::env::var("SHELL")
                .ok()
                .and_then(|s| {
                    std::path::Path::new(&s)
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                })
                .unwrap_or_default(),
        }
    }
}

/// Reads the first battery from sysfs (`/sys/class/power_supply`).
/// Returns `None` on non-Linux systems or when no battery is present.
#[cfg(target_os = "linux")]
fn read_battery() -> Option<BatterySnapshot> {
    let entries = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = std::fs::read_to_string(path.join("type")).unwrap_or_default();
        if kind.trim() != "Battery" {
            continue;
        }
        let pct = std::fs::read_to_string(path.join("capacity"))
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()?;
        let status = std::fs::read_to_string(path.join("status")).unwrap_or_default();
        let state = match status.trim() {
            "Charging" => "Charging",
            "Discharging" => "Discharging",
            "Full" => "Full",
            "Not charging" => "Not charging",
            _ => "Unknown",
        };
        return Some(BatterySnapshot {
            pct: pct.clamp(0.0, 100.0),
            state: state.into(),
        });
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn read_battery() -> Option<BatterySnapshot> {
    None
}

pub fn fmt_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_bytes_bytes() {
        assert_eq!(fmt_bytes(500), "500 B");
    }

    #[test]
    fn fmt_bytes_kilobytes() {
        assert_eq!(fmt_bytes(1_500), "1.5 KB");
    }

    #[test]
    fn fmt_bytes_megabytes() {
        assert_eq!(fmt_bytes(1_500_000), "1.4 MB");
    }

    #[test]
    fn fmt_bytes_gigabytes() {
        assert_eq!(fmt_bytes(1_500_000_000), "1.4 GB");
    }

    #[test]
    fn fmt_bytes_zero() {
        assert_eq!(fmt_bytes(0), "0 B");
    }

    #[test]
    fn fmt_bytes_terabytes() {
        assert_eq!(fmt_bytes(1_500_000_000_000), "1.4 TB");
    }
}
