//! Session management routes.

use std::path::Path;

use axum::extract::{Path as AxumPath, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::error::WebError;
use crate::state::AppState;

/// Create session request.
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub working_directory: Option<String>,
}

/// Verify path request.
#[derive(Debug, Deserialize)]
pub struct VerifyPathRequest {
    #[serde(default)]
    pub path: Option<String>,
}

/// Browse directory request.
#[derive(Debug, Deserialize)]
pub struct BrowseDirectoryRequest {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub show_hidden: bool,
}

/// Session model update request.
#[derive(Debug, Deserialize)]
pub struct SessionModelUpdate {
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub model_thinking_provider: Option<String>,
    pub model_thinking: Option<String>,
    pub model_vlm_provider: Option<String>,
    pub model_vlm: Option<String>,
    pub model_critique_provider: Option<String>,
    pub model_critique: Option<String>,
    pub model_compact_provider: Option<String>,
    pub model_compact: Option<String>,
}

/// Query parameters for file listing.
#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    #[serde(default)]
    pub query: String,
}

/// Build the sessions router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/bridge-info", get(get_bridge_info))
        .route("/api/sessions/files", get(list_files))
        .route("/api/sessions/verify-path", post(verify_path))
        .route("/api/sessions/browse-directory", post(browse_directory))
        .route(
            "/api/sessions/{id}",
            get(get_session).delete(delete_session),
        )
        .route("/api/sessions/{id}/resume", post(resume_session))
        .route("/api/sessions/{id}/messages", get(get_session_messages))
        .route(
            "/api/sessions/{id}/model",
            get(get_session_model)
                .put(update_session_model)
                .delete(clear_session_model),
        )
}

/// List all sessions.
async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, WebError> {
    let mgr = state.session_manager().await;
    let index = mgr.index().read_index();

    let sessions: Vec<serde_json::Value> = match index {
        Some(idx) => idx
            .entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "id": entry.session_id,
                    "created_at": entry.created,
                    "updated_at": entry.modified,
                    "message_count": entry.message_count,
                    "title": entry.title,
                    "working_directory": entry.working_directory,
                })
            })
            .collect(),
        None => Vec::new(),
    };

    Ok(Json(serde_json::json!(sessions)))
}

/// Create a new session.
///
/// Before creating a brand-new session, checks if there is an existing empty
/// session (message_count == 0) for the same workspace. If found and not stale,
/// the existing session is reused instead of creating a new one.
async fn create_session(
    State(state): State<AppState>,
    Json(payload): Json<CreateSessionRequest>,
) -> Result<Json<serde_json::Value>, WebError> {
    let requested_wd = payload.working_directory.clone();

    let mut mgr = state.session_manager_mut().await;

    // Try to reuse an existing empty session for the same workspace.
    if let Some(ref wd) = requested_wd {
        if let Some(index) = mgr.index().read_index() {
            let empty_match = index.entries.iter().find(|entry| {
                entry.message_count == 0
                    && entry
                        .working_directory
                        .as_deref()
                        .map(|d| d == wd.as_str())
                        .unwrap_or(false)
            });

            if let Some(entry) = empty_match {
                let candidate_id = entry.session_id.clone();

                // Guard against stale index: if the candidate is already the
                // current session with in-memory messages, skip reuse.
                let is_stale = mgr
                    .current_session()
                    .map(|s| s.id == candidate_id && !s.messages.is_empty())
                    .unwrap_or(false);

                if !is_stale {
                    // Try to load and resume the candidate session.
                    if mgr.resume_session(&candidate_id).is_ok() {
                        return Ok(Json(serde_json::json!({
                            "id": candidate_id,
                            "status": "reused",
                            "message": "Reusing existing empty session",
                        })));
                    }
                    // If load fails (e.g. file deleted), fall through to create new.
                }
            }
        }
    }

    // No reusable session found — create a new one.
    let session = mgr.create_session();
    let session_id = session.id.clone();

    // Set working directory if provided.
    if let Some(wd) = requested_wd {
        if let Some(session) = mgr.current_session_mut() {
            session.working_directory = Some(wd);
        }
    }

    // Save the new session.
    mgr.save_current().map_err(|e| {
        WebError::Internal(format!("Failed to save session: {}", e))
    })?;

    Ok(Json(serde_json::json!({
        "id": session_id,
        "status": "created",
    })))
}

/// Get a specific session.
async fn get_session(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, WebError> {
    let mgr = state.session_manager().await;
    let session = mgr.load_session(&id).map_err(|e| {
        WebError::NotFound(format!("Session {} not found: {}", id, e))
    })?;

    Ok(Json(serde_json::to_value(&session.get_metadata()).map_err(
        |e| WebError::Internal(format!("Failed to serialize session: {}", e)),
    )?))
}

/// Delete a specific session.
async fn delete_session(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, WebError> {
    let mut mgr = state.session_manager_mut().await;

    // Check session exists.
    mgr.load_session(&id).map_err(|e| {
        WebError::NotFound(format!("Session {} not found: {}", id, e))
    })?;

    // Delete session files (.json, .jsonl).
    let session_dir = mgr.session_dir().to_path_buf();
    let json_path = session_dir.join(format!("{}.json", id));
    let jsonl_path = session_dir.join(format!("{}.jsonl", id));
    let debug_path = session_dir.join(format!("{}.debug", id));

    if json_path.exists() {
        std::fs::remove_file(&json_path).map_err(|e| {
            WebError::Internal(format!("Failed to delete session file: {}", e))
        })?;
    }
    if jsonl_path.exists() {
        std::fs::remove_file(&jsonl_path).map_err(|e| {
            WebError::Internal(format!("Failed to delete session transcript: {}", e))
        })?;
    }
    if debug_path.exists() {
        let _ = std::fs::remove_file(&debug_path);
    }

    // Remove from index.
    mgr.index().remove_entry(&id).map_err(|e| {
        WebError::Internal(format!("Failed to update index: {}", e))
    })?;

    // Clear current session if it was the deleted one.
    if mgr
        .current_session()
        .map(|s| s.id == id)
        .unwrap_or(false)
    {
        mgr.set_current_session(opendev_models::Session::new());
    }

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": format!("Session {} deleted", id),
    })))
}

/// Resume a session.
async fn resume_session(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, WebError> {
    let mut mgr = state.session_manager_mut().await;
    mgr.resume_session(&id).map_err(|e| {
        WebError::NotFound(format!("Session {} not found: {}", id, e))
    })?;

    Ok(Json(serde_json::json!({
        "status": "resumed",
        "session_id": id,
    })))
}

/// Get messages for a session.
async fn get_session_messages(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, WebError> {
    let mgr = state.session_manager().await;
    let session = mgr.load_session(&id).map_err(|e| {
        WebError::NotFound(format!("Session {} not found: {}", id, e))
    })?;

    let messages: Vec<serde_json::Value> = session
        .messages
        .iter()
        .map(|msg| {
            serde_json::json!({
                "role": msg.role,
                "content": msg.content,
                "timestamp": msg.timestamp,
                "tool_calls": msg.tool_calls.len(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!(messages)))
}

/// Get bridge mode status.
async fn get_bridge_info(
    State(_state): State<AppState>,
) -> Json<serde_json::Value> {
    // Bridge mode is not yet implemented in the Rust port.
    // Return a default non-bridge response.
    Json(serde_json::json!({
        "bridge_mode": false,
        "session_id": null,
    }))
}

/// List files in the current session's working directory.
async fn list_files(
    State(state): State<AppState>,
    Query(params): Query<ListFilesQuery>,
) -> Result<Json<serde_json::Value>, WebError> {
    let mgr = state.session_manager().await;
    let session = mgr.current_session();

    let working_dir = match session.and_then(|s| s.working_directory.as_deref()) {
        Some(wd) => wd.to_string(),
        None => {
            return Ok(Json(serde_json::json!({"files": []})));
        }
    };

    let wd_path = Path::new(&working_dir);
    if !wd_path.exists() || !wd_path.is_dir() {
        return Ok(Json(serde_json::json!({"files": []})));
    }

    // Directories to always exclude.
    let always_exclude: &[&str] = &[
        ".git",
        ".hg",
        ".svn",
        "node_modules",
        "__pycache__",
        ".pytest_cache",
        ".mypy_cache",
        ".venv",
        "venv",
        ".DS_Store",
        ".idea",
        ".vscode",
        "target",
        "dist",
        "build",
        "out",
        ".next",
        ".nuxt",
        ".cache",
        ".tox",
        ".nox",
        ".gradle",
        "coverage",
        "htmlcov",
    ];

    let query = params.query.to_lowercase();
    let mut files: Vec<serde_json::Value> = Vec::new();
    let max_files = 100;

    // Walk directory tree (iterative BFS).
    let mut stack = vec![wd_path.to_path_buf()];
    'outer: while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                if !always_exclude.contains(&name.as_ref()) {
                    stack.push(entry.path());
                }
                continue;
            }

            if file_type.is_file() {
                let rel_path = match entry.path().strip_prefix(wd_path) {
                    Ok(p) => p.to_string_lossy().to_string(),
                    Err(_) => continue,
                };

                // Filter by query if provided.
                if !query.is_empty() && !rel_path.to_lowercase().contains(&query) {
                    continue;
                }

                files.push(serde_json::json!({
                    "path": rel_path,
                    "name": name,
                    "is_file": true,
                }));

                if files.len() >= max_files {
                    break 'outer;
                }
            }
        }
    }

    // Sort by path.
    files.sort_by(|a, b| {
        let pa = a["path"].as_str().unwrap_or("");
        let pb = b["path"].as_str().unwrap_or("");
        pa.cmp(pb)
    });

    Ok(Json(serde_json::json!({"files": files})))
}

/// Verify if a directory path exists and is accessible.
async fn verify_path(
    State(_state): State<AppState>,
    Json(payload): Json<VerifyPathRequest>,
) -> Json<serde_json::Value> {
    let path_str = payload
        .path
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();

    if path_str.is_empty() {
        return Json(serde_json::json!({
            "exists": false,
            "is_directory": false,
            "error": "Path cannot be empty",
        }));
    }

    // Expand ~ to home directory.
    let expanded = if path_str.starts_with('~') {
        if let Some(home) = dirs_path_home() {
            path_str.replacen('~', &home, 1)
        } else {
            path_str.clone()
        }
    } else {
        path_str.clone()
    };

    let path = Path::new(&expanded);

    if !path.exists() {
        return Json(serde_json::json!({
            "exists": false,
            "is_directory": false,
            "error": "Path does not exist",
        }));
    }

    if !path.is_dir() {
        return Json(serde_json::json!({
            "exists": true,
            "is_directory": false,
            "error": "Path is not a directory",
        }));
    }

    // Check read access by trying to read_dir.
    if std::fs::read_dir(path).is_err() {
        return Json(serde_json::json!({
            "exists": true,
            "is_directory": true,
            "error": "No read access to directory",
        }));
    }

    let canonical = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf());

    Json(serde_json::json!({
        "exists": true,
        "is_directory": true,
        "path": canonical.to_string_lossy(),
        "error": null,
    }))
}

/// Browse directories at a given path for the workspace picker.
async fn browse_directory(
    State(_state): State<AppState>,
    Json(payload): Json<BrowseDirectoryRequest>,
) -> Json<serde_json::Value> {
    let raw = payload.path.trim().to_string();

    let target = if raw.is_empty() {
        // Default to home directory.
        match dirs_path_home() {
            Some(home) => std::path::PathBuf::from(home),
            None => std::path::PathBuf::from("/"),
        }
    } else {
        let expanded = if raw.starts_with('~') {
            if let Some(home) = dirs_path_home() {
                raw.replacen('~', &home, 1)
            } else {
                raw.clone()
            }
        } else {
            raw.clone()
        };
        std::path::PathBuf::from(expanded)
    };

    let target = target
        .canonicalize()
        .unwrap_or_else(|_| target.clone());

    if !target.exists() {
        return Json(serde_json::json!({
            "current_path": target.to_string_lossy(),
            "parent_path": target.parent().map(|p| p.to_string_lossy().to_string()),
            "directories": [],
            "error": "Path does not exist",
        }));
    }

    if !target.is_dir() {
        return Json(serde_json::json!({
            "current_path": target.to_string_lossy(),
            "parent_path": target.parent().map(|p| p.to_string_lossy().to_string()),
            "directories": [],
            "error": "Path is not a directory",
        }));
    }

    let parent_path = if target.parent() != Some(&target) {
        target.parent().map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };

    let entries = match std::fs::read_dir(&target) {
        Ok(e) => e,
        Err(_) => {
            return Json(serde_json::json!({
                "current_path": target.to_string_lossy(),
                "parent_path": parent_path,
                "directories": [],
                "error": "Permission denied reading directory contents",
            }));
        }
    };

    let mut dirs: Vec<serde_json::Value> = Vec::new();
    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && !payload.show_hidden {
            continue;
        }
        // Check read access.
        if std::fs::read_dir(entry.path()).is_err() {
            continue;
        }
        dirs.push(serde_json::json!({
            "name": name,
            "path": entry.path().to_string_lossy(),
        }));
    }

    dirs.sort_by(|a, b| {
        let na = a["name"].as_str().unwrap_or("").to_lowercase();
        let nb = b["name"].as_str().unwrap_or("").to_lowercase();
        na.cmp(&nb)
    });

    Json(serde_json::json!({
        "current_path": target.to_string_lossy(),
        "parent_path": parent_path,
        "directories": dirs,
        "error": null,
    }))
}

/// Get session model overlay.
async fn get_session_model(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, WebError> {
    let mgr = state.session_manager().await;
    let session = mgr.load_session(&id).map_err(|e| {
        WebError::NotFound(format!("Session {} not found: {}", id, e))
    })?;

    let overlay = session
        .metadata
        .get("session_model")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    Ok(Json(overlay))
}

/// Update session model overlay.
async fn update_session_model(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<SessionModelUpdate>,
) -> Result<Json<serde_json::Value>, WebError> {
    let mgr = state.session_manager().await;

    let mut session = mgr.load_session(&id).map_err(|e| {
        WebError::NotFound(format!("Session {} not found: {}", id, e))
    })?;

    // Build overlay from non-None fields.
    let mut overlay = serde_json::Map::new();
    if let Some(v) = body.model_provider {
        overlay.insert("model_provider".to_string(), serde_json::json!(v));
    }
    if let Some(v) = body.model {
        overlay.insert("model".to_string(), serde_json::json!(v));
    }
    if let Some(v) = body.model_thinking_provider {
        overlay.insert("model_thinking_provider".to_string(), serde_json::json!(v));
    }
    if let Some(v) = body.model_thinking {
        overlay.insert("model_thinking".to_string(), serde_json::json!(v));
    }
    if let Some(v) = body.model_vlm_provider {
        overlay.insert("model_vlm_provider".to_string(), serde_json::json!(v));
    }
    if let Some(v) = body.model_vlm {
        overlay.insert("model_vlm".to_string(), serde_json::json!(v));
    }
    if let Some(v) = body.model_critique_provider {
        overlay.insert("model_critique_provider".to_string(), serde_json::json!(v));
    }
    if let Some(v) = body.model_critique {
        overlay.insert("model_critique".to_string(), serde_json::json!(v));
    }
    if let Some(v) = body.model_compact_provider {
        overlay.insert("model_compact_provider".to_string(), serde_json::json!(v));
    }
    if let Some(v) = body.model_compact {
        overlay.insert("model_compact".to_string(), serde_json::json!(v));
    }

    if overlay.is_empty() {
        return Err(WebError::BadRequest(
            "No model fields provided".to_string(),
        ));
    }

    // Store overlay in session metadata.
    session.metadata.insert(
        "session_model".to_string(),
        serde_json::Value::Object(overlay),
    );

    mgr.save_session(&session).map_err(|e| {
        WebError::Internal(format!("Failed to save session: {}", e))
    })?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Session model updated",
    })))
}

/// Clear session model overlay.
async fn clear_session_model(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, WebError> {
    let mgr = state.session_manager().await;

    let mut session = mgr.load_session(&id).map_err(|e| {
        WebError::NotFound(format!("Session {} not found: {}", id, e))
    })?;

    session.metadata.remove("session_model");

    mgr.save_session(&session).map_err(|e| {
        WebError::Internal(format!("Failed to save session: {}", e))
    })?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Session model cleared",
    })))
}

/// Helper: get the home directory path as a String.
fn dirs_path_home() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use opendev_config::ModelRegistry;
    use opendev_history::SessionManager;
    use opendev_http::UserStore;
    use opendev_models::AppConfig;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn make_state_with_dir(tmp: &std::path::Path) -> AppState {
        let session_manager = SessionManager::new(tmp.to_path_buf()).unwrap();
        let config = AppConfig::default();
        let user_store = UserStore::new(tmp.to_path_buf()).unwrap();
        let model_registry = ModelRegistry::new();
        AppState::new(session_manager, config, "/tmp/test".to_string(), user_store, model_registry)
    }

    #[tokio::test]
    async fn test_session_reuse_empty_session() {
        let tmp = TempDir::new().unwrap();
        let state = make_state_with_dir(tmp.path());

        // Create an empty session with a workspace.
        {
            let mut mgr = state.session_manager_mut().await;
            let _session = mgr.create_session();
            mgr.current_session_mut().unwrap().working_directory =
                Some("/workspace/project".to_string());
            mgr.save_current().unwrap();
            // Clear current so it doesn't interfere with the stale check.
            drop(mgr);
        }

        // Get the first session ID from the index.
        let first_session_id = {
            let mgr = state.session_manager().await;
            let index = mgr.index().read_index().unwrap();
            assert_eq!(index.entries.len(), 1);
            index.entries[0].session_id.clone()
        };

        // Now POST /api/sessions with the same workspace.
        let app = crate::server::build_app(state.clone(), None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"working_directory":"/workspace/project"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "reused");
        assert_eq!(json["id"], first_session_id);
    }

    #[tokio::test]
    async fn test_session_create_new_when_no_empty_match() {
        let tmp = TempDir::new().unwrap();
        let state = make_state_with_dir(tmp.path());

        let app = crate::server::build_app(state, None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"working_directory":"/workspace/new-project"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "created");
    }

    #[tokio::test]
    async fn test_delete_session() {
        let tmp = TempDir::new().unwrap();
        let state = make_state_with_dir(tmp.path());

        // Create a session.
        let session_id = {
            let mut mgr = state.session_manager_mut().await;
            let session = mgr.create_session();
            let id = session.id.clone();
            mgr.save_current().unwrap();
            id
        };

        let app = crate::server::build_app(state.clone(), None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/sessions/{}", session_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "success");
    }

    #[tokio::test]
    async fn test_delete_session_not_found() {
        let tmp = TempDir::new().unwrap();
        let state = make_state_with_dir(tmp.path());

        let app = crate::server::build_app(state, None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/sessions/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_bridge_info() {
        let tmp = TempDir::new().unwrap();
        let state = make_state_with_dir(tmp.path());

        let app = crate::server::build_app(state, None);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sessions/bridge-info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["bridge_mode"], false);
    }

    #[tokio::test]
    async fn test_verify_path_empty() {
        let tmp = TempDir::new().unwrap();
        let state = make_state_with_dir(tmp.path());

        let app = crate::server::build_app(state, None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sessions/verify-path")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"path":""}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["exists"], false);
    }

    #[tokio::test]
    async fn test_verify_path_valid_dir() {
        let tmp = TempDir::new().unwrap();
        let state = make_state_with_dir(tmp.path());

        let app = crate::server::build_app(state, None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sessions/verify-path")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"path":"{}"}}"#,
                        tmp.path().to_string_lossy()
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["exists"], true);
        assert_eq!(json["is_directory"], true);
    }

    #[tokio::test]
    async fn test_browse_directory() {
        let tmp = TempDir::new().unwrap();
        // Create a subdirectory.
        std::fs::create_dir(tmp.path().join("subdir")).unwrap();

        let state = make_state_with_dir(tmp.path());

        let app = crate::server::build_app(state, None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/sessions/browse-directory")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"path":"{}","show_hidden":false}}"#,
                        tmp.path().to_string_lossy()
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].is_null());
        let dirs = json["directories"].as_array().unwrap();
        assert!(dirs.iter().any(|d| d["name"] == "subdir"));
    }

    #[tokio::test]
    async fn test_session_model_lifecycle() {
        let tmp = TempDir::new().unwrap();
        let state = make_state_with_dir(tmp.path());

        // Create a session.
        let session_id = {
            let mut mgr = state.session_manager_mut().await;
            let session = mgr.create_session();
            let id = session.id.clone();
            mgr.save_current().unwrap();
            id
        };

        // GET model — should be empty.
        let app = crate::server::build_app(state.clone(), None);
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/sessions/{}/model", session_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // PUT model.
        let app = crate::server::build_app(state.clone(), None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/sessions/{}/model", session_id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"model_provider":"openai","model":"gpt-4"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // GET model — should have values.
        let app = crate::server::build_app(state.clone(), None);
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/sessions/{}/model", session_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["model_provider"], "openai");

        // DELETE model.
        let app = crate::server::build_app(state.clone(), None);
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/sessions/{}/model", session_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_files_no_session() {
        let tmp = TempDir::new().unwrap();
        let state = make_state_with_dir(tmp.path());

        let app = crate::server::build_app(state, None);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sessions/files")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["files"].as_array().unwrap().len(), 0);
    }
}
