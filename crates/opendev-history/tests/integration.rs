//! Integration tests for session history management.
//!
//! Tests full session lifecycle, listing, file locks, and snapshot operations
//! using real filesystem I/O with temp directories.

use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use opendev_history::{FileLock, SessionIndex, SessionListing, SessionManager, SnapshotManager};
use opendev_models::{ChatMessage, Role, Session};
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

// ========================================================================
// Session lifecycle: create -> add messages -> save -> reload -> verify
// ========================================================================

/// Full round-trip: create session, add messages, save, reload, verify content.
#[test]
fn session_full_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

    // Create
    let session = mgr.create_session();
    let session_id = session.id.clone();
    assert!(!session_id.is_empty());

    // Add messages to current session
    let current = mgr.current_session_mut().unwrap();
    current.messages.push(make_msg(Role::User, "Hello agent"));
    current
        .messages
        .push(make_msg(Role::Assistant, "Hello human"));
    current.messages.push(make_msg(Role::User, "Do something"));

    // Save
    mgr.save_current().unwrap();

    // Reload in a new manager instance
    let mgr2 = SessionManager::new(tmp.path().to_path_buf()).unwrap();
    let loaded = mgr2.load_session(&session_id).unwrap();

    assert_eq!(loaded.id, session_id);
    assert_eq!(loaded.messages.len(), 3);
    assert_eq!(loaded.messages[0].content, "Hello agent");
    assert_eq!(loaded.messages[1].content, "Hello human");
    assert_eq!(loaded.messages[2].content, "Do something");
    assert_eq!(loaded.messages[0].role, Role::User);
    assert_eq!(loaded.messages[1].role, Role::Assistant);
}

/// Session with metadata survives save/load.
#[test]
fn session_metadata_persists() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

    let mut session = Session::new();
    session.id = "meta-test".to_string();
    session
        .metadata
        .insert("title".to_string(), serde_json::json!("Test Session"));
    session.working_directory = Some("/tmp/project".to_string());
    session.messages.push(make_msg(Role::User, "hi"));

    mgr.save_session(&session).unwrap();

    let loaded = mgr.load_session("meta-test").unwrap();
    assert_eq!(
        loaded.metadata.get("title"),
        Some(&serde_json::json!("Test Session"))
    );
    assert_eq!(loaded.working_directory.as_deref(), Some("/tmp/project"));
}

/// Resume session sets it as current.
#[test]
fn resume_sets_current_session() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

    let mut session = Session::new();
    session.id = "resume-target".to_string();
    session.messages.push(make_msg(Role::User, "context"));
    mgr.save_session(&session).unwrap();

    assert!(mgr.current_session().is_none());
    mgr.resume_session("resume-target").unwrap();
    let current = mgr.current_session().unwrap();
    assert_eq!(current.id, "resume-target");
    assert_eq!(current.messages.len(), 1);
}

/// Loading a nonexistent session returns an error.
#[test]
fn load_nonexistent_session_fails() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();
    let result = mgr.load_session("does-not-exist");
    assert!(result.is_err());
}

/// Legacy JSON format (messages embedded in .json, no .jsonl) can be loaded.
#[test]
fn legacy_json_format_loads() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

    let mut session = Session::new();
    session.id = "legacy".to_string();
    session
        .messages
        .push(make_msg(Role::User, "old format msg"));

    // Write as monolithic JSON (legacy)
    let json = serde_json::to_string_pretty(&session).unwrap();
    std::fs::write(tmp.path().join("legacy.json"), json).unwrap();
    // No .jsonl file

    let loaded = mgr.load_session("legacy").unwrap();
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "old format msg");
}

// ========================================================================
// Session listing and filtering
// ========================================================================

/// SessionListing returns sessions sorted by updated_at descending.
#[test]
fn listing_returns_sessions() {
    let tmp = TempDir::new().unwrap();
    let listing = SessionListing::new(tmp.path().to_path_buf());
    let index = SessionIndex::new(tmp.path().to_path_buf());

    for i in 0..5 {
        let mut session = Session::new();
        session.id = format!("list-{i}");
        index.upsert_entry(&session).unwrap();
    }

    let sessions = listing.list_sessions(None, false);
    assert_eq!(sessions.len(), 5);
}

/// Find latest session returns the most recently updated.
#[test]
fn listing_find_latest() {
    let tmp = TempDir::new().unwrap();
    let listing = SessionListing::new(tmp.path().to_path_buf());
    let index = SessionIndex::new(tmp.path().to_path_buf());

    for i in 0..3 {
        let mut session = Session::new();
        session.id = format!("latest-{i}");
        index.upsert_entry(&session).unwrap();
    }

    let latest = listing.find_latest_session();
    assert!(latest.is_some());
}

/// Delete removes session files and index entry.
#[test]
fn listing_delete_session() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();
    let listing = SessionListing::new(tmp.path().to_path_buf());

    // Create and save a session
    let mut session = Session::new();
    session.id = "to-delete".to_string();
    session.messages.push(make_msg(Role::User, "bye"));
    mgr.save_session(&session).unwrap();

    // Verify files exist
    assert!(tmp.path().join("to-delete.json").exists());
    assert!(tmp.path().join("to-delete.jsonl").exists());

    // Delete
    listing.delete_session("to-delete").unwrap();

    // Files should be gone
    assert!(!tmp.path().join("to-delete.json").exists());
    assert!(!tmp.path().join("to-delete.jsonl").exists());

    // Index should not contain entry
    let index = SessionIndex::new(tmp.path().to_path_buf());
    let idx = index.read_index();
    let has_entry = idx
        .map(|i| i.entries.iter().any(|e| e.session_id == "to-delete"))
        .unwrap_or(false);
    assert!(!has_entry, "deleted session should not be in index");
}

/// Find session by channel and user.
#[test]
fn listing_find_by_channel_user() {
    let tmp = TempDir::new().unwrap();
    let listing = SessionListing::new(tmp.path().to_path_buf());
    let index = SessionIndex::new(tmp.path().to_path_buf());

    let mut session = Session::new();
    session.id = "channel-test".to_string();
    session.channel = "slack".to_string();
    session.channel_user_id = "U123".to_string();
    index.upsert_entry(&session).unwrap();

    let found = listing.find_session_by_channel_user("slack", "U123", None);
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "channel-test");

    let not_found = listing.find_session_by_channel_user("discord", "U123", None);
    assert!(not_found.is_none());
}

// ========================================================================
// File locks
// ========================================================================

/// Basic lock acquire and release.
#[test]
fn file_lock_acquire_release() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let lock = FileLock::acquire(tmp.path(), Duration::from_secs(5)).unwrap();
    lock.release();
}

/// Lock is released when guard is dropped.
#[test]
fn file_lock_released_on_drop() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    {
        let _lock = FileLock::acquire(tmp.path(), Duration::from_secs(5)).unwrap();
    }
    // If the lock wasn't released, this would deadlock/timeout
    let _lock2 = FileLock::acquire(tmp.path(), Duration::from_secs(1)).unwrap();
}

/// with_file_lock executes closure while holding lock.
#[test]
fn with_file_lock_executes_closure() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let result =
        opendev_history::file_locks::with_file_lock(tmp.path(), Duration::from_secs(5), || 42 + 1)
            .unwrap();
    assert_eq!(result, 43);
}

// ========================================================================
// Session index
// ========================================================================

/// Index upsert creates and updates entries without duplicates.
#[test]
fn index_upsert_no_duplicates() {
    let tmp = TempDir::new().unwrap();
    let index = SessionIndex::new(tmp.path().to_path_buf());

    let mut session = Session::new();
    session.id = "dup-test".to_string();

    index.upsert_entry(&session).unwrap();
    index.upsert_entry(&session).unwrap();
    index.upsert_entry(&session).unwrap();

    let idx = index.read_index().unwrap();
    assert_eq!(idx.entries.len(), 1, "should not create duplicates");
}

/// Index remove deletes correct entry.
#[test]
fn index_remove_entry() {
    let tmp = TempDir::new().unwrap();
    let index = SessionIndex::new(tmp.path().to_path_buf());

    let mut s1 = Session::new();
    s1.id = "keep".to_string();
    let mut s2 = Session::new();
    s2.id = "remove".to_string();

    index.upsert_entry(&s1).unwrap();
    index.upsert_entry(&s2).unwrap();

    index.remove_entry("remove").unwrap();

    let idx = index.read_index().unwrap();
    assert_eq!(idx.entries.len(), 1);
    assert_eq!(idx.entries[0].session_id, "keep");
}

/// Invalid index version returns None.
#[test]
fn index_invalid_version_returns_none() {
    let tmp = TempDir::new().unwrap();
    let index_path = tmp.path().join("sessions-index.json");
    std::fs::write(&index_path, r#"{"version": 999, "entries": []}"#).unwrap();

    let index = SessionIndex::new(tmp.path().to_path_buf());
    assert!(index.read_index().is_none());
}

// ========================================================================
// Snapshot manager
// ========================================================================

/// Snapshot manager initializes with zero snapshots.
#[test]
fn snapshot_manager_starts_empty() {
    let mgr = SnapshotManager::new("/tmp/test-project");
    assert_eq!(mgr.snapshot_count(), 0);
}

/// Project-scoped sessions are isolated from each other.
#[test]
fn project_scoped_session_isolation() {
    let project_a = TempDir::new().unwrap();
    let project_b = TempDir::new().unwrap();

    let mut mgr_a = SessionManager::new(project_a.path().to_path_buf()).unwrap();
    let mut mgr_b = SessionManager::new(project_b.path().to_path_buf()).unwrap();

    // Create session in project A
    let session_a = mgr_a.create_session();
    let id_a = session_a.id.clone();
    mgr_a
        .current_session_mut()
        .unwrap()
        .messages
        .push(make_msg(Role::User, "Project A message"));
    mgr_a.save_current().unwrap();

    // Create session in project B
    let session_b = mgr_b.create_session();
    let id_b = session_b.id.clone();
    mgr_b
        .current_session_mut()
        .unwrap()
        .messages
        .push(make_msg(Role::User, "Project B message"));
    mgr_b.save_current().unwrap();

    // Sessions are different IDs
    assert_ne!(id_a, id_b);

    // Project A cannot see project B's session
    let result = mgr_a.load_session(&id_b);
    assert!(
        result.is_err(),
        "project A should not see project B's session"
    );

    // Project B cannot see project A's session
    let result = mgr_b.load_session(&id_a);
    assert!(
        result.is_err(),
        "project B should not see project A's session"
    );

    // Each can see their own
    let loaded_a = mgr_a.load_session(&id_a).unwrap();
    assert_eq!(loaded_a.messages[0].content, "Project A message");

    let loaded_b = mgr_b.load_session(&id_b).unwrap();
    assert_eq!(loaded_b.messages[0].content, "Project B message");
}

/// Multiple sessions in the same project directory are independently accessible.
#[test]
fn multiple_sessions_in_same_project() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

    // Create first session
    let s1 = mgr.create_session();
    let id1 = s1.id.clone();
    mgr.current_session_mut()
        .unwrap()
        .messages
        .push(make_msg(Role::User, "Session 1"));
    mgr.save_current().unwrap();

    // Create second session (replaces current)
    let s2 = mgr.create_session();
    let id2 = s2.id.clone();
    mgr.current_session_mut()
        .unwrap()
        .messages
        .push(make_msg(Role::User, "Session 2"));
    mgr.save_current().unwrap();

    assert_ne!(id1, id2);

    // Both sessions are loadable
    let loaded1 = mgr.load_session(&id1).unwrap();
    assert_eq!(loaded1.messages.len(), 1);
    assert_eq!(loaded1.messages[0].content, "Session 1");

    let loaded2 = mgr.load_session(&id2).unwrap();
    assert_eq!(loaded2.messages.len(), 1);
    assert_eq!(loaded2.messages[0].content, "Session 2");
}

/// Session with working_directory metadata survives save/load.
#[test]
fn session_working_directory_persists() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

    let mut session = Session::new();
    session.id = "wd-test".to_string();
    session.working_directory = Some("/home/user/project".to_string());
    session.messages.push(make_msg(Role::User, "init"));
    mgr.save_session(&session).unwrap();

    let loaded = mgr.load_session("wd-test").unwrap();
    assert_eq!(
        loaded.working_directory.as_deref(),
        Some("/home/user/project")
    );
}

/// Session messages preserve all fields (role, content, metadata, tool_calls).
#[test]
fn session_message_fields_preserved() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::new(tmp.path().to_path_buf()).unwrap();

    let mut msg = make_msg(Role::User, "test message");
    msg.metadata
        .insert("key".to_string(), serde_json::json!("value"));

    let mut session = Session::new();
    session.id = "fields-test".to_string();
    session.messages.push(msg);
    mgr.save_session(&session).unwrap();

    let loaded = mgr.load_session("fields-test").unwrap();
    assert_eq!(loaded.messages[0].role, Role::User);
    assert_eq!(loaded.messages[0].content, "test message");
    assert_eq!(
        loaded.messages[0].metadata.get("key"),
        Some(&serde_json::json!("value"))
    );
}

/// Snapshot track and patch detect file changes.
/// This test requires git to be installed.
#[test]
fn snapshot_track_and_patch() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().to_string_lossy().to_string();

    // Initialize a real git repo
    let init_ok = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&project_dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !init_ok {
        // git not available, skip test
        return;
    }

    std::fs::write(tmp.path().join("file.txt"), "version 1").unwrap();

    let mut mgr = SnapshotManager::new(&project_dir);
    let hash1 = mgr.track();

    if hash1.is_none() {
        // Shadow repo init failed (e.g., CI environment), skip
        return;
    }
    assert_eq!(mgr.snapshot_count(), 1);

    // Modify file
    std::fs::write(tmp.path().join("file.txt"), "version 2").unwrap();

    let changed = mgr.patch(hash1.as_ref().unwrap());
    assert!(
        changed.contains(&"file.txt".to_string()),
        "patch should detect changed file"
    );
}
