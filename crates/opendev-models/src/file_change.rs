//! File change tracking models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Types of file changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// Represents a file change within a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    #[serde(default = "generate_change_id")]
    pub id: String,
    #[serde(rename = "type")]
    pub change_type: FileChangeType,
    pub file_path: String,
    /// For renames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    #[serde(default = "Utc::now", with = "crate::datetime_compat")]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub lines_added: u64,
    #[serde(default)]
    pub lines_removed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn generate_change_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

impl FileChange {
    /// Create a new FileChange with defaults.
    pub fn new(change_type: FileChangeType, file_path: String) -> Self {
        Self {
            id: generate_change_id(),
            change_type,
            file_path,
            old_path: None,
            timestamp: Utc::now(),
            lines_added: 0,
            lines_removed: 0,
            tool_call_id: None,
            session_id: None,
            description: None,
        }
    }

    /// Get file icon based on type.
    pub fn get_file_icon(&self) -> &'static str {
        match self.change_type {
            FileChangeType::Created => "+",
            FileChangeType::Modified => "~",
            FileChangeType::Deleted => "-",
            FileChangeType::Renamed => ">",
        }
    }

    /// Get status color for UI display.
    pub fn get_status_color(&self) -> &'static str {
        match self.change_type {
            FileChangeType::Created => "green",
            FileChangeType::Modified => "blue",
            FileChangeType::Deleted => "red",
            FileChangeType::Renamed => "orange",
        }
    }

    /// Get human-readable change summary.
    pub fn get_change_summary(&self) -> String {
        match self.change_type {
            FileChangeType::Created => "New file".to_string(),
            FileChangeType::Modified => {
                if self.lines_added > 0 && self.lines_removed > 0 {
                    format!("+{} -{}", self.lines_added, self.lines_removed)
                } else if self.lines_added > 0 {
                    format!("+{}", self.lines_added)
                } else if self.lines_removed > 0 {
                    format!("-{}", self.lines_removed)
                } else {
                    "Modified".to_string()
                }
            }
            FileChangeType::Deleted => "Deleted".to_string(),
            FileChangeType::Renamed => format!("Renamed -> {}", self.file_path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_change_type_serialization() {
        let ct = FileChangeType::Created;
        let json = serde_json::to_string(&ct).unwrap();
        assert_eq!(json, "\"created\"");

        let deserialized: FileChangeType = serde_json::from_str("\"modified\"").unwrap();
        assert_eq!(deserialized, FileChangeType::Modified);
    }

    #[test]
    fn test_file_change_roundtrip() {
        let fc = FileChange::new(FileChangeType::Modified, "src/main.rs".to_string());
        let json = serde_json::to_string(&fc).unwrap();
        let deserialized: FileChange = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_path, "src/main.rs");
        assert_eq!(deserialized.change_type, FileChangeType::Modified);
    }

    #[test]
    fn test_change_summary() {
        let fc = FileChange {
            lines_added: 10,
            lines_removed: 3,
            ..FileChange::new(FileChangeType::Modified, "test.rs".to_string())
        };
        assert_eq!(fc.get_change_summary(), "+10 -3");

        let created = FileChange::new(FileChangeType::Created, "new.rs".to_string());
        assert_eq!(created.get_change_summary(), "New file");
    }

    #[test]
    fn test_file_icons() {
        assert_eq!(
            FileChange::new(FileChangeType::Created, "a".to_string()).get_file_icon(),
            "+"
        );
        assert_eq!(
            FileChange::new(FileChangeType::Deleted, "a".to_string()).get_file_icon(),
            "-"
        );
    }
}
