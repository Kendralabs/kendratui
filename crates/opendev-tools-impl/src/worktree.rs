//! Git worktree management for isolated agent workspaces.
//!
//! Mirrors `opendev/core/git/worktree.py`.
//!
//! Provides [`WorktreeManager`] for creating, listing, removing, and
//! cleaning up git worktrees.  Each worktree gives an agent an isolated
//! checkout where it can make changes without interfering with other
//! sessions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, warn};

// ── Naming ──────────────────────────────────────────────────────────────────

const ADJECTIVES: &[&str] = &[
    "swift", "bright", "calm", "bold", "keen", "warm", "cool", "deep", "fair", "fine", "glad",
    "pure", "safe", "wise", "neat",
];

const NOUNS: &[&str] = &[
    "branch", "patch", "spike", "draft", "build", "probe", "trial", "craft", "forge", "bloom",
    "spark", "quest", "grove", "ridge", "haven",
];

/// Generate a random adjective-noun worktree name.
fn random_name() -> String {
    use std::time::SystemTime;
    // Simple deterministic-enough RNG from timestamp nanos
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    let adj = ADJECTIVES[seed % ADJECTIVES.len()];
    let noun = NOUNS[(seed / ADJECTIVES.len()) % NOUNS.len()];
    format!("{adj}-{noun}")
}

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error("git command failed: {0}")]
    GitError(String),
    #[error("worktree not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("worktree already exists: {0}")]
    AlreadyExists(String),
}

// ── WorktreeInfo ────────────────────────────────────────────────────────────

/// Information about a single git worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    /// Absolute path to the worktree directory.
    pub path: String,
    /// Branch checked out in this worktree.
    pub branch: String,
    /// HEAD commit hash.
    pub commit: String,
    /// Whether this is the main (bare) worktree.
    pub is_main: bool,
}

impl std::fmt::Display for WorktreeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let suffix = if self.is_main { " (main)" } else { "" };
        write!(f, "Worktree({}{}, {})", self.branch, suffix, self.path)
    }
}

// ── WorktreeManager ─────────────────────────────────────────────────────────

/// Manages git worktrees for a project.
pub struct WorktreeManager {
    /// Root directory of the git repository.
    project_dir: PathBuf,
    /// Base directory for storing worktrees.
    worktree_base: PathBuf,
    /// Tracked worktrees in session state: name -> WorktreeInfo.
    tracked: HashMap<String, WorktreeInfo>,
}

impl WorktreeManager {
    /// Create a new manager for the given project directory.
    ///
    /// Worktrees are stored under `~/.opendev/data/worktree/`.
    pub fn new(project_dir: impl Into<PathBuf>) -> Self {
        let project_dir = project_dir.into();
        let worktree_base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".opendev")
            .join("data")
            .join("worktree");
        Self {
            project_dir,
            worktree_base,
            tracked: HashMap::new(),
        }
    }

    /// Create a new manager with a custom worktree base directory.
    ///
    /// Primarily useful for tests.
    pub fn with_base(project_dir: impl Into<PathBuf>, worktree_base: impl Into<PathBuf>) -> Self {
        Self {
            project_dir: project_dir.into(),
            worktree_base: worktree_base.into(),
            tracked: HashMap::new(),
        }
    }

    /// Get the project directory.
    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    /// Get the worktree base directory.
    pub fn worktree_base(&self) -> &Path {
        &self.worktree_base
    }

    /// Create a new worktree.
    ///
    /// - `name`: worktree name (auto-generated if `None`)
    /// - `branch`: branch name (defaults to `worktree-{name}`)
    /// - `base_branch`: base commit/branch to start from (defaults to `"HEAD"`)
    pub async fn create(
        &mut self,
        name: Option<&str>,
        branch: Option<&str>,
        base_branch: &str,
    ) -> Result<WorktreeInfo, WorktreeError> {
        let name = name
            .map(String::from)
            .unwrap_or_else(random_name);
        let branch = branch
            .map(String::from)
            .unwrap_or_else(|| format!("worktree-{name}"));
        let worktree_path = self.worktree_base.join(&name);

        if worktree_path.exists() {
            return Err(WorktreeError::AlreadyExists(name));
        }

        // Ensure parent exists
        if let Some(parent) = worktree_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let output = Command::new("git")
            .args(["worktree", "add", "-b", &branch])
            .arg(&worktree_path)
            .arg(base_branch)
            .current_dir(&self.project_dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            warn!("Failed to create worktree: {stderr}");
            return Err(WorktreeError::GitError(stderr));
        }

        // Read HEAD commit in the new worktree
        let commit = self
            .git_output(&["rev-parse", "HEAD"], Some(&worktree_path))
            .await
            .unwrap_or_default();

        let info = WorktreeInfo {
            path: worktree_path.to_string_lossy().to_string(),
            branch,
            commit,
            is_main: false,
        };

        debug!("Created worktree: {info}");
        self.tracked.insert(name, info.clone());
        Ok(info)
    }

    /// List all worktrees for the project (from `git worktree list --porcelain`).
    pub async fn list(&self) -> Result<Vec<WorktreeInfo>, WorktreeError> {
        let raw = self
            .git_output(&["worktree", "list", "--porcelain"], None)
            .await
            .ok_or_else(|| WorktreeError::GitError("git worktree list failed".into()))?;

        Ok(parse_porcelain_output(&raw))
    }

    /// Remove a worktree by name (or absolute path).
    pub async fn remove(&mut self, name: &str, force: bool) -> Result<(), WorktreeError> {
        let worktree_path = self.resolve_worktree_path(name);

        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        let path_str = worktree_path.to_string_lossy().to_string();
        args.push(&path_str);

        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.project_dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            warn!("Failed to remove worktree: {stderr}");
            return Err(WorktreeError::GitError(stderr));
        }

        self.tracked.remove(name);
        debug!("Removed worktree: {name}");
        Ok(())
    }

    /// Clean up stale/prunable worktree references.
    pub async fn cleanup(&self) -> Result<String, WorktreeError> {
        let output = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(&self.project_dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(WorktreeError::GitError(stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        debug!("Worktree cleanup done");
        Ok(stdout)
    }

    /// Get tracked worktrees in session state.
    pub fn tracked(&self) -> &HashMap<String, WorktreeInfo> {
        &self.tracked
    }

    /// Track a worktree in session state.
    pub fn track(&mut self, name: String, info: WorktreeInfo) {
        self.tracked.insert(name, info);
    }

    /// Untrack a worktree from session state.
    pub fn untrack(&mut self, name: &str) -> Option<WorktreeInfo> {
        self.tracked.remove(name)
    }

    // ── internal helpers ────────────────────────────────────────────────────

    fn resolve_worktree_path(&self, name: &str) -> PathBuf {
        let candidate = self.worktree_base.join(name);
        if candidate.exists() {
            candidate
        } else {
            // Try treating as absolute path
            PathBuf::from(name)
        }
    }

    async fn git_output(&self, args: &[&str], cwd: Option<&Path>) -> Option<String> {
        let cwd = cwd.unwrap_or(&self.project_dir);
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

// ── Parsing ─────────────────────────────────────────────────────────────────

/// Parse `git worktree list --porcelain` output into [`WorktreeInfo`] entries.
fn parse_porcelain_output(raw: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut path = String::new();
    let mut commit = String::new();
    let mut branch = String::new();
    let mut is_main = false;
    let mut has_entry = false;

    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("worktree ") {
            // Flush previous entry
            if has_entry {
                worktrees.push(WorktreeInfo {
                    path: std::mem::take(&mut path),
                    branch: if branch.is_empty() {
                        "detached".to_string()
                    } else {
                        std::mem::take(&mut branch)
                    },
                    commit: std::mem::take(&mut commit),
                    is_main,
                });
                is_main = false;
            }
            path = rest.to_string();
            has_entry = true;
        } else if let Some(rest) = line.strip_prefix("HEAD ") {
            commit = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("branch ") {
            branch = rest.replace("refs/heads/", "");
        } else if line == "bare" {
            is_main = true;
        }
    }

    // Flush last entry
    if has_entry {
        worktrees.push(WorktreeInfo {
            path,
            branch: if branch.is_empty() {
                "detached".to_string()
            } else {
                branch
            },
            commit,
            is_main,
        });
    }

    worktrees
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_name_format() {
        let name = random_name();
        assert!(name.contains('-'), "random name should contain a hyphen");
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 2, "should have exactly two parts");
    }

    #[test]
    fn test_parse_porcelain_empty() {
        let result = parse_porcelain_output("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_porcelain_single_main() {
        let output = "\
worktree /home/user/project
HEAD abc123def456
branch refs/heads/main
";
        let result = parse_porcelain_output(output);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "/home/user/project");
        assert_eq!(result[0].branch, "main");
        assert_eq!(result[0].commit, "abc123def456");
        assert!(!result[0].is_main);
    }

    #[test]
    fn test_parse_porcelain_bare_worktree() {
        let output = "\
worktree /home/user/project
HEAD abc123
bare
";
        let result = parse_porcelain_output(output);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_main);
        assert_eq!(result[0].branch, "detached");
    }

    #[test]
    fn test_parse_porcelain_multiple() {
        let output = "\
worktree /home/user/project
HEAD aaa111
branch refs/heads/main

worktree /home/user/.opendev/data/worktree/swift-patch
HEAD bbb222
branch refs/heads/worktree-swift-patch

worktree /home/user/.opendev/data/worktree/calm-spike
HEAD ccc333
branch refs/heads/worktree-calm-spike
";
        let result = parse_porcelain_output(output);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].branch, "main");
        assert_eq!(result[1].branch, "worktree-swift-patch");
        assert_eq!(result[2].branch, "worktree-calm-spike");
    }

    #[test]
    fn test_worktree_info_display() {
        let info = WorktreeInfo {
            path: "/tmp/wt".into(),
            branch: "feature-x".into(),
            commit: "abc".into(),
            is_main: false,
        };
        let s = format!("{info}");
        assert!(s.contains("feature-x"));
        assert!(s.contains("/tmp/wt"));
        assert!(!s.contains("(main)"));

        let main_info = WorktreeInfo {
            path: "/tmp/main".into(),
            branch: "main".into(),
            commit: "def".into(),
            is_main: true,
        };
        let s = format!("{main_info}");
        assert!(s.contains("(main)"));
    }

    #[test]
    fn test_manager_new_default_base() {
        let mgr = WorktreeManager::new("/tmp/project");
        assert_eq!(mgr.project_dir(), Path::new("/tmp/project"));
        assert!(
            mgr.worktree_base()
                .to_string_lossy()
                .contains("worktree"),
            "base should contain 'worktree'"
        );
    }

    #[test]
    fn test_manager_with_base() {
        let mgr = WorktreeManager::with_base("/tmp/project", "/tmp/wt-base");
        assert_eq!(mgr.project_dir(), Path::new("/tmp/project"));
        assert_eq!(mgr.worktree_base(), Path::new("/tmp/wt-base"));
    }

    #[test]
    fn test_resolve_worktree_path_by_name() {
        let mgr = WorktreeManager::with_base("/tmp/project", "/tmp/wt-base");
        let resolved = mgr.resolve_worktree_path("my-worktree");
        // Since /tmp/wt-base/my-worktree doesn't exist, falls back to PathBuf::from
        assert_eq!(resolved, PathBuf::from("my-worktree"));
    }

    #[test]
    fn test_track_untrack() {
        let mut mgr = WorktreeManager::with_base("/tmp/project", "/tmp/wt-base");
        assert!(mgr.tracked().is_empty());

        let info = WorktreeInfo {
            path: "/tmp/wt-base/test".into(),
            branch: "wt-test".into(),
            commit: "abc".into(),
            is_main: false,
        };

        mgr.track("test".into(), info.clone());
        assert_eq!(mgr.tracked().len(), 1);
        assert_eq!(mgr.tracked().get("test"), Some(&info));

        let removed = mgr.untrack("test");
        assert_eq!(removed, Some(info));
        assert!(mgr.tracked().is_empty());
    }

    #[test]
    fn test_untrack_nonexistent() {
        let mut mgr = WorktreeManager::with_base("/tmp/project", "/tmp/wt-base");
        assert_eq!(mgr.untrack("nope"), None);
    }

    #[test]
    fn test_worktree_error_display() {
        let e = WorktreeError::NotFound("wt-1".into());
        assert!(e.to_string().contains("wt-1"));

        let e = WorktreeError::AlreadyExists("wt-2".into());
        assert!(e.to_string().contains("wt-2"));

        let e = WorktreeError::GitError("fatal: not a git repo".into());
        assert!(e.to_string().contains("fatal"));
    }

    #[test]
    fn test_parse_porcelain_detached_head() {
        let output = "\
worktree /tmp/detached
HEAD deadbeef
detached
";
        let result = parse_porcelain_output(output);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch, "detached");
        assert_eq!(result[0].commit, "deadbeef");
    }

    #[tokio::test]
    async fn test_list_in_non_git_dir() {
        let mgr = WorktreeManager::with_base("/tmp", "/tmp/wt-nonexist");
        let result = mgr.list().await;
        // /tmp is not a git repo, so this should fail
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_in_non_git_dir() {
        let mut mgr = WorktreeManager::with_base("/tmp", "/tmp/wt-create-test");
        let result = mgr.create(Some("test"), None, "HEAD").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cleanup_in_non_git_dir() {
        let mgr = WorktreeManager::with_base("/tmp", "/tmp/wt-cleanup-test");
        let result = mgr.cleanup().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_in_non_git_dir() {
        let mut mgr = WorktreeManager::with_base("/tmp", "/tmp/wt-remove-test");
        let result = mgr.remove("nonexistent", false).await;
        assert!(result.is_err());
    }
}
