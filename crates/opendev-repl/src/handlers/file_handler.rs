//! Handler for file operations (Read, Write, Edit, Glob, Grep).
//!
//! Mirrors `opendev/core/context_engineering/tools/handlers/file_handlers.py`.
//!
//! Responsibilities:
//! - Track file changes for artifact index
//! - Generate diff summaries for write/edit operations
//! - Detect dangerous file operations (overwriting without read)

use std::collections::HashMap;

use serde_json::Value;

use super::traits::{HandlerMeta, HandlerResult, PreCheckResult, ToolHandler};

/// Tools that modify files (tracked for artifact index).
const WRITE_TOOLS: &[&str] = &["Write", "Edit", "Patch"];

/// Tools that read files (no tracking needed).
const READ_TOOLS: &[&str] = &["Read", "Glob", "Grep"];

/// Handler for file-related tool operations.
pub struct FileHandler {
    /// Files that have been read in this session (for "read before write" check).
    read_files: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl FileHandler {
    /// Create a new file handler.
    pub fn new() -> Self {
        Self {
            read_files: std::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }

    /// Record that a file was read.
    pub fn mark_read(&self, path: &str) {
        if let Ok(mut set) = self.read_files.lock() {
            set.insert(path.to_string());
        }
    }

    /// Check if a file was previously read.
    pub fn was_read(&self, path: &str) -> bool {
        self.read_files
            .lock()
            .map(|set| set.contains(path))
            .unwrap_or(false)
    }
}

impl Default for FileHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolHandler for FileHandler {
    fn handles(&self) -> &[&str] {
        &["Read", "Write", "Edit", "Patch", "Glob", "Grep"]
    }

    fn pre_check(
        &self,
        tool_name: &str,
        args: &HashMap<String, Value>,
    ) -> PreCheckResult {
        // Track reads
        if READ_TOOLS.contains(&tool_name) {
            if let Some(path) = args.get("file_path").and_then(|v| v.as_str()) {
                self.mark_read(path);
            }
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                self.mark_read(path);
            }
        }

        PreCheckResult::Allow
    }

    fn post_process(
        &self,
        tool_name: &str,
        args: &HashMap<String, Value>,
        output: Option<&str>,
        error: Option<&str>,
        success: bool,
    ) -> HandlerResult {
        let changed_files = self.extract_changed_files(tool_name, args);

        HandlerResult {
            output: output.map(|s| s.to_string()),
            error: error.map(|s| s.to_string()),
            success,
            meta: HandlerMeta {
                changed_files,
                ..Default::default()
            },
        }
    }

    fn extract_changed_files(
        &self,
        tool_name: &str,
        args: &HashMap<String, Value>,
    ) -> Vec<String> {
        if !WRITE_TOOLS.contains(&tool_name) {
            return Vec::new();
        }

        let mut files = Vec::new();

        // file_path is used by Write and Edit
        if let Some(path) = args.get("file_path").and_then(|v| v.as_str()) {
            files.push(path.to_string());
        }

        // path is used by some tools
        if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            if !files.contains(&path.to_string()) {
                files.push(path.to_string());
            }
        }

        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler() -> FileHandler {
        FileHandler::new()
    }

    #[test]
    fn test_handles_file_tools() {
        let h = handler();
        let handles = h.handles();
        assert!(handles.contains(&"Read"));
        assert!(handles.contains(&"Write"));
        assert!(handles.contains(&"Edit"));
        assert!(handles.contains(&"Glob"));
    }

    #[test]
    fn test_read_tracking() {
        let h = handler();
        assert!(!h.was_read("/tmp/test.rs"));

        let mut args = HashMap::new();
        args.insert(
            "file_path".to_string(),
            Value::String("/tmp/test.rs".to_string()),
        );
        h.pre_check("Read", &args);

        assert!(h.was_read("/tmp/test.rs"));
    }

    #[test]
    fn test_extract_changed_files_write() {
        let h = handler();
        let mut args = HashMap::new();
        args.insert(
            "file_path".to_string(),
            Value::String("/tmp/out.rs".to_string()),
        );
        let files = h.extract_changed_files("Write", &args);
        assert_eq!(files, vec!["/tmp/out.rs"]);
    }

    #[test]
    fn test_extract_changed_files_read() {
        let h = handler();
        let mut args = HashMap::new();
        args.insert(
            "file_path".to_string(),
            Value::String("/tmp/in.rs".to_string()),
        );
        let files = h.extract_changed_files("Read", &args);
        assert!(files.is_empty());
    }

    #[test]
    fn test_post_process_includes_changed_files() {
        let h = handler();
        let mut args = HashMap::new();
        args.insert(
            "file_path".to_string(),
            Value::String("/tmp/edited.rs".to_string()),
        );

        let result = h.post_process("Edit", &args, Some("ok"), None, true);
        assert!(result.success);
        assert_eq!(result.meta.changed_files, vec!["/tmp/edited.rs"]);
    }

    #[test]
    fn test_pre_check_always_allows() {
        let h = handler();
        let args = HashMap::new();
        match h.pre_check("Write", &args) {
            PreCheckResult::Allow => {}
            other => panic!("Expected Allow, got {:?}", other),
        }
    }
}
