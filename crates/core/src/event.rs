use std::collections::VecDeque;

use crate::theme::Theme;

/// Events that can be published through the application's [`EventBus`].
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The active theme changed (plugins should refresh their colours).
    ThemeChanged(Theme),
    /// Plugin-to-plugin message.  `from` and `to` are plugin ids.
    PluginMessage {
        from: String,
        to: String,
        action: String,
        data: String,
    },
}

/// A read-only observer registered via [`EventBus::subscribe`].
pub type EventSubscriber = Box<dyn FnMut(&Event) + Send>;

/// A simple in-app event bus for decoupling components.
///
/// Components emit events via [`EventBus::emit`] and the main loop drains the
/// pending queue via [`EventBus::drain`] once per frame, forwarding them to
/// [`PluginManager::process_events`](crate::app::plugin_manager::PluginManager).
///
/// External code can register read-only observers with [`EventBus::subscribe`]
/// to react to events without consuming them (e.g. event logging, metrics).
///
/// The pending queue is capped at [`MAX_PENDING`] entries.  If full, the oldest
/// event is dropped to make room for the newest, ensuring the bus never grows
/// without bound.
pub struct EventBus {
    pending: VecDeque<Event>,
    subscribers: Vec<EventSubscriber>,
}

const MAX_PENDING: usize = 1024;

impl EventBus {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            subscribers: Vec::new(),
        }
    }

    /// Register a read-only observer that is called for every emitted event.
    ///
    /// Subscribers are invoked synchronously inside [`EventBus::emit`] after the
    /// event is pushed to the pending queue. They receive a shared reference and
    /// cannot modify or consume the event.
    pub fn subscribe(&mut self, f: EventSubscriber) {
        self.subscribers.push(f);
    }

    /// Push an event onto the pending queue and notify all subscribers.
    ///
    /// If the queue is at capacity the oldest event is dropped.
    pub fn emit(&mut self, event: Event) {
        for sub in &mut self.subscribers {
            sub(&event);
        }
        if self.pending.len() >= MAX_PENDING {
            self.pending.pop_front();
        }
        self.pending.push_back(event);
    }

    /// Drain all pending events.
    pub fn drain(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending).into_iter().collect()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("pending", &self.pending)
            .field(
                "subscribers",
                &format_args!("{} subscribers", self.subscribers.len()),
            )
            .finish()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn msg(from: &str, to: &str) -> Event {
        Event::PluginMessage {
            from: from.into(),
            to: to.into(),
            action: "test".into(),
            data: "{}".into(),
        }
    }

    #[test]
    fn new_creates_empty_bus() {
        let mut bus = EventBus::new();
        assert!(bus.drain().is_empty());
    }

    #[test]
    fn default_creates_empty_bus() {
        let mut bus = EventBus::default();
        assert!(bus.drain().is_empty());
    }

    #[test]
    fn emit_and_drain_returns_event() {
        let mut bus = EventBus::new();
        bus.emit(msg("a", "b"));
        let drained = bus.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0], msg("a", "b"));
    }

    #[test]
    fn drain_returns_in_order() {
        let mut bus = EventBus::new();
        bus.emit(msg("a", "b"));
        bus.emit(msg("c", "d"));
        let drained = bus.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0], msg("a", "b"));
        assert_eq!(drained[1], msg("c", "d"));
    }

    #[test]
    fn drain_clears_queue() {
        let mut bus = EventBus::new();
        bus.emit(msg("a", "b"));
        let first = bus.drain();
        assert_eq!(first.len(), 1);
        let second = bus.drain();
        assert!(second.is_empty());
    }

    #[test]
    fn drain_empty_returns_empty() {
        let mut bus = EventBus::new();
        assert!(bus.drain().is_empty());
        assert!(bus.drain().is_empty());
    }

    #[test]
    fn subscriber_receives_event() {
        let mut bus = EventBus::new();
        let count = std::sync::Arc::new(Mutex::new(0usize));
        let c = count.clone();
        bus.subscribe(Box::new(move |_| {
            *c.lock().unwrap() += 1;
        }));
        bus.emit(msg("a", "b"));
        assert_eq!(*count.lock().unwrap(), 1);
        bus.emit(msg("c", "d"));
        assert_eq!(*count.lock().unwrap(), 2);
    }

    #[test]
    fn multiple_subscribers_all_called() {
        let mut bus = EventBus::new();
        let c1 = std::sync::Arc::new(Mutex::new(0usize));
        let c2 = std::sync::Arc::new(Mutex::new(0usize));
        let a1 = c1.clone();
        let a2 = c2.clone();
        bus.subscribe(Box::new(move |_| {
            *a1.lock().unwrap() += 1;
        }));
        bus.subscribe(Box::new(move |_| {
            *a2.lock().unwrap() += 1;
        }));
        bus.emit(msg("a", "b"));
        assert_eq!(*c1.lock().unwrap(), 1);
        assert_eq!(*c2.lock().unwrap(), 1);
    }

    #[test]
    fn subscriber_receives_correct_event() {
        let mut bus = EventBus::new();
        let seen = std::sync::Arc::new(Mutex::new(String::new()));
        let s = seen.clone();
        bus.subscribe(Box::new(move |e| {
            if let Event::PluginMessage { from, .. } = e {
                *s.lock().unwrap() = from.clone();
            }
        }));
        bus.emit(msg("hello", "world"));
        assert_eq!(*seen.lock().unwrap(), "hello");
    }

    #[test]
    fn max_pending_drops_oldest() {
        let mut bus = EventBus::new();
        for i in 0..super::MAX_PENDING {
            bus.emit(msg(&format!("s{i}"), "t"));
        }
        bus.emit(msg("last", "t"));
        let drained = bus.drain();
        assert_eq!(drained.len(), super::MAX_PENDING);
        if let Event::PluginMessage { from, .. } = &drained[0] {
            assert_eq!(from, "s1");
        } else {
            panic!("expected PluginMessage");
        }
        if let Event::PluginMessage { from, .. } = &drained[drained.len() - 1] {
            assert_eq!(from, "last");
        } else {
            panic!("expected PluginMessage");
        }
    }

    #[test]
    fn subscriber_called_before_queue_drain() {
        let mut bus = EventBus::new();
        let flag = std::sync::Arc::new(Mutex::new(false));
        let f = flag.clone();
        bus.subscribe(Box::new(move |_| {
            *f.lock().unwrap() = true;
        }));
        bus.emit(msg("a", "b"));
        assert!(*flag.lock().unwrap());
    }

    #[test]
    fn debug_format() {
        let mut bus = EventBus::new();
        bus.subscribe(Box::new(|_| {}));
        let s = format!("{:?}", bus);
        assert!(s.contains("EventBus"));
        assert!(s.contains("1 subscriber"));
    }
}
