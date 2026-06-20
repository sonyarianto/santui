use santui_core::event::{Event, EventBus};

#[test]
fn event_bus_new_is_empty() {
    let mut bus = EventBus::new();
    let events = bus.drain();
    assert!(events.is_empty());
}

#[test]
fn event_bus_emit_and_drain() {
    let mut bus = EventBus::new();
    bus.emit(Event::ThemeChanged);
    bus.emit(Event::UserUpdated);

    let events = bus.drain();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], Event::ThemeChanged);
    assert_eq!(events[1], Event::UserUpdated);
}

#[test]
fn event_bus_drain_clears_queue() {
    let mut bus = EventBus::new();
    bus.emit(Event::ThemeChanged);

    let _ = bus.drain();
    let remaining = bus.drain();
    assert!(remaining.is_empty());
}

#[test]
fn event_bus_plugin_message() {
    let mut bus = EventBus::new();
    bus.emit(Event::PluginMessage {
        from: "plugin_a".into(),
        to: "plugin_b".into(),
        action: "sync".into(),
        data: "hello".into(),
    });

    let events = bus.drain();
    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::PluginMessage {
            from,
            to,
            action,
            data,
        } => {
            assert_eq!(from, "plugin_a");
            assert_eq!(to, "plugin_b");
            assert_eq!(action, "sync");
            assert_eq!(data, "hello");
        }
        _ => panic!("Expected PluginMessage"),
    }
}

#[test]
fn event_bus_drain_does_not_panic_when_empty() {
    let mut bus = EventBus::new();
    let events = bus.drain();
    assert!(events.is_empty());
    // Second drain should also be empty.
    let events = bus.drain();
    assert!(events.is_empty());
}
