use std::collections::HashMap;

use sysinfo::{Disks, Networks, ProcessesToUpdate, RefreshKind, System};

use crate::state::{
    CpuSnapshot, DiskSnapshot, MemSnapshot, NetSnapshot, ProcessSnapshot, SystemSnapshot,
};

pub struct Sampler {
    sys: System,
    disks: Disks,
    networks: Networks,
    prev_net_rx: HashMap<String, u64>,
    prev_net_tx: HashMap<String, u64>,
    prev_sample_time: std::time::Instant,
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
        }
    }
}

impl Sampler {
    pub fn sample(&mut self) -> SystemSnapshot {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, false);
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
                    removable: d.is_removable(),
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

        let all_procs: Vec<ProcessSnapshot> = self
            .sys
            .processes()
            .values()
            .map(|p| ProcessSnapshot {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu_pct: p.cpu_usage(),
                mem_bytes: p.memory(),
            })
            .collect();
        let total_procs = all_procs.len();
        let mut procs: Vec<ProcessSnapshot> = all_procs;
        procs.sort_by(|a, b| {
            b.cpu_pct
                .partial_cmp(&a.cpu_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        procs.truncate(10);

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
            total_processes: total_procs as u16,
            top_processes: procs,
            hostname: System::host_name().unwrap_or_default(),
            os_name: System::long_os_version().unwrap_or_default(),
            kernel: System::kernel_version().unwrap_or_default(),
            uptime_secs: System::uptime(),
            load_avg: [load_avg.one, load_avg.five, load_avg.fifteen],
        }
    }
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
