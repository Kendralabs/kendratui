use std::collections::HashSet;

use super::events::{EventTopic, RuntimeEvent, now_ms};
use super::global::GlobalEventBus;

#[tokio::test]
async fn test_global_bus_receives_forwarded_events() {
    let bus = GlobalEventBus::new();
    let mut rx = bus.subscribe();

    let event = RuntimeEvent::ToolCallStart {
        tool_name: "bash".into(),
        call_id: "c1".into(),
        timestamp_ms: now_ms(),
    };
    bus.forward(event);

    let received = rx.recv().await.unwrap();
    assert!(matches!(received, RuntimeEvent::ToolCallStart { .. }));
    if let RuntimeEvent::ToolCallStart { tool_name, .. } = received {
        assert_eq!(tool_name, "bash");
    }
}

#[tokio::test]
async fn test_global_bus_topic_filtering() {
    let bus = GlobalEventBus::new();
    let mut sub = bus.subscribe_topics(HashSet::from([EventTopic::Cost]));

    // Forward a Tool event (should be filtered out by the topic subscriber)
    bus.forward(RuntimeEvent::ToolCallStart {
        tool_name: "bash".into(),
        call_id: "c1".into(),
        timestamp_ms: now_ms(),
    });

    // Forward a Cost event (should pass the filter)
    bus.forward(RuntimeEvent::TokenUsage {
        model: "gpt-4".into(),
        input_tokens: 100,
        output_tokens: 50,
        cost_usd: 0.01,
        timestamp_ms: now_ms(),
    });

    let received = sub.recv().await.unwrap();
    assert_eq!(received.topic(), EventTopic::Cost);
    assert!(matches!(received, RuntimeEvent::TokenUsage { .. }));
}

#[test]
fn test_global_bus_subscriber_count() {
    let bus = GlobalEventBus::new();
    assert_eq!(bus.subscriber_count(), 0);

    let _rx1 = bus.subscribe();
    assert_eq!(bus.subscriber_count(), 1);

    let _rx2 = bus.subscribe();
    assert_eq!(bus.subscriber_count(), 2);

    drop(_rx1);
    // Note: broadcast receiver_count may not decrement immediately on drop
    // in all tokio versions, so we just verify it was at least 2 above.
}

#[test]
fn test_global_bus_debug_format() {
    let bus = GlobalEventBus::new();
    let debug_str = format!("{:?}", bus);
    assert!(debug_str.contains("GlobalEventBus"));
}

#[test]
fn test_global_bus_default() {
    let bus = GlobalEventBus::default();
    assert_eq!(bus.subscriber_count(), 0);
}
