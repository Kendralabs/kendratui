use super::*;
use crate::event_bus::EventBus;
use kendra_models::config::GovernanceConfig;
use std::sync::Arc;
use tokio::sync::Mutex;

struct MockExporter {
    events: Arc<Mutex<Vec<Value>>>,
}

#[async_trait]
impl TelemetryExporter for MockExporter {
    async fn export(&self, event: &Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.events.lock().await.push(event.clone());
        Ok(())
    }
}

#[tokio::test]
async fn test_observability_manager_collection_and_enrichment() {
    let config = ObservabilityConfig {
        enabled: true,
        sinks: vec![],
        governance: GovernanceConfig {
            sanitize_pii: false,
            redact_keys: vec!["secret_key".to_string()],
        },
    };

    let mut manager = ObservabilityManager::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    manager.add_exporter(Box::new(MockExporter {
        events: events.clone(),
    }));

    let event_bus = EventBus::new();
    manager.run(&event_bus);

    // Publish an event
    event_bus.publish(RuntimeEvent::SessionStart {
        session_id: "test-session".to_string(),
        timestamp_ms: 0,
    });

    // Give it a moment to process
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let captured = events.lock().await;
    assert_eq!(captured.len(), 1);

    let event = &captured[0];
    assert!(event.get("header").is_some());
    assert_eq!(event["header"]["cli_name"], "kendra-cli");
    assert_eq!(event["type"], "SessionStart");
}

#[tokio::test]
async fn test_observability_manager_redaction() {
    let config = ObservabilityConfig {
        enabled: true,
        sinks: vec![],
        governance: GovernanceConfig {
            sanitize_pii: false,
            redact_keys: vec!["session_id".to_string()], // Redact session_id for test
        },
    };

    let mut manager = ObservabilityManager::new(config);
    let events = Arc::new(Mutex::new(Vec::new()));
    manager.add_exporter(Box::new(MockExporter {
        events: events.clone(),
    }));

    let event_bus = EventBus::new();
    manager.run(&event_bus);

    event_bus.publish(RuntimeEvent::SessionStart {
        session_id: "sensitive-id".to_string(),
        timestamp_ms: 0,
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let captured = events.lock().await;
    let event = &captured[0];

    // session_id should be missing from the payload
    assert!(event.get("session_id").is_none());
}
