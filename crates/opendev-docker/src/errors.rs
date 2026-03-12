//! Error types for the Docker crate.

use thiserror::Error;

/// Result alias for Docker operations.
pub type Result<T> = std::result::Result<T, DockerError>;

/// Errors that can occur during Docker operations.
#[derive(Debug, Error)]
pub enum DockerError {
    #[error("Docker image pull failed for '{image}': {reason}")]
    ImagePullFailed { image: String, reason: String },

    #[error("Docker command failed: {message}\nstderr: {stderr}")]
    CommandFailed { message: String, stderr: String },

    #[error("Operation timed out after {seconds}s: {operation}")]
    Timeout { seconds: f64, operation: String },

    #[error("Container not started — call start() first")]
    NotStarted,

    #[error("Session '{0}' already exists")]
    SessionExists(String),

    #[error("Session '{0}' not found")]
    SessionNotFound(String),

    #[error("Command exited with code {exit_code}: {command}\n{output}")]
    NonZeroExit {
        exit_code: i32,
        command: String,
        output: String,
    },

    #[error("Connection error to {host}:{port} — {reason}")]
    ConnectionFailed {
        host: String,
        port: u16,
        reason: String,
    },

    #[error("{0}")]
    Other(String),
}
