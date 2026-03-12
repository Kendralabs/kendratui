//! Background task manager for tracking long-running operations.
//!
//! Mirrors Python's background task tracking from
//! `opendev/core/context_engineering/tools/background_task_manager.py`.

use std::collections::HashMap;
use std::time::Instant;

/// Status of a background task.
#[derive(Debug, Clone)]
pub struct TaskStatus {
    /// Human-readable description of the task.
    pub description: String,
    /// When the task was started.
    pub started_at: Instant,
    /// Current status label (e.g. "running", "completed", "failed").
    pub status: String,
}

/// Manages a set of background tasks identified by string IDs.
pub struct BackgroundTaskManager {
    tasks: HashMap<String, TaskStatus>,
}

impl BackgroundTaskManager {
    /// Create a new empty task manager.
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    /// Add a new task with the given ID and description.
    ///
    /// If a task with the same ID already exists, it is replaced.
    pub fn add_task(&mut self, id: String, description: String) {
        self.tasks.insert(
            id,
            TaskStatus {
                description,
                started_at: Instant::now(),
                status: "running".into(),
            },
        );
    }

    /// Update the status label of an existing task.
    ///
    /// Returns `true` if the task was found and updated.
    pub fn update_task(&mut self, id: &str, status: String) -> bool {
        if let Some(task) = self.tasks.get_mut(id) {
            task.status = status;
            true
        } else {
            false
        }
    }

    /// Remove a task by ID.
    ///
    /// Returns `true` if the task existed and was removed.
    pub fn remove_task(&mut self, id: &str) -> bool {
        self.tasks.remove(id).is_some()
    }

    /// Get all tasks that have "running" status.
    pub fn active_tasks(&self) -> Vec<(&str, &TaskStatus)> {
        self.tasks
            .iter()
            .filter(|(_, t)| t.status == "running")
            .map(|(id, t)| (id.as_str(), t))
            .collect()
    }

    /// Get a task by ID.
    pub fn get_task(&self, id: &str) -> Option<&TaskStatus> {
        self.tasks.get(id)
    }

    /// Total number of tracked tasks (all statuses).
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Whether there are no tracked tasks.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

impl Default for BackgroundTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let mgr = BackgroundTaskManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
        assert!(mgr.active_tasks().is_empty());
    }

    #[test]
    fn test_add_and_get() {
        let mut mgr = BackgroundTaskManager::new();
        mgr.add_task("t1".into(), "Build project".into());
        assert_eq!(mgr.len(), 1);

        let task = mgr.get_task("t1").unwrap();
        assert_eq!(task.description, "Build project");
        assert_eq!(task.status, "running");
    }

    #[test]
    fn test_update_task() {
        let mut mgr = BackgroundTaskManager::new();
        mgr.add_task("t1".into(), "Compiling".into());

        assert!(mgr.update_task("t1", "completed".into()));
        assert_eq!(mgr.get_task("t1").unwrap().status, "completed");

        // No longer active
        assert!(mgr.active_tasks().is_empty());

        // Non-existent task
        assert!(!mgr.update_task("nope", "failed".into()));
    }

    #[test]
    fn test_remove_task() {
        let mut mgr = BackgroundTaskManager::new();
        mgr.add_task("t1".into(), "Running tests".into());
        assert!(mgr.remove_task("t1"));
        assert!(mgr.is_empty());
        assert!(!mgr.remove_task("t1")); // already removed
    }

    #[test]
    fn test_active_tasks() {
        let mut mgr = BackgroundTaskManager::new();
        mgr.add_task("t1".into(), "Build".into());
        mgr.add_task("t2".into(), "Test".into());
        mgr.update_task("t1", "completed".into());

        let active = mgr.active_tasks();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].0, "t2");
    }
}
