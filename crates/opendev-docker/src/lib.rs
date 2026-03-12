//! Docker runtime system for opendev.
//!
//! This crate provides container lifecycle management, command execution,
//! file operations, and tool handling inside Docker containers. It mirrors
//! the Python `opendev.core.docker` package, using `tokio::process::Command`
//! to shell out to the Docker CLI.

pub mod deployment;
pub mod errors;
pub mod local_runtime;
pub mod models;
pub mod remote_runtime;
pub mod session;
pub mod tool_handler;

// Re-exports for convenience.
pub use deployment::DockerDeployment;
pub use errors::{DockerError, Result};
pub use local_runtime::LocalRuntime;
pub use models::{
    BashAction, BashObservation, CheckMode, ContainerSpec, ContainerStatus, DockerConfig,
    PortMapping, RuntimeType, ToolResult, VolumeMount,
};
pub use remote_runtime::RemoteRuntime;
pub use session::DockerSession;
pub use tool_handler::DockerToolHandler;
