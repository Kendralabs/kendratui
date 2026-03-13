//! Local Docker runtime — manages bash sessions and file operations
//! inside a container by shelling out to `docker exec`.
//!
//! Ports the Python `LocalRuntime` class.

use std::collections::HashMap;
use std::path::Path;

use tracing::info;

use crate::errors::{DockerError, Result};
use crate::models::{
    BashAction, BashObservation, CheckMode, CloseSessionRequest, CloseSessionResponse,
    CreateSessionRequest, CreateSessionResponse, IsAliveResponse, ReadFileRequest,
    ReadFileResponse, WriteFileRequest, WriteFileResponse,
};
use crate::session::DockerSession;

/// Local runtime that manages bash sessions and file operations.
///
/// Each session is backed by a persistent `docker exec` process.
pub struct LocalRuntime {
    container_id: String,
    sessions: HashMap<String, DockerSession>,
    closed: bool,
}

impl LocalRuntime {
    /// Create a new local runtime attached to the given container.
    pub fn new(container_id: impl Into<String>) -> Self {
        Self {
            container_id: container_id.into(),
            sessions: HashMap::new(),
            closed: false,
        }
    }

    /// Health check.
    pub fn is_alive(&self) -> IsAliveResponse {
        if self.closed {
            return IsAliveResponse {
                status: "error".into(),
                message: "Runtime is closed".into(),
            };
        }
        IsAliveResponse::default()
    }

    /// Create a new bash session.
    pub async fn create_session(
        &mut self,
        request: &CreateSessionRequest,
    ) -> Result<CreateSessionResponse> {
        if self.sessions.contains_key(&request.session) {
            return Err(DockerError::SessionExists(request.session.clone()));
        }

        let session = DockerSession::new(&self.container_id, &request.session);

        self.sessions.insert(request.session.clone(), session);
        info!("Created session '{}'", request.session);

        Ok(CreateSessionResponse {
            success: true,
            session: request.session.clone(),
            message: String::new(),
        })
    }

    /// Run a command in an existing session.
    pub async fn run_in_session(&mut self, action: &BashAction) -> Result<BashObservation> {
        if !self.sessions.contains_key(&action.session) {
            if action.session == "default" {
                self.create_session(&CreateSessionRequest::default())
                    .await?;
            } else {
                return Err(DockerError::SessionNotFound(action.session.clone()));
            }
        }

        let session = self.sessions.get(&action.session).unwrap();
        session
            .exec_command(&action.command, action.timeout, action.check)
            .await
    }

    /// Close a session.
    pub async fn close_session(&mut self, request: &CloseSessionRequest) -> CloseSessionResponse {
        if let Some(_session) = self.sessions.remove(&request.session) {
            info!("Closed session '{}'", request.session);
            CloseSessionResponse {
                success: true,
                message: String::new(),
            }
        } else {
            CloseSessionResponse {
                success: false,
                message: format!("Session '{}' not found", request.session),
            }
        }
    }

    /// Read a file from inside the container via `docker exec cat`.
    pub async fn read_file(&self, request: &ReadFileRequest) -> Result<ReadFileResponse> {
        let session = self.get_or_default_session()?;
        let obs = session
            .exec_command(&format!("cat '{}'", request.path), 30.0, CheckMode::Silent)
            .await?;

        if obs.exit_code == Some(0) || obs.exit_code.is_none() {
            Ok(ReadFileResponse {
                success: true,
                content: obs.output,
                error: None,
            })
        } else {
            Ok(ReadFileResponse {
                success: false,
                content: String::new(),
                error: Some(obs.output),
            })
        }
    }

    /// Write a file inside the container via `docker exec`.
    pub async fn write_file(&self, request: &WriteFileRequest) -> Result<WriteFileResponse> {
        let session = self.get_or_default_session()?;
        // Create parent dirs then write via heredoc
        let parent = Path::new(&request.path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into());

        let escaped_content = request.content.replace('\'', "'\\''");
        let cmd = format!(
            "mkdir -p '{}' && printf '%s' '{}' > '{}'",
            parent, escaped_content, request.path
        );

        let obs = session.exec_command(&cmd, 30.0, CheckMode::Silent).await?;

        if obs.exit_code == Some(0) || obs.exit_code.is_none() {
            Ok(WriteFileResponse {
                success: true,
                error: None,
            })
        } else {
            Ok(WriteFileResponse {
                success: false,
                error: Some(obs.output),
            })
        }
    }

    /// Close all sessions and mark runtime closed.
    pub async fn close(&mut self) {
        let names: Vec<String> = self.sessions.keys().cloned().collect();
        for name in names {
            self.close_session(&CloseSessionRequest { session: name })
                .await;
        }
        self.closed = true;
        info!("Runtime closed");
    }

    /// Get the container ID.
    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    /// Helper: get any existing session or return an error.
    fn get_or_default_session(&self) -> Result<&DockerSession> {
        self.sessions
            .get("default")
            .or_else(|| self.sessions.values().next())
            .ok_or_else(|| DockerError::SessionNotFound("default".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_runtime_new() {
        let rt = LocalRuntime::new("abc123");
        assert_eq!(rt.container_id(), "abc123");
        assert!(!rt.closed);
    }

    #[test]
    fn test_is_alive() {
        let rt = LocalRuntime::new("abc123");
        let resp = rt.is_alive();
        assert_eq!(resp.status, "ok");
    }

    #[test]
    fn test_is_alive_when_closed() {
        let mut rt = LocalRuntime::new("abc123");
        rt.closed = true;
        let resp = rt.is_alive();
        assert_eq!(resp.status, "error");
    }

    #[tokio::test]
    async fn test_close_session_not_found() {
        let mut rt = LocalRuntime::new("abc123");
        let resp = rt
            .close_session(&CloseSessionRequest {
                session: "nonexistent".into(),
            })
            .await;
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn test_create_session_duplicate() {
        let mut rt = LocalRuntime::new("abc123");
        // Manually insert a session
        rt.sessions
            .insert("default".into(), DockerSession::new("abc123", "default"));
        let err = rt
            .create_session(&CreateSessionRequest::default())
            .await
            .unwrap_err();
        assert!(matches!(err, DockerError::SessionExists(_)));
    }

    #[tokio::test]
    async fn test_close_runtime() {
        let mut rt = LocalRuntime::new("abc123");
        rt.close().await;
        assert!(rt.closed);
        let resp = rt.is_alive();
        assert_eq!(resp.status, "error");
    }
}
