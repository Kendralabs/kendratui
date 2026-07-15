use super::*;
use tempfile::tempdir;

#[test]
fn test_orchestrator_initialization() {
    let tmp = tempdir().unwrap();
    let project_dir = tmp.path().join("project");
    std::fs::create_dir(&project_dir).unwrap();
    let session_id = "test-session".to_string();
    let orchestrator =
        GovernedOrchestrator::new(session_id.clone(), &project_dir, tmp.path().to_path_buf());

    assert_eq!(orchestrator.phase(), OrchestrationPhase::Explore);
}

#[test]
fn test_orchestrator_transition_and_persistence() {
    let tmp = tempdir().unwrap();
    let project_dir = tmp.path().join("project");
    std::fs::create_dir(&project_dir).unwrap();
    let session_id = "test-session".to_string();
    let mut orchestrator =
        GovernedOrchestrator::new(session_id.clone(), &project_dir, tmp.path().to_path_buf());

    orchestrator.transition(OrchestrationPhase::Plan);
    assert_eq!(orchestrator.phase(), OrchestrationPhase::Plan);

    // Verify file exists
    let state_path = tmp.path().join("test-session_orchestration.json");
    assert!(state_path.exists());

    // Create new orchestrator to verify reload
    let orchestrator2 =
        GovernedOrchestrator::new(session_id, &project_dir, tmp.path().to_path_buf());
    assert_eq!(orchestrator2.phase(), OrchestrationPhase::Plan);
}

#[test]
fn test_orchestrator_step_completion() {
    let tmp = tempdir().unwrap();
    let project_dir = tmp.path().join("project");
    std::fs::create_dir(&project_dir).unwrap();
    let session_id = "test-session".to_string();
    let mut orchestrator =
        GovernedOrchestrator::new(session_id, &project_dir, tmp.path().to_path_buf());

    // Add a mock step
    orchestrator
        .state
        .steps
        .push(kendra_models::orchestration::PlanStep {
            id: "step1".to_string(),
            description: "description".to_string(),
            status: StepStatus::Pending,
            checkpoint_id: None,
        });

    orchestrator.complete_current_step();
    assert_eq!(orchestrator.phase(), OrchestrationPhase::Validate);
    assert_eq!(orchestrator.state.steps[0].status, StepStatus::Completed);
}
