//! Governed Orchestration Loop: Planner → Worker → Validator.
//!
//! Provides a durable state machine for agent sessions, ensuring that
//! every action is planned, executed with checkpoints, and validated.
#![allow(clippy::collapsible_if)]

use crate::snapshot::SnapshotManager;
use kendra_models::orchestration::{OrchestrationPhase, OrchestrationState, StepStatus};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info};

#[derive(Debug)]
pub struct GovernedOrchestrator {
    state: OrchestrationState,
    state_path: PathBuf,
    snapshot_manager: SnapshotManager,
}

impl GovernedOrchestrator {
    pub fn new(session_id: String, project_dir: &Path, state_dir: PathBuf) -> Self {
        let state_path = state_dir.join(format!("{}_orchestration.json", session_id));
        let snapshot_manager = SnapshotManager::new(project_dir);

        let state = if state_path.exists() {
            match fs::read_to_string(&state_path) {
                Ok(content) => serde_json::from_str(&content)
                    .unwrap_or_else(|_| OrchestrationState::new(session_id)),
                Err(_) => OrchestrationState::new(session_id),
            }
        } else {
            OrchestrationState::new(session_id)
        };

        Self {
            state,
            state_path,
            snapshot_manager,
        }
    }

    /// Create a checkpoint (snapshot) for the current step.
    pub fn create_checkpoint(&mut self, files: &[&str]) {
        let label = format!("step_{}", self.state.current_step_index);
        if let Some(snapshot_id) = self.snapshot_manager.take_snapshot(files, &label) {
            if let Some(step) = self.state.steps.get_mut(self.state.current_step_index) {
                step.checkpoint_id = Some(snapshot_id);
                self.save();
            }
        }
    }

    /// Rollback the current step to its initial state.
    pub fn rollback_current_step(&mut self) {
        if let Some(step) = self.state.steps.get(self.state.current_step_index) {
            if let Some(ref snapshot_id) = step.checkpoint_id {
                info!(
                    "Rolling back step {} to checkpoint {}",
                    self.state.current_step_index, snapshot_id
                );
                self.snapshot_manager.revert_to_snapshot(snapshot_id);
            }
        }
    }

    pub fn phase(&self) -> OrchestrationPhase {
        self.state.phase
    }

    /// Transition to a new orchestration phase.
    pub fn transition(&mut self, new_phase: OrchestrationPhase) {
        info!(
            "Orchestration Transition: {:?} -> {:?}",
            self.state.phase, new_phase
        );
        self.state.phase = new_phase;
        self.save();
    }

    /// Mark the current step as completed and transition to validation.
    pub fn complete_current_step(&mut self) {
        if let Some(step) = self.state.steps.get_mut(self.state.current_step_index) {
            step.status = StepStatus::Completed;
        }
        self.transition(OrchestrationPhase::Validate);
    }

    /// Fail the current step and trigger a re-plan.
    pub fn fail_current_step(&mut self) {
        if let Some(step) = self.state.steps.get_mut(self.state.current_step_index) {
            step.status = StepStatus::Failed;
        }
        self.transition(OrchestrationPhase::Plan);
    }

    /// Persist the state to disk.
    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.state) {
            if let Err(e) = fs::write(&self.state_path, json) {
                error!("Failed to save orchestration state: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests;
