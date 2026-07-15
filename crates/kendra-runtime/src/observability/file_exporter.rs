//! File-based telemetry exporter for KendraCLI.
//!
//! Exports telemetry events to a file in NDJSON format.

use super::TelemetryExporter;
use crate::event_bus::RuntimeEvent;
use async_trait::async_trait;
use serde_json::Value;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// NDJSON file exporter.
pub struct FileTelemetryExporter {
    file: Mutex<File>,
}

impl FileTelemetryExporter {
    pub fn new(path: PathBuf) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

#[async_trait]
impl TelemetryExporter for FileTelemetryExporter {
    async fn export(&self, event: &Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json = serde_json::to_string(event)?;
        let mut file = self.file.lock().unwrap();
        writeln!(file, "{}", json)?;
        Ok(())
    }
}
