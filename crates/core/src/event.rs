use std::mem;

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
#[derive(Debug, Default)]
pub struct EventBus {
    pending: Vec<Event>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Push an event onto the pending queue.
    pub fn emit(&mut self, event: Event) {
        self.pending.push(event);
    }

    /// Drain all pending events.
    pub fn drain(&mut self) -> Vec<Event> {
        mem::take(&mut self.pending)
    }
}
