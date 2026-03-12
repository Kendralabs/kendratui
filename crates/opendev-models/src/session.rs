//! Session management models.

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::file_change::{FileChange, FileChangeType};
use crate::message::ChatMessage;

/// Session metadata for listing and searching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: String,
    #[serde(with = "crate::datetime_compat")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "crate::datetime_compat")]
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub has_session_model: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,

    // Summary stats
    #[serde(default)]
    pub summary_additions: u64,
    #[serde(default)]
    pub summary_deletions: u64,
    #[serde(default)]
    pub summary_files: u64,

    // Multi-channel fields
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default)]
    pub channel_user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

fn default_channel() -> String {
    "cli".to_string()
}

fn generate_session_id() -> String {
    Uuid::new_v4().to_string()[..12].to_string()
}

/// Represents a conversation session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(default = "generate_session_id")]
    pub id: String,
    #[serde(default = "Utc::now", with = "crate::datetime_compat")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now", with = "crate::datetime_compat")]
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub context_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Serialized ACE Playbook.
    #[serde(default)]
    pub playbook: Option<HashMap<String, serde_json::Value>>,
    /// Track file changes in this session.
    #[serde(default)]
    pub file_changes: Vec<FileChange>,

    // Multi-channel fields
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default = "default_chat_type")]
    pub chat_type: String,
    #[serde(default)]
    pub channel_user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub delivery_context: HashMap<String, serde_json::Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::datetime_compat::option"
    )]
    pub last_activity: Option<DateTime<Utc>>,
    #[serde(default)]
    pub workspace_confirmed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    /// ID of parent session (if forked).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// tool_call_id -> child session_id
    #[serde(default)]
    pub subagent_sessions: HashMap<String, String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::datetime_compat::option"
    )]
    pub time_archived: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
}

fn default_chat_type() -> String {
    "direct".to_string()
}

impl Session {
    /// Create a new session with defaults.
    pub fn new() -> Self {
        Self {
            id: generate_session_id(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            messages: Vec::new(),
            context_files: Vec::new(),
            working_directory: None,
            metadata: HashMap::new(),
            playbook: Some(HashMap::new()),
            file_changes: Vec::new(),
            channel: "cli".to_string(),
            chat_type: "direct".to_string(),
            channel_user_id: String::new(),
            thread_id: None,
            delivery_context: HashMap::new(),
            last_activity: None,
            workspace_confirmed: false,
            owner_id: None,
            parent_id: None,
            subagent_sessions: HashMap::new(),
            time_archived: None,
            slug: None,
        }
    }

    /// Total lines added across all file changes.
    pub fn summary_additions(&self) -> u64 {
        self.file_changes.iter().map(|fc| fc.lines_added).sum()
    }

    /// Total lines removed across all file changes.
    pub fn summary_deletions(&self) -> u64 {
        self.file_changes.iter().map(|fc| fc.lines_removed).sum()
    }

    /// Number of unique files changed.
    pub fn summary_files(&self) -> usize {
        let unique: std::collections::HashSet<&str> = self
            .file_changes
            .iter()
            .map(|fc| fc.file_path.as_str())
            .collect();
        unique.len()
    }

    /// Soft-archive this session.
    pub fn archive(&mut self) {
        self.time_archived = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Restore an archived session.
    pub fn unarchive(&mut self) {
        self.time_archived = None;
        self.updated_at = Utc::now();
    }

    /// Check if session is archived.
    pub fn is_archived(&self) -> bool {
        self.time_archived.is_some()
    }

    /// Generate URL-friendly slug from title.
    pub fn generate_slug(&self, title: Option<&str>) -> String {
        let text = title
            .or_else(|| self.metadata.get("title").and_then(|v| v.as_str()))
            .unwrap_or("");

        if text.is_empty() {
            return self.id[..self.id.len().min(8)].to_string();
        }

        let re = Regex::new(r"[^a-z0-9]+").unwrap();
        let lowered = text.to_lowercase();
        let slug = re.replace_all(&lowered, "-");
        let slug = slug.trim_matches('-');
        let slug = if slug.len() > 50 {
            slug[..50].trim_end_matches('-')
        } else {
            slug
        };

        if slug.is_empty() {
            self.id[..self.id.len().min(8)].to_string()
        } else {
            slug.to_string()
        }
    }

    /// Add a file change to the session.
    pub fn add_file_change(&mut self, file_change: FileChange) {
        // Check if this is a modification of an existing file
        for existing in &mut self.file_changes {
            if existing.file_path == file_change.file_path
                && existing.change_type == FileChangeType::Modified
                && file_change.change_type == FileChangeType::Modified
            {
                existing.lines_added += file_change.lines_added;
                existing.lines_removed += file_change.lines_removed;
                existing.timestamp = file_change.timestamp;
                existing.description = file_change.description.clone();
                return;
            }
        }

        // Remove any previous change for the same file (for non-modifications)
        self.file_changes
            .retain(|fc| fc.file_path != file_change.file_path);

        let mut fc = file_change;
        fc.session_id = Some(self.id.clone());
        self.file_changes.push(fc);
        self.updated_at = Utc::now();
    }

    /// Get a summary of file changes in this session.
    pub fn get_file_changes_summary(&self) -> FileChangesSummary {
        let created = self
            .file_changes
            .iter()
            .filter(|fc| fc.change_type == FileChangeType::Created)
            .count();
        let modified = self
            .file_changes
            .iter()
            .filter(|fc| fc.change_type == FileChangeType::Modified)
            .count();
        let deleted = self
            .file_changes
            .iter()
            .filter(|fc| fc.change_type == FileChangeType::Deleted)
            .count();
        let renamed = self
            .file_changes
            .iter()
            .filter(|fc| fc.change_type == FileChangeType::Renamed)
            .count();
        let total_lines_added: u64 = self.file_changes.iter().map(|fc| fc.lines_added).sum();
        let total_lines_removed: u64 = self.file_changes.iter().map(|fc| fc.lines_removed).sum();

        FileChangesSummary {
            total: self.file_changes.len(),
            created,
            modified,
            deleted,
            renamed,
            total_lines_added,
            total_lines_removed,
            net_lines: total_lines_added as i64 - total_lines_removed as i64,
        }
    }

    /// Calculate total token count.
    pub fn total_tokens(&self) -> u64 {
        self.messages.iter().map(|msg| msg.token_estimate()).sum()
    }

    /// Get session metadata.
    pub fn get_metadata(&self) -> SessionMetadata {
        SessionMetadata {
            id: self.id.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            message_count: self.messages.len(),
            total_tokens: self.total_tokens(),
            title: self
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .map(String::from),
            summary: self
                .metadata
                .get("summary")
                .and_then(|v| v.as_str())
                .map(String::from),
            tags: self
                .metadata
                .get("tags")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default(),
            working_directory: self.working_directory.clone(),
            has_session_model: false,
            owner_id: self.owner_id.clone(),
            summary_additions: self.summary_additions(),
            summary_deletions: self.summary_deletions(),
            summary_files: self.summary_files() as u64,
            channel: self.channel.clone(),
            channel_user_id: self.channel_user_id.clone(),
            thread_id: self.thread_id.clone(),
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of file changes in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangesSummary {
    pub total: usize,
    pub created: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
    pub total_lines_added: u64,
    pub total_lines_removed: u64,
    pub net_lines: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let session = Session::new();
        assert!(!session.id.is_empty());
        assert_eq!(session.channel, "cli");
        assert_eq!(session.chat_type, "direct");
        assert!(!session.is_archived());
    }

    #[test]
    fn test_session_roundtrip() {
        let session = Session::new();
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, session.id);
        assert_eq!(deserialized.channel, "cli");
    }

    #[test]
    fn test_archive_unarchive() {
        let mut session = Session::new();
        assert!(!session.is_archived());

        session.archive();
        assert!(session.is_archived());

        session.unarchive();
        assert!(!session.is_archived());
    }

    #[test]
    fn test_generate_slug() {
        let session = Session::new();

        assert_eq!(
            session.generate_slug(Some("Hello World Test")),
            "hello-world-test"
        );
        assert_eq!(
            session.generate_slug(Some("Special @#$ Characters!")),
            "special-characters"
        );
        // Empty title falls back to session ID prefix
        let slug = session.generate_slug(Some(""));
        assert_eq!(slug.len(), session.id.len().min(8));
    }

    #[test]
    fn test_file_changes_summary() {
        let mut session = Session::new();

        session.add_file_change(FileChange {
            change_type: FileChangeType::Created,
            file_path: "src/main.rs".to_string(),
            lines_added: 50,
            ..FileChange::new(FileChangeType::Created, "src/main.rs".to_string())
        });

        session.add_file_change(FileChange {
            change_type: FileChangeType::Modified,
            file_path: "src/lib.rs".to_string(),
            lines_added: 10,
            lines_removed: 5,
            ..FileChange::new(FileChangeType::Modified, "src/lib.rs".to_string())
        });

        let summary = session.get_file_changes_summary();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.created, 1);
        assert_eq!(summary.modified, 1);
        assert_eq!(summary.total_lines_added, 60);
        assert_eq!(summary.total_lines_removed, 5);
        assert_eq!(summary.net_lines, 55);
    }

    /// Verify that a JSON string matching the Python session format can be
    /// deserialized into the Rust Session struct.  This guards against
    /// serialization drift between the Python and Rust codebases.
    #[test]
    fn test_python_session_compat() {
        let python_json = r#"{
            "id": "a1b2c3d4e5f6",
            "created_at": "2025-06-15T10:30:00Z",
            "updated_at": "2025-06-15T11:45:00Z",
            "messages": [
                {
                    "role": "user",
                    "content": "Hello from Python",
                    "timestamp": "2025-06-15T10:30:00Z",
                    "metadata": {},
                    "tool_calls": [],
                    "tokens": null
                },
                {
                    "role": "assistant",
                    "content": "Hi! How can I help?",
                    "timestamp": "2025-06-15T10:30:05Z",
                    "metadata": {"model": "gpt-4"},
                    "tool_calls": [
                        {
                            "id": "tc-001",
                            "name": "read_file",
                            "parameters": {"path": "/tmp/test.py"},
                            "result": "print('hello')",
                            "result_summary": "Read 1 line",
                            "timestamp": "2025-06-15T10:30:03Z",
                            "approved": true,
                            "error": null,
                            "nested_tool_calls": []
                        }
                    ],
                    "tokens": 42,
                    "thinking_trace": "I should read the file first.",
                    "reasoning_content": null,
                    "token_usage": {"prompt_tokens": 100, "completion_tokens": 42},
                    "provenance": null
                }
            ],
            "context_files": ["src/main.py"],
            "working_directory": "/home/user/project",
            "metadata": {"title": "Test session", "tags": ["rust", "python"]},
            "playbook": {"strategy": "default"},
            "file_changes": [
                {
                    "id": "fc-001",
                    "type": "modified",
                    "file_path": "src/main.py",
                    "timestamp": "2025-06-15T11:00:00Z",
                    "lines_added": 10,
                    "lines_removed": 3
                }
            ],
            "channel": "cli",
            "chat_type": "direct",
            "channel_user_id": "",
            "thread_id": null,
            "delivery_context": {},
            "last_activity": "2025-06-15T11:45:00Z",
            "workspace_confirmed": true,
            "owner_id": "user-123",
            "parent_id": null,
            "subagent_sessions": {"tc-agent-1": "child-sess-1"},
            "time_archived": null,
            "slug": "test-session"
        }"#;

        let session: Session = serde_json::from_str(python_json)
            .expect("Python session JSON must deserialize into Rust Session");

        assert_eq!(session.id, "a1b2c3d4e5f6");
        assert_eq!(session.channel, "cli");
        assert_eq!(session.chat_type, "direct");
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, crate::message::Role::User);
        assert_eq!(session.messages[0].content, "Hello from Python");
        assert_eq!(session.messages[1].tool_calls.len(), 1);
        assert_eq!(session.messages[1].tool_calls[0].name, "read_file");
        assert!(session.messages[1].thinking_trace.is_some());
        assert_eq!(session.context_files, vec!["src/main.py"]);
        assert_eq!(
            session.working_directory.as_deref(),
            Some("/home/user/project")
        );
        assert!(session.workspace_confirmed);
        assert_eq!(session.owner_id.as_deref(), Some("user-123"));
        assert_eq!(session.file_changes.len(), 1);
        assert_eq!(session.file_changes[0].lines_added, 10);
        assert_eq!(session.file_changes[0].lines_removed, 3);
        assert_eq!(
            session
                .subagent_sessions
                .get("tc-agent-1")
                .map(String::as_str),
            Some("child-sess-1")
        );
        assert_eq!(session.slug.as_deref(), Some("test-session"));
        assert!(session.last_activity.is_some());
        assert!(!session.is_archived());

        // Round-trip: serialize back and re-deserialize
        let reserialized = serde_json::to_string(&session).unwrap();
        let roundtrip: Session = serde_json::from_str(&reserialized).unwrap();
        assert_eq!(roundtrip.id, session.id);
        assert_eq!(roundtrip.messages.len(), session.messages.len());
    }

    /// Verify that a minimal Python session (only required fields) deserializes
    /// correctly, with all optional/default fields populated.
    #[test]
    fn test_python_minimal_session_compat() {
        let minimal_json = r#"{}"#;
        let session: Session = serde_json::from_str(minimal_json)
            .expect("Empty JSON object must deserialize with defaults");

        assert!(!session.id.is_empty());
        assert_eq!(session.channel, "cli");
        assert_eq!(session.chat_type, "direct");
        assert!(session.messages.is_empty());
        assert!(session.file_changes.is_empty());
        assert!(session.working_directory.is_none());
        assert!(!session.workspace_confirmed);
    }
}
