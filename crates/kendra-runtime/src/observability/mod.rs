//! Pluggable observability system for KendraCLI.
//!
//! Captures system-wide events via [`EventBus`] and exports them to configured sinks.

use crate::event_bus::{EventBus, RuntimeEvent};
use async_trait::async_trait;
use kendra_models::config::ObservabilityConfig;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::error;

/// Trait for exporting telemetry events.
#[async_trait]
pub trait TelemetryExporter: Send + Sync {
    async fn export(&self, event: &Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Orchestrates telemetry collection and exporting.
pub struct ObservabilityManager {
    exporters: Vec<Box<dyn TelemetryExporter>>,
    config: ObservabilityConfig,
}

impl ObservabilityManager {
    pub fn new(config: ObservabilityConfig) -> Self {
        Self {
            exporters: Vec::new(),
            config,
        }
    }

    pub fn add_exporter(&mut self, exporter: Box<dyn TelemetryExporter>) {
        self.exporters.push(exporter);
    }

    fn sanitize_and_enrich(config: &ObservabilityConfig, event: &RuntimeEvent) -> Value {
        let mut value = serde_json::to_value(event).unwrap_or_default();

        // 1. Enrich with headers
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "header".to_string(),
                json!({
                    "cli_name": "kendra-cli",
                    "cli_version": "0.1.0",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }),
            );
        }

        // 2. Apply governance
        if config.governance.sanitize_pii {
            // (Placeholder for PII sanitization logic)
        }

        for key in &config.governance.redact_keys {
            if let Some(obj) = value.as_object_mut() {
                obj.remove(key);
            }
        }

        value
    }

    /// Start the telemetry collection loop.
    pub fn run(self, event_bus: &EventBus) {
        if !self.config.enabled {
            return;
        }

        let mut rx = event_bus.subscribe();
        let config = self.config;
        let exporters = Arc::new(self.exporters);

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let processed_event = Self::sanitize_and_enrich(&config, &event);
                        for exporter in exporters.iter() {
                            if let Err(e) = exporter.export(&processed_event).await {
                                error!("Failed to export telemetry: {}", e);
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        error!("Telemetry collector lagged by {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}

#[cfg(test)]
mod tests;
