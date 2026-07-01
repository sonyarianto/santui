use std::collections::VecDeque;

pub const HISTORY_LEN: usize = 60;

#[derive(Debug, Clone, Default)]
pub struct CpuSnapshot {
    pub global_pct: f32,
    pub per_core: Vec<f32>,
    pub core_count: usize,
    pub brand: String,
    pub frequency_mhz: u64,
}

#[derive(Debug, Clone, Default)]
pub struct MemSnapshot {
    pub ram_used: u64,
    pub ram_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
}

#[derive(Debug, Clone)]
pub struct DiskSnapshot {
    pub name: String,
    pub mount: String,
    pub used: u64,
    pub total: u64,
    pub fs: String,
    #[allow(dead_code)]
    pub removable: bool,
}

#[derive(Debug, Clone)]
pub struct NetSnapshot {
    pub name: String,
    pub rx_bytes_sec: u64,
    pub tx_bytes_sec: u64,
    pub total_rx: u64,
    pub total_tx: u64,
}

#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub mem_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct SystemSnapshot {
    pub cpu: CpuSnapshot,
    pub mem: MemSnapshot,
    pub disks: Vec<DiskSnapshot>,
    pub net: Vec<NetSnapshot>,
    pub total_processes: u16,
    pub top_processes: Vec<ProcessSnapshot>,
    pub hostname: String,
    pub os_name: String,
    #[allow(dead_code)]
    pub kernel: String,
    pub uptime_secs: u64,
    pub load_avg: [f64; 3],
}

pub struct MetricHistory {
    pub cpu: VecDeque<f32>,
    pub ram: VecDeque<f32>,
    pub net_rx: VecDeque<u64>,
    pub net_tx: VecDeque<u64>,
}

impl Default for MetricHistory {
    fn default() -> Self {
        Self {
            cpu: VecDeque::with_capacity(HISTORY_LEN),
            ram: VecDeque::with_capacity(HISTORY_LEN),
            net_rx: VecDeque::with_capacity(HISTORY_LEN),
            net_tx: VecDeque::with_capacity(HISTORY_LEN),
        }
    }
}

impl MetricHistory {
    pub fn push(&mut self, snap: &SystemSnapshot) {
        let ram_pct = if snap.mem.ram_total > 0 {
            snap.mem.ram_used as f32 / snap.mem.ram_total as f32 * 100.0
        } else {
            0.0
        };

        self.cpu.push_back(snap.cpu.global_pct);
        self.ram.push_back(ram_pct);

        let net_rx: u64 = snap.net.iter().map(|n| n.rx_bytes_sec).sum();
        let net_tx: u64 = snap.net.iter().map(|n| n.tx_bytes_sec).sum();
        self.net_rx.push_back(net_rx);
        self.net_tx.push_back(net_tx);

        while self.cpu.len() > HISTORY_LEN {
            self.cpu.pop_front();
        }
        while self.ram.len() > HISTORY_LEN {
            self.ram.pop_front();
        }
        while self.net_rx.len() > HISTORY_LEN {
            self.net_rx.pop_front();
        }
        while self.net_tx.len() > HISTORY_LEN {
            self.net_tx.pop_front();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    Overview,
    CpuDetail,
    MemDetail,
    DiskDetail,
    NetDetail,
    ProcessList,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortBy {
    Cpu,
    Memory,
    Name,
}

pub struct SysMonState {
    pub snapshot: SystemSnapshot,
    pub history: MetricHistory,
    pub screen: Screen,
    pub process_sort: SortBy,
    pub selected_process: usize,
    pub last_second: u64,
}

impl Default for SysMonState {
    fn default() -> Self {
        Self {
            snapshot: SystemSnapshot::default(),
            history: MetricHistory::default(),
            screen: Screen::Overview,
            process_sort: SortBy::Cpu,
            selected_process: 0,
            last_second: 0,
        }
    }
}

impl SysMonState {
    pub fn sort_processes(&mut self) {
        match self.process_sort {
            SortBy::Cpu => {
                self.snapshot.top_processes.sort_by(|a, b| {
                    b.cpu_pct
                        .partial_cmp(&a.cpu_pct)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortBy::Memory => {
                self.snapshot
                    .top_processes
                    .sort_by_key(|b| std::cmp::Reverse(b.mem_bytes));
            }
            SortBy::Name => {
                self.snapshot
                    .top_processes
                    .sort_by_key(|a| a.name.to_lowercase());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_history_push_caps_at_history_len() {
        let mut mh = MetricHistory::default();
        let snap = SystemSnapshot::default();
        for _ in 0..70 {
            mh.push(&snap);
        }
        assert_eq!(mh.cpu.len(), HISTORY_LEN);
        assert_eq!(mh.ram.len(), HISTORY_LEN);
        assert_eq!(mh.net_rx.len(), HISTORY_LEN);
        assert_eq!(mh.net_tx.len(), HISTORY_LEN);
    }

    #[test]
    fn metric_history_push_computes_correct_ram_pct() {
        let mut mh = MetricHistory::default();
        let mut snap = SystemSnapshot::default();
        snap.mem.ram_total = 16_000_000_000;
        snap.mem.ram_used = 8_000_000_000;
        mh.push(&snap);
        assert!((mh.ram[0] - 50.0).abs() < 0.001);
    }

    fn make_proc(name: &str, cpu: f32, mem: u64) -> ProcessSnapshot {
        ProcessSnapshot {
            pid: 0,
            name: name.into(),
            cpu_pct: cpu,
            mem_bytes: mem,
        }
    }

    #[test]
    fn process_sort_by_cpu() {
        let mut state = SysMonState::default();
        state.snapshot.top_processes = vec![
            make_proc("a", 10.0, 100),
            make_proc("b", 50.0, 200),
            make_proc("c", 30.0, 300),
        ];
        state.process_sort = SortBy::Cpu;
        state.sort_processes();
        assert_eq!(state.snapshot.top_processes[0].name, "b");
        assert_eq!(state.snapshot.top_processes[1].name, "c");
        assert_eq!(state.snapshot.top_processes[2].name, "a");
    }

    #[test]
    fn process_sort_by_memory() {
        let mut state = SysMonState::default();
        state.snapshot.top_processes = vec![
            make_proc("a", 10.0, 100),
            make_proc("b", 50.0, 200),
            make_proc("c", 30.0, 300),
        ];
        state.process_sort = SortBy::Memory;
        state.sort_processes();
        assert_eq!(state.snapshot.top_processes[0].name, "c");
        assert_eq!(state.snapshot.top_processes[1].name, "b");
        assert_eq!(state.snapshot.top_processes[2].name, "a");
    }

    #[test]
    fn process_sort_by_name() {
        let mut state = SysMonState::default();
        state.snapshot.top_processes = vec![
            make_proc("zed", 10.0, 100),
            make_proc("alpha", 50.0, 200),
            make_proc("beta", 30.0, 300),
        ];
        state.process_sort = SortBy::Name;
        state.sort_processes();
        assert_eq!(state.snapshot.top_processes[0].name, "alpha");
        assert_eq!(state.snapshot.top_processes[1].name, "beta");
        assert_eq!(state.snapshot.top_processes[2].name, "zed");
    }
}
