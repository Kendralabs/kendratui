use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The phase of the governed orchestration loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrchestrationPhase {
    /// Initial state: understanding the request and exploring the codebase.
    Explore,
    /// Formalizing a step-by-step execution plan.
    Plan,
    /// Executing specific steps of the plan.
    Work,
    /// Verifying that the work performed matches the plan and requirements.
    Validate,
}

/// A single step in an orchestration plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub description: String,
    pub status: StepStatus,
    /// Optional checkpoint ID created before starting this step.
    pub checkpoint_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// The durable state of an agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationState {
    pub session_id: String,
    pub phase: OrchestrationPhase,
    pub steps: Vec<PlanStep>,
    pub current_step_index: usize,
    /// Key-value store for session-specific context.
    pub metadata: HashMap<String, String>,
}

impl OrchestrationState {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            phase: OrchestrationPhase::Explore,
            steps: Vec::new(),
            current_step_index: 0,
            metadata: HashMap::new(),
        }
    }
}
