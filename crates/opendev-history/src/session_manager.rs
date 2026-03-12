//! Session manager: JSON read/write to ~/.opendev/sessions/.
//!
//! Handles session file I/O, including reading from both legacy JSON format
//! (all-in-one) and the newer JSON+JSONL split format.

use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use opendev_models::Session;

use crate::index::SessionIndex;

/// Session manager for persisting and loading sessions.
pub struct SessionManager {
    session_dir: PathBuf,
    index: SessionIndex,
    current_session: Option<Session>,
}

impl SessionManager {
    /// Create a new session manager.
    ///
    /// The `session_dir` is typically `~/.opendev/projects/{encoded-path}/`
    /// or `~/.opendev/sessions/` for the legacy global directory.
    pub fn new(session_dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&session_dir)?;
        let index = SessionIndex::new(session_dir.clone());
        Ok(Self {
            session_dir,
            index,
            current_session: None,
        })
    }

    /// Get the session directory.
    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    /// Get the current session (if any).
    pub fn current_session(&self) -> Option<&Session> {
        self.current_session.as_ref()
    }

    /// Get mutable reference to the current session.
    pub fn current_session_mut(&mut self) -> Option<&mut Session> {
        self.current_session.as_mut()
    }

    /// Set the current session.
    pub fn set_current_session(&mut self, session: Session) {
        self.current_session = Some(session);
    }

    /// Create a new session and set it as current.
    pub fn create_session(&mut self) -> &Session {
        let session = Session::new();
        info!("Created new session: {}", session.id);
        self.current_session = Some(session);
        self.current_session.as_ref().unwrap()
    }

    /// Save a session to disk.
    ///
    /// Writes session metadata to `{id}.json` and messages to `{id}.jsonl`.
    pub fn save_session(&self, session: &Session) -> std::io::Result<()> {
        let json_path = self.session_dir.join(format!("{}.json", session.id));
        let jsonl_path = self.session_dir.join(format!("{}.jsonl", session.id));

        // Write metadata (session without messages for the JSON file)
        let mut session_for_json = session.clone();
        session_for_json.messages.clear();

        let json_content = serde_json::to_string_pretty(&session_for_json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Atomic write for metadata
        let tmp_json = self.session_dir.join(format!(".{}.json.tmp", session.id));
        std::fs::write(&tmp_json, &json_content)?;
        std::fs::rename(&tmp_json, &json_path)?;

        // Write messages as JSONL
        let mut jsonl_content = String::new();
        for msg in &session.messages {
            let line = serde_json::to_string(msg)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            jsonl_content.push_str(&line);
            jsonl_content.push('\n');
        }

        let tmp_jsonl = self.session_dir.join(format!(".{}.jsonl.tmp", session.id));
        std::fs::write(&tmp_jsonl, &jsonl_content)?;
        std::fs::rename(&tmp_jsonl, &jsonl_path)?;

        // Update index
        if let Err(e) = self.index.upsert_entry(session) {
            warn!("Failed to update session index: {}", e);
        }

        debug!("Saved session {} ({} messages)", session.id, session.messages.len());
        Ok(())
    }

    /// Save the current session.
    pub fn save_current(&self) -> std::io::Result<()> {
        if let Some(session) = &self.current_session {
            self.save_session(session)
        } else {
            Ok(())
        }
    }

    /// Load a session from disk.
    ///
    /// Reads from both the JSON metadata file and JSONL transcript.
    /// Falls back to reading messages from the JSON file for legacy format.
    pub fn load_session(&self, session_id: &str) -> std::io::Result<Session> {
        let json_path = self.session_dir.join(format!("{session_id}.json"));
        self.load_from_file(&json_path)
    }

    /// Load a session from a specific file path.
    pub fn load_from_file(&self, json_path: &Path) -> std::io::Result<Session> {
        let content = std::fs::read_to_string(json_path)?;
        let mut session: Session = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Try to load messages from JSONL file
        let jsonl_path = json_path.with_extension("jsonl");
        if jsonl_path.exists() {
            let jsonl_content = std::fs::read_to_string(&jsonl_path)?;
            let mut messages = Vec::new();
            for line in jsonl_content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str(line) {
                    Ok(msg) => messages.push(msg),
                    Err(e) => {
                        warn!("Skipping invalid JSONL line: {}", e);
                    }
                }
            }
            if !messages.is_empty() {
                session.messages = messages;
            }
        }
        // If no JSONL file, messages from the JSON file are used (legacy format)

        debug!(
            "Loaded session {} ({} messages)",
            session.id,
            session.messages.len()
        );
        Ok(session)
    }

    /// Load a session and set it as current.
    pub fn resume_session(&mut self, session_id: &str) -> std::io::Result<&Session> {
        let session = self.load_session(session_id)?;
        self.current_session = Some(session);
        Ok(self.current_session.as_ref().unwrap())
    }

    /// Get the session index.
    pub fn index(&self) -> &SessionIndex {
        &self.index
    }

    /// Store a string value in the current session's metadata.
    ///
    /// Useful for persisting mode, thinking level, autonomy level, etc.
    pub fn set_metadata(&mut self, key: &str, value: &str) {
        if let Some(session) = &mut self.current_session {
            session
                .metadata
                .insert(key.to_string(), serde_json::Value::String(value.to_string()));
        }
    }

    /// Read a string value from the current session's metadata.
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        self.current_session
            .as_ref()
            .and_then(|s| s.metadata.get(key))
            .and_then(|v| v.as_str())
            .map(String::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use opendev_models::{ChatMessage, Role};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_msg(role: Role, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: content.to_string(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            tool_calls: vec![],
            tokens: None,
            thinking_trace: None,
            reasoning_content: None,
            token_usage: None,
            provenance: None,
        }
    }

    #[test]
    fn test_create_session() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();
        let session = mgr.create_session();
        assert!(!session.id.is_empty());
        assert!(mgr.current_session().is_some());
    }

    #[test]
    fn test_save_and_load_session() {
        let tmp = TempDir::new().unwrap();
        let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

        let mut session = Session::new();
        session.id = "test-save-load".to_string();
        session
            .messages
            .push(make_msg(Role::User, "hello"));
        session
            .messages
            .push(make_msg(Role::Assistant, "hi there"));

        mgr.save_session(&session).unwrap();

        let loaded = mgr.load_session("test-save-load").unwrap();
        assert_eq!(loaded.id, "test-save-load");
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].content, "hello");
        assert_eq!(loaded.messages[1].content, "hi there");
    }

    #[test]
    fn test_save_updates_index() {
        let tmp = TempDir::new().unwrap();
        let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

        let mut session = Session::new();
        session.id = "indexed-session".to_string();
        mgr.save_session(&session).unwrap();

        let index = mgr.index().read_index().unwrap();
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].session_id, "indexed-session");
    }

    #[test]
    fn test_resume_session() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

        let mut session = Session::new();
        session.id = "resume-test".to_string();
        session.messages.push(make_msg(Role::User, "hi"));
        mgr.save_session(&session).unwrap();

        mgr.resume_session("resume-test").unwrap();
        let current = mgr.current_session().unwrap();
        assert_eq!(current.id, "resume-test");
        assert_eq!(current.messages.len(), 1);
    }

    #[test]
    fn test_load_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();
        let result = mgr.load_session("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_legacy_json_format() {
        // Test loading from legacy format (messages in JSON, no JSONL)
        let tmp = TempDir::new().unwrap();
        let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

        let mut session = Session::new();
        session.id = "legacy-test".to_string();
        session.messages.push(make_msg(Role::User, "old format"));

        // Write as legacy format (all in JSON, no JSONL)
        let json_path = tmp.path().join("legacy-test.json");
        let content = serde_json::to_string_pretty(&session).unwrap();
        std::fs::write(&json_path, content).unwrap();

        let loaded = mgr.load_session("legacy-test").unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].content, "old format");
    }

    #[test]
    fn test_set_get_metadata() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.create_session();

        // No metadata set yet
        assert!(mgr.get_metadata("mode").is_none());

        // Set and get
        mgr.set_metadata("mode", "PLAN");
        assert_eq!(mgr.get_metadata("mode").as_deref(), Some("PLAN"));

        mgr.set_metadata("thinking_level", "High");
        assert_eq!(
            mgr.get_metadata("thinking_level").as_deref(),
            Some("High")
        );

        mgr.set_metadata("autonomy_level", "Auto");
        assert_eq!(
            mgr.get_metadata("autonomy_level").as_deref(),
            Some("Auto")
        );
    }

    #[test]
    fn test_metadata_persists_across_save_load() {
        let tmp = TempDir::new().unwrap();
        let mut mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();
        mgr.create_session();

        let session_id = mgr.current_session().unwrap().id.clone();

        mgr.set_metadata("mode", "PLAN");
        mgr.set_metadata("thinking_level", "High");
        mgr.set_metadata("autonomy_level", "Manual");
        mgr.save_current().unwrap();

        // Load in a fresh manager
        let mgr2 = SessionManager::new(tmp.path().to_path_buf()).unwrap();
        let loaded = mgr2.load_session(&session_id).unwrap();

        assert_eq!(
            loaded.metadata.get("mode").and_then(|v| v.as_str()),
            Some("PLAN")
        );
        assert_eq!(
            loaded
                .metadata
                .get("thinking_level")
                .and_then(|v| v.as_str()),
            Some("High")
        );
        assert_eq!(
            loaded
                .metadata
                .get("autonomy_level")
                .and_then(|v| v.as_str()),
            Some("Manual")
        );
    }
}
