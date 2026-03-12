//! Lightweight event bus for decoupled inter-component communication.
//!
//! Components publish events; subscribers receive copies asynchronously.
//! Events are broadcast via `tokio::sync::broadcast`.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::broadcast;
use tracing::debug;

/// Maximum number of events buffered per channel.
const DEFAULT_CAPACITY: usize = 256;

/// An event published on the bus.
#[derive(Debug, Clone)]
pub struct Event {
    /// Event type identifier (e.g., "tool_call_start", "llm_response").
    pub event_type: String,
    /// Component that published the event.
    pub source: String,
    /// Event payload.
    pub data: Value,
    /// Timestamp (milliseconds since epoch).
    pub timestamp_ms: u64,
}

impl Event {
    /// Create a new event.
    pub fn new(event_type: impl Into<String>, source: impl Into<String>, data: Value) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            event_type: event_type.into(),
            source: source.into(),
            data,
            timestamp_ms,
        }
    }
}

/// Event bus for broadcasting events to subscribers.
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

struct EventBusInner {
    sender: broadcast::Sender<Event>,
    _capacity: usize,
}

impl EventBus {
    /// Create a new event bus with the default capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a new event bus with a specific capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            inner: Arc::new(EventBusInner {
                sender,
                _capacity: capacity,
            }),
        }
    }

    /// Publish an event to all subscribers.
    pub fn publish(&self, event: Event) {
        let event_type = event.event_type.clone();
        match self.inner.sender.send(event) {
            Ok(n) => debug!("Event '{}' sent to {} subscribers", event_type, n),
            Err(_) => debug!("Event '{}' published with no subscribers", event_type),
        }
    }

    /// Convenience method to publish a simple event.
    pub fn emit(&self, event_type: &str, source: &str, data: Value) {
        self.publish(Event::new(event_type, source, data));
    }

    /// Subscribe to receive events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.inner.sender.subscribe()
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.inner.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("subscribers", &self.subscriber_count())
            .finish()
    }
}

/// Filtered event subscriber — only receives events matching a filter.
pub struct FilteredSubscriber {
    receiver: broadcast::Receiver<Event>,
    event_types: Option<Vec<String>>,
}

impl FilteredSubscriber {
    /// Create a filtered subscriber.
    pub fn new(bus: &EventBus, event_types: Option<Vec<String>>) -> Self {
        Self {
            receiver: bus.subscribe(),
            event_types,
        }
    }

    /// Receive the next matching event.
    pub async fn recv(&mut self) -> Option<Event> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => {
                    if let Some(ref types) = self.event_types {
                        if !types.iter().any(|t| t == &event.event_type) {
                            continue;
                        }
                    }
                    return Some(event);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    debug!("Subscriber lagged, missed {n} events");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}

/// Collect events into a map grouped by event type (useful for metrics).
pub fn group_events_by_type(events: &[Event]) -> HashMap<String, Vec<&Event>> {
    let mut groups: HashMap<String, Vec<&Event>> = HashMap::new();
    for event in events {
        groups
            .entry(event.event_type.clone())
            .or_default()
            .push(event);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new("test", "component", serde_json::json!({"key": "value"}));
        assert_eq!(event.event_type, "test");
        assert_eq!(event.source, "component");
        assert!(event.timestamp_ms > 0);
    }

    #[test]
    fn test_bus_creation() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_publish_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.emit("test_event", "test", serde_json::json!({"count": 1}));

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "test_event");
        assert_eq!(event.data["count"], 1);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        bus.emit("event", "src", serde_json::json!(null));

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();
        assert_eq!(e1.event_type, "event");
        assert_eq!(e2.event_type, "event");
    }

    #[tokio::test]
    async fn test_filtered_subscriber() {
        let bus = EventBus::new();
        let mut sub = FilteredSubscriber::new(
            &bus,
            Some(vec!["wanted".to_string()]),
        );

        bus.emit("unwanted", "src", serde_json::json!(null));
        bus.emit("wanted", "src", serde_json::json!({"ok": true}));

        let event = sub.recv().await.unwrap();
        assert_eq!(event.event_type, "wanted");
    }

    #[test]
    fn test_no_subscribers() {
        let bus = EventBus::new();
        // Should not panic
        bus.emit("event", "src", serde_json::json!(null));
    }

    #[test]
    fn test_group_events_by_type() {
        let events = vec![
            Event::new("a", "src", serde_json::json!(null)),
            Event::new("b", "src", serde_json::json!(null)),
            Event::new("a", "src", serde_json::json!(null)),
        ];
        let groups = group_events_by_type(&events);
        assert_eq!(groups["a"].len(), 2);
        assert_eq!(groups["b"].len(), 1);
    }

    #[test]
    fn test_bus_clone() {
        let bus1 = EventBus::new();
        let _rx = bus1.subscribe();
        let bus2 = bus1.clone();
        assert_eq!(bus2.subscriber_count(), 1);
    }

    #[test]
    fn test_debug_format() {
        let bus = EventBus::new();
        let debug_str = format!("{:?}", bus);
        assert!(debug_str.contains("EventBus"));
    }
}
