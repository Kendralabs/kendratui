//! Shadow git snapshot system for per-step undo.
//!
//! Maintains a parallel shadow git repository at `~/.opendev/snapshot/<project_id>/`
//! that captures a tree hash at every agent step, enabling perfect per-step
//! undo/revert without touching the user's real git repo.

use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info, warn};

/// Create a stable, filesystem-safe ID from a project path.
fn encode_project_id(project_path: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    project_path.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Manages shadow git snapshots for per-step undo.
///
/// Each snapshot is a git tree hash that captures the complete state
/// of the workspace at that point in time.
pub struct SnapshotManager {
    project_dir: String,
    #[allow(dead_code)]
    project_id: String,
    shadow_dir: PathBuf,
    snapshots: Vec<String>,
    initialized: bool,
}

impl SnapshotManager {
    /// Create a new snapshot manager for a project.
    pub fn new(project_dir: &str) -> Self {
        let abs_path = std::path::absolute(Path::new(project_dir))
            .unwrap_or_else(|_| PathBuf::from(project_dir));
        let project_dir_str = abs_path.to_string_lossy().to_string();
        let project_id = encode_project_id(&project_dir_str);
        let shadow_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".opendev")
            .join("snapshot")
            .join(&project_id);

        Self {
            project_dir: project_dir_str,
            project_id,
            shadow_dir,
            snapshots: Vec::new(),
            initialized: false,
        }
    }

    /// Path to the shadow .git directory.
    pub fn shadow_git_dir(&self) -> &Path {
        &self.shadow_dir
    }

    /// Number of snapshots recorded this session.
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Capture current workspace state as a tree hash.
    pub fn track(&mut self) -> Option<String> {
        if !self.ensure_initialized() {
            return None;
        }

        match self.git(&["--work-tree", &self.project_dir, "add", "--all", "--force"]) {
            Ok(_) => {}
            Err(e) => {
                debug!("Failed to stage files: {}", e);
                return None;
            }
        }

        match self.git(&["write-tree"]) {
            Ok(output) => {
                let tree_hash = output.trim().to_string();
                if !tree_hash.is_empty() {
                    self.snapshots.push(tree_hash.clone());
                    debug!(
                        "Snapshot captured: {} (total: {})",
                        &tree_hash[..8.min(tree_hash.len())],
                        self.snapshots.len()
                    );
                    Some(tree_hash)
                } else {
                    None
                }
            }
            Err(e) => {
                debug!("Failed to write tree: {}", e);
                None
            }
        }
    }

    /// Get list of files that changed since a snapshot.
    pub fn patch(&mut self, tree_hash: &str) -> Vec<String> {
        if !self.ensure_initialized() {
            return Vec::new();
        }

        let current_hash = match self.track() {
            Some(h) => h,
            None => return Vec::new(),
        };

        match self.git(&["diff-tree", "-r", "--name-only", tree_hash, &current_hash]) {
            Ok(output) => output
                .lines()
                .filter(|line| !line.is_empty())
                .map(|s| s.to_string())
                .collect(),
            Err(e) => {
                debug!("Failed to compute patch: {}", e);
                Vec::new()
            }
        }
    }

    /// Restore specific files (or all) from a snapshot.
    pub fn revert(&mut self, tree_hash: &str, files: Option<Vec<String>>) -> Vec<String> {
        if !self.ensure_initialized() {
            return Vec::new();
        }

        let files_to_restore = match files {
            Some(f) => f,
            None => self.patch(tree_hash),
        };

        if files_to_restore.is_empty() {
            return Vec::new();
        }

        let mut restored = Vec::new();
        for filepath in &files_to_restore {
            match self.git(&[
                "--work-tree",
                &self.project_dir,
                "checkout",
                tree_hash,
                "--",
                filepath,
            ]) {
                Ok(_) => restored.push(filepath.clone()),
                Err(_e) => {
                    debug!(
                        "Failed to restore {} from {}",
                        filepath,
                        &tree_hash[..8.min(tree_hash.len())]
                    );
                }
            }
        }

        if !restored.is_empty() {
            info!(
                "Restored {} files from snapshot {}",
                restored.len(),
                &tree_hash[..8.min(tree_hash.len())]
            );
        }
        restored
    }

    /// Full restoration to a snapshot state.
    pub fn restore(&mut self, tree_hash: &str) -> bool {
        if !self.ensure_initialized() {
            return false;
        }

        if let Err(e) = self.git(&["read-tree", tree_hash]) {
            warn!("Failed to read-tree: {}", e);
            return false;
        }

        match self.git(&[
            "--work-tree",
            &self.project_dir,
            "checkout-index",
            "--all",
            "--force",
        ]) {
            Ok(_) => {
                info!(
                    "Fully restored workspace to snapshot {}",
                    &tree_hash[..8.min(tree_hash.len())]
                );
                true
            }
            Err(e) => {
                warn!("Failed to checkout-index: {}", e);
                false
            }
        }
    }

    /// Revert to the snapshot before the most recent one.
    pub fn undo_last(&mut self) -> Option<String> {
        if self.snapshots.len() < 2 {
            return None;
        }

        self.snapshots.pop();
        let target_hash = self.snapshots.last()?.clone();

        let changed = self.patch(&target_hash);
        if changed.is_empty() {
            return None;
        }

        if self.restore(&target_hash) {
            let desc = if changed.len() <= 5 {
                format!(
                    "Reverted {} file(s) to previous state: {}",
                    changed.len(),
                    changed.join(", ")
                )
            } else {
                format!("Reverted {} file(s) to previous state", changed.len())
            };
            Some(desc)
        } else {
            None
        }
    }

    /// Run git gc on the shadow repo to free space.
    pub fn cleanup(&self) {
        if !self.initialized {
            return;
        }
        let _ = self.git(&["gc", "--prune=7.days.ago", "--quiet"]);
    }

    fn ensure_initialized(&mut self) -> bool {
        if self.initialized {
            return true;
        }

        if let Err(e) = std::fs::create_dir_all(&self.shadow_dir) {
            warn!("Failed to create shadow dir: {}", e);
            return false;
        }

        // Check if already a git repo
        if self.shadow_dir.join("HEAD").exists() {
            self.initialized = true;
            return true;
        }

        // Initialize bare-ish shadow repo
        match self.git(&["init", "--bare"]) {
            Ok(_) => {
                self.initialized = true;
                info!(
                    "Shadow snapshot repo initialized at {}",
                    self.shadow_dir.display()
                );
                true
            }
            Err(e) => {
                warn!("Failed to initialize shadow snapshot repo: {}", e);
                false
            }
        }
    }

    fn git(&self, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new("git");
        cmd.arg("--git-dir")
            .arg(self.shadow_dir.to_string_lossy().as_ref());
        for arg in args {
            cmd.arg(arg);
        }
        cmd.current_dir(&self.project_dir);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute git: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(stderr)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_project_id() {
        let id1 = encode_project_id("/Users/foo/project");
        let id2 = encode_project_id("/Users/foo/project");
        assert_eq!(id1, id2); // Deterministic

        let id3 = encode_project_id("/Users/bar/project");
        assert_ne!(id1, id3); // Different paths -> different IDs

        assert_eq!(id1.len(), 16); // Fixed width hex
    }

    #[test]
    fn test_snapshot_manager_new() {
        let mgr = SnapshotManager::new("/tmp/test-project");
        assert_eq!(mgr.snapshot_count(), 0);
        assert!(!mgr.initialized);
    }

    // Integration tests that require git are skipped in CI
    // but can be run locally with: cargo test -- --ignored

    #[test]
    #[ignore]
    fn test_snapshot_track_and_patch() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_dir = tmp.path().to_string_lossy().to_string();

        // Initialize a git repo in the project dir
        Command::new("git")
            .args(["init"])
            .current_dir(&project_dir)
            .output()
            .unwrap();

        // Create a file
        std::fs::write(tmp.path().join("test.txt"), "hello").unwrap();

        let mut mgr = SnapshotManager::new(&project_dir);
        let hash1 = mgr.track();
        assert!(hash1.is_some());
        assert_eq!(mgr.snapshot_count(), 1);

        // Modify the file
        std::fs::write(tmp.path().join("test.txt"), "hello world").unwrap();

        let changed = mgr.patch(hash1.as_ref().unwrap());
        assert!(changed.contains(&"test.txt".to_string()));
    }
}
