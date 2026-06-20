use std::collections::VecDeque;

/// Events that can be published through the application's [`EventBus`].
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The active theme changed (plugins should refresh their colours).
    ThemeChanged,
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

/// A simple in-app event bus for decoupling components.
///
/// Components emit events via [`EventBus::emit`] and the main loop drains the
/// pending queue via [`EventBus::drain`] once per frame, forwarding them to
/// [`PluginManager::process_events`](crate::app::plugin_manager::PluginManager).
///
/// The pending queue is capped at [`MAX_PENDING`] entries.  If full, the oldest
/// event is dropped to make room for the newest, ensuring the bus never grows
/// without bound.
#[derive(Debug, Default)]
pub struct EventBus {
    pending: VecDeque<Event>,
}

const MAX_PENDING: usize = 1024;

impl EventBus {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
        }
    }

    /// Push an event onto the pending queue.
    ///
    /// If the queue is at capacity the oldest event is dropped.
    pub fn emit(&mut self, event: Event) {
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
