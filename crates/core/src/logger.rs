use std::collections::VecDeque;
use std::sync::Mutex;

/// A single log entry captured by [`LoggerBuffer`].
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// A `log::Log` implementation that stores entries in a bounded ring buffer
/// and optionally forwards them to an inner logger (e.g. `env_logger` for
/// stderr output). The buffer is exposed for in-process consumption by the
/// log-viewer plugin.
pub struct LoggerBuffer {
    buffer: Mutex<VecDeque<LogEntry>>,
    max_entries: usize,
    inner: Option<Box<dyn log::Log + Send + 'static>>,
}

impl LoggerBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            buffer: Mutex::new(VecDeque::with_capacity(max_entries)),
            max_entries,
            inner: None,
        }
    }

    /// Set an inner logger that every record is forwarded to in addition to
    /// the ring buffer. Call before installing as the global logger.
    pub fn set_inner(&mut self, inner: Box<dyn log::Log + Send + 'static>) {
        self.inner = Some(inner);
    }

    /// Take a snapshot of all buffered entries, leaving the buffer empty.
    pub fn drain(&self) -> Vec<LogEntry> {
        let mut buf = self.buffer.lock().expect("log buffer poisoned");
        buf.drain(..).collect()
    }

    /// Peek at current entries without draining.
    pub fn snapshot(&self) -> Vec<LogEntry> {
        let buf = self.buffer.lock().expect("log buffer poisoned");
        buf.iter().cloned().collect()
    }
}

impl log::Log for LoggerBuffer {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.inner
            .as_ref()
            .is_none_or(|inner| inner.enabled(metadata))
    }

    fn log(&self, record: &log::Record) {
        if let Some(ref inner) = self.inner {
            inner.log(record);
        }
        let entry = LogEntry {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            level: record.level().to_string(),
            target: record.target().to_string(),
            message: record.args().to_string(),
        };
        let mut buf = self.buffer.lock().expect("log buffer poisoned");
        if buf.len() >= self.max_entries {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    fn flush(&self) {
        if let Some(ref inner) = self.inner {
            inner.flush();
        }
    }
}
