use std::collections::VecDeque;

use crate::theme::Theme;

/// Events that can be published through the application's [`EventBus`].
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The active theme changed (plugins should refresh their colours).
    ThemeChanged(Theme),
    /// The current user signed in or out (plugins should refresh their state).
    UserUpdated,
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
        self.pending.drain(..).collect()
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
