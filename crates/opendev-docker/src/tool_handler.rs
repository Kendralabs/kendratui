//! Docker tool handler — routes tool execution (bash, file operations)
//! into a Docker container.
//!
//! Ports the Python `DockerToolHandler` and `DockerToolRegistry`.

use std::path::Path;

use crate::models::{CheckMode, ToolResult};
use crate::session::DockerSession;

/// Common error patterns that indicate a command failure even if exit code is 0.
const ERROR_PATTERNS: &[&str] = &[
    "Error:",
    "error:",
    "ERROR:",
    "ModuleNotFoundError",
    "ImportError",
    "No such file or directory",
    "SyntaxError",
    "TypeError",
    "ValueError",
    "Traceback (most recent call last)",
    "FileNotFoundError",
    "NameError",
    "AttributeError",
];

/// Handle tool execution inside Docker containers.
pub struct DockerToolHandler {
    session: DockerSession,
    workspace_dir: String,
    shell_init: String,
}

impl DockerToolHandler {
    /// Create a new tool handler.
    pub fn new(
        session: DockerSession,
        workspace_dir: impl Into<String>,
        shell_init: impl Into<String>,
    ) -> Self {
        Self {
            session,
            workspace_dir: workspace_dir.into(),
            shell_init: shell_init.into(),
        }
    }

    /// Reference to the underlying session.
    pub fn session(&self) -> &DockerSession {
        &self.session
    }

    /// Mutable reference to the underlying session.
    pub fn session_mut(&mut self) -> &mut DockerSession {
        &mut self.session
    }

    /// Execute a bash command inside the container.
    pub async fn run_command(
        &self,
        command: &str,
        timeout: f64,
        working_dir: Option<&str>,
    ) -> ToolResult {
        if command.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("command is required".into()),
                exit_code: None,
            };
        }

        let mut full_command = String::new();

        // Prepend working directory cd
        if let Some(wd) = working_dir {
            let container_path = self.translate_path(wd);
            full_command.push_str(&format!("cd {} && ", container_path));
        }

        // Prepend shell init
        if !self.shell_init.is_empty() {
            full_command.push_str(&format!("{} && ", self.shell_init));
        }

        full_command.push_str(command);

        match self
            .session
            .exec_command(&full_command, timeout, CheckMode::Silent)
            .await
        {
            Ok(obs) => {
                let success = obs.exit_code == Some(0) || obs.exit_code.is_none();
                ToolResult {
                    success,
                    output: Some(obs.output),
                    error: obs.failure_reason,
                    exit_code: obs.exit_code,
                }
            }
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                exit_code: None,
            },
        }
    }

    /// Read a file from the container.
    pub async fn read_file(&self, path: &str) -> ToolResult {
        if path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("path is required".into()),
                exit_code: None,
            };
        }

        let container_path = self.translate_path(path);
        let cmd = format!("cat '{}'", container_path);

        match self
            .session
            .exec_command(&cmd, 30.0, CheckMode::Silent)
            .await
        {
            Ok(obs) if obs.exit_code == Some(0) || obs.exit_code.is_none() => ToolResult {
                success: true,
                output: Some(obs.output),
                error: None,
                exit_code: obs.exit_code,
            },
            Ok(obs) => ToolResult {
                success: false,
                output: None,
                error: Some(obs.output),
                exit_code: obs.exit_code,
            },
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                exit_code: None,
            },
        }
    }

    /// Write a file inside the container.
    pub async fn write_file(&self, path: &str, content: &str) -> ToolResult {
        if path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("path is required".into()),
                exit_code: None,
            };
        }

        let container_path = self.translate_path(path);
        let parent = Path::new(&container_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into());

        let escaped = content.replace('\'', "'\\''");
        let cmd = format!(
            "mkdir -p '{}' && printf '%s' '{}' > '{}'",
            parent, escaped, container_path
        );

        match self
            .session
            .exec_command(&cmd, 30.0, CheckMode::Silent)
            .await
        {
            Ok(obs) if obs.exit_code == Some(0) || obs.exit_code.is_none() => ToolResult {
                success: true,
                output: Some(format!(
                    "Wrote {} bytes to {}",
                    content.len(),
                    container_path
                )),
                error: None,
                exit_code: obs.exit_code,
            },
            Ok(obs) => ToolResult {
                success: false,
                output: None,
                error: Some(obs.output),
                exit_code: obs.exit_code,
            },
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                exit_code: None,
            },
        }
    }

    /// List files in a directory inside the container.
    pub async fn list_files(
        &self,
        path: &str,
        pattern: Option<&str>,
        recursive: bool,
    ) -> ToolResult {
        let container_path = self.translate_path(if path.is_empty() { "." } else { path });
        let pat = pattern.unwrap_or("*");

        let cmd = if recursive {
            format!(
                "find {} -name '{}' -type f 2>/dev/null | head -100",
                container_path, pat
            )
        } else {
            format!("ls -la {} 2>/dev/null", container_path)
        };

        match self
            .session
            .exec_command(&cmd, 30.0, CheckMode::Silent)
            .await
        {
            Ok(obs) => ToolResult {
                success: obs.exit_code == Some(0) || obs.exit_code.is_none(),
                output: Some(if obs.output.is_empty() {
                    "(empty directory)".into()
                } else {
                    obs.output
                }),
                error: obs.failure_reason,
                exit_code: obs.exit_code,
            },
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                exit_code: None,
            },
        }
    }

    /// Search for text in files inside the container.
    pub async fn search(&self, query: &str, path: Option<&str>) -> ToolResult {
        if query.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("query is required".into()),
                exit_code: None,
            };
        }

        let container_path = self.translate_path(path.unwrap_or("."));
        let cmd = format!(
            "grep -rn '{}' {} 2>/dev/null | head -50",
            query, container_path
        );

        match self
            .session
            .exec_command(&cmd, 60.0, CheckMode::Silent)
            .await
        {
            Ok(obs) => ToolResult {
                success: true,
                output: Some(if obs.output.is_empty() {
                    "No matches found".into()
                } else {
                    obs.output
                }),
                error: None,
                exit_code: obs.exit_code,
            },
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                exit_code: None,
            },
        }
    }

    /// Translate a host path to a container path.
    pub fn translate_path(&self, path: &str) -> String {
        if path.is_empty() {
            return self.workspace_dir.clone();
        }

        // Already a container path
        if path.starts_with("/testbed") || path.starts_with("/workspace") {
            return path.to_string();
        }

        // Relative path
        if !path.starts_with('/') {
            let clean = path.trim_start_matches("./");
            return format!("{}/{}", self.workspace_dir, clean);
        }

        // Absolute host path — extract filename
        if let Some(name) = Path::new(path).file_name() {
            return format!("{}/{}", self.workspace_dir, name.to_string_lossy());
        }

        format!("{}/{}", self.workspace_dir, path)
    }

    /// Check if command output indicates an error.
    pub fn check_command_has_error(exit_code: i32, output: &str) -> bool {
        if exit_code != 0 {
            return true;
        }
        ERROR_PATTERNS.iter().any(|p| output.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_handler() -> DockerToolHandler {
        let session = DockerSession::new("test123", "default");
        DockerToolHandler::new(session, "/workspace", "")
    }

    #[test]
    fn test_translate_path_empty() {
        let h = make_handler();
        assert_eq!(h.translate_path(""), "/workspace");
    }

    #[test]
    fn test_translate_path_container_path() {
        let h = make_handler();
        assert_eq!(h.translate_path("/testbed/repo"), "/testbed/repo");
        assert_eq!(h.translate_path("/workspace/file.py"), "/workspace/file.py");
    }

    #[test]
    fn test_translate_path_relative() {
        let h = make_handler();
        assert_eq!(h.translate_path("src/main.rs"), "/workspace/src/main.rs");
        assert_eq!(h.translate_path("./src/main.rs"), "/workspace/src/main.rs");
    }

    #[test]
    fn test_translate_path_absolute_host() {
        let h = make_handler();
        assert_eq!(
            h.translate_path("/Users/me/project/file.py"),
            "/workspace/file.py"
        );
    }

    #[test]
    fn test_check_command_has_error_nonzero_exit() {
        assert!(DockerToolHandler::check_command_has_error(1, ""));
        assert!(DockerToolHandler::check_command_has_error(127, ""));
    }

    #[test]
    fn test_check_command_has_error_patterns() {
        assert!(DockerToolHandler::check_command_has_error(
            0,
            "Traceback (most recent call last)"
        ));
        assert!(DockerToolHandler::check_command_has_error(
            0,
            "ModuleNotFoundError: foo"
        ));
        assert!(!DockerToolHandler::check_command_has_error(0, "all good"));
    }

    #[tokio::test]
    async fn test_run_command_empty() {
        let h = make_handler();
        let r = h.run_command("", 10.0, None).await;
        assert!(!r.success);
        assert!(r.error.as_deref().unwrap().contains("required"));
    }

    #[tokio::test]
    async fn test_read_file_empty_path() {
        let h = make_handler();
        let r = h.read_file("").await;
        assert!(!r.success);
    }

    #[tokio::test]
    async fn test_write_file_empty_path() {
        let h = make_handler();
        let r = h.write_file("", "content").await;
        assert!(!r.success);
    }

    #[tokio::test]
    async fn test_search_empty_query() {
        let h = make_handler();
        let r = h.search("", None).await;
        assert!(!r.success);
    }
}
