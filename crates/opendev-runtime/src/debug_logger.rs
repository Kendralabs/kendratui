//! Per-session structured debug logger.
//!
//! Writes JSONL events to `~/.opendev/sessions/{session_id}.debug` when
//! verbose mode is enabled. Each line is a JSON object:
//! ```json
//! {"ts": "...", "elapsed_ms": 123, "event": "llm_call_start", "component": "react", "data": {...}}
//! ```
//!
//! Thread-safe via `Mutex`. Use [`SessionDebugLogger::noop()`] for zero-cost
//! disabled logging.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use serde_json::Value;

/// Maximum length for string values in event data.
const MAX_PREVIEW_LEN: usize = 200;

/// Truncate a serde_json::Value's string fields if too long.
fn truncate_value(value: &Value) -> Value {
    match value {
        Value::String(s) if s.len() > MAX_PREVIEW_LEN => {
            let total = s.len();
            Value::String(format!(
                "{}... ({total} chars)",
                &s[..MAX_PREVIEW_LEN]
            ))
        }
        Value::Object(map) => {
            let truncated: serde_json::Map<String, Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), truncate_value(v)))
                .collect();
            Value::Object(truncated)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(truncate_value).collect()),
        other => other.clone(),
    }
}

/// Per-session structured debug logger.
pub struct SessionDebugLogger {
    inner: Option<LoggerInner>,
}

struct LoggerInner {
    file_path: PathBuf,
    start_time: Instant,
    lock: Mutex<()>,
}

impl SessionDebugLogger {
    /// Create a new debug logger writing to `{session_dir}/{session_id}.debug`.
    pub fn new(session_dir: &Path, session_id: &str) -> Self {
        let file_path = session_dir.join(format!("{session_id}.debug"));

        // Ensure directory exists
        if let Some(parent) = file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        Self {
            inner: Some(LoggerInner {
                file_path,
                start_time: Instant::now(),
                lock: Mutex::new(()),
            }),
        }
    }

    /// Create a no-op logger that discards all events (zero overhead).
    pub fn noop() -> Self {
        Self { inner: None }
    }

    /// Whether this logger is active.
    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Path to the debug log file, if active.
    pub fn file_path(&self) -> Option<&Path> {
        self.inner.as_ref().map(|i| i.file_path.as_path())
    }

    /// Log a structured event.
    ///
    /// # Arguments
    /// - `event` — Event type (e.g., `"llm_call_start"`, `"tool_call_end"`)
    /// - `component` — Component name (e.g., `"react"`, `"tool"`, `"llm"`)
    /// - `data` — Arbitrary JSON data (string values truncated if too long)
    pub fn log(&self, event: &str, component: &str, data: Value) {
        let inner = match &self.inner {
            Some(i) => i,
            None => return,
        };

        let elapsed_ms = inner.start_time.elapsed().as_millis() as u64;
        let ts = chrono::Utc::now().to_rfc3339();

        let truncated_data = truncate_value(&data);

        let entry = serde_json::json!({
            "ts": ts,
            "elapsed_ms": elapsed_ms,
            "event": event,
            "component": component,
            "data": truncated_data,
        });

        let line = match serde_json::to_string(&entry) {
            Ok(s) => format!("{s}\n"),
            Err(_) => return,
        };

        let _guard = inner.lock.lock().ok();
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&inner.file_path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(line.as_bytes())
            });
    }
}

impl std::fmt::Debug for SessionDebugLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionDebugLogger")
            .field("enabled", &self.is_enabled())
            .field("file_path", &self.file_path())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_logger() {
        let logger = SessionDebugLogger::noop();
        assert!(!logger.is_enabled());
        assert!(logger.file_path().is_none());
        // Should not panic
        logger.log("test", "test", serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_active_logger() {
        let tmp = tempfile::TempDir::new().unwrap();
        let logger = SessionDebugLogger::new(tmp.path(), "test123");
        assert!(logger.is_enabled());
        assert!(logger.file_path().is_some());

        logger.log("event1", "comp1", serde_json::json!({"foo": "bar"}));
        logger.log("event2", "comp2", serde_json::json!({"count": 42}));

        let content = std::fs::read_to_string(logger.file_path().unwrap()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let entry: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(entry["event"], "event1");
        assert_eq!(entry["component"], "comp1");
        assert_eq!(entry["data"]["foo"], "bar");
    }

    #[test]
    fn test_truncation() {
        let long_string = "x".repeat(300);
        let data = serde_json::json!({"msg": long_string});
        let truncated = truncate_value(&data);

        let msg = truncated["msg"].as_str().unwrap();
        assert!(msg.len() < 300);
        assert!(msg.contains("300 chars"));
    }

    #[test]
    fn test_nested_truncation() {
        let long = "y".repeat(500);
        let data = serde_json::json!({
            "outer": {
                "inner": long
            }
        });
        let truncated = truncate_value(&data);
        let inner = truncated["outer"]["inner"].as_str().unwrap();
        assert!(inner.contains("500 chars"));
    }

    #[test]
    fn test_elapsed_ms() {
        let tmp = tempfile::TempDir::new().unwrap();
        let logger = SessionDebugLogger::new(tmp.path(), "elapsed_test");

        std::thread::sleep(std::time::Duration::from_millis(10));
        logger.log("test", "test", serde_json::json!({}));

        let content = std::fs::read_to_string(logger.file_path().unwrap()).unwrap();
        let entry: Value = serde_json::from_str(content.lines().next().unwrap()).unwrap();
        let elapsed = entry["elapsed_ms"].as_u64().unwrap();
        assert!(elapsed >= 10);
    }
}
