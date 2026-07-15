#![allow(clippy::collapsible_if)]
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use tracing::error;

/// A record of a permission evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionAuditEntry {
    pub timestamp: DateTime<Utc>,
    pub tool_name: String,
    pub args: String,
    pub action: super::PermissionAction,
    pub directory_scope: Option<String>,
}

/// Simple file-based audit logger.
#[derive(Debug, Clone)]
pub struct PermissionAuditLogger {
    log_path: PathBuf,
}

impl PermissionAuditLogger {
    pub fn new(log_path: PathBuf) -> Self {
        Self { log_path }
    }

    pub fn log(&self, entry: PermissionAuditEntry) {
        if let Ok(file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
        {
            let mut file = file;
            if let Ok(json) = serde_json::to_string(&entry) {
                if let Err(e) = writeln!(file, "{}", json) {
                    error!("Failed to write audit log: {}", e);
                }
            }
        } else {
            error!("Failed to open audit log file: {:?}", self.log_path);
        }
    }
}
