use super::*;

#[test]
fn test_new_controller_is_inactive() {
    let ctrl = ApprovalController::new();
    assert!(!ctrl.active());
    assert!(ctrl.options().is_empty());
}

#[tokio::test]
async fn test_start_activates() {
    let mut ctrl = ApprovalController::new();
    let _rx = ctrl.start("rm -rf /tmp/test".into(), "/home/user".into(), None);
    assert!(ctrl.active());
    assert_eq!(ctrl.options().len(), 3);
    assert_eq!(ctrl.command(), "rm -rf /tmp/test");
    assert_eq!(ctrl.selected_index(), 0);
}

#[tokio::test]
async fn test_start_with_diff() {
    let mut ctrl = ApprovalController::new();
    let diff = Some("--- a/file\n+++ b/file\n@@ -1 +1 @@\n-old\n+new".to_string());
    let _rx = ctrl.start("edit".into(), ".".into(), diff.clone());
    assert!(ctrl.active());
    assert_eq!(ctrl.diff(), diff.as_deref());
}

#[tokio::test]
async fn test_move_selection_wraps() {
    let mut ctrl = ApprovalController::new();
    let _rx = ctrl.start("cmd".into(), ".".into(), None);

    ctrl.move_selection(1); // 0 -> 1
    assert_eq!(ctrl.selected_index(), 1);

    ctrl.move_selection(1); // 1 -> 2
    assert_eq!(ctrl.selected_index(), 2);

    ctrl.move_selection(1); // 2 -> 0 (wrap)
    assert_eq!(ctrl.selected_index(), 0);

    ctrl.move_selection(-1); // 0 -> 2 (wrap back)
    assert_eq!(ctrl.selected_index(), 2);
}

#[tokio::test]
async fn test_confirm_sends_decision() {
    let mut ctrl = ApprovalController::new();
    let rx = ctrl.start("git push".into(), ".".into(), None);

    // Default selection is "Yes" (index 0)
    ctrl.confirm();
    assert!(!ctrl.active());

    let decision = rx.await.unwrap();
    assert!(decision.approved);
    assert_eq!(decision.choice, "1");
    assert_eq!(decision.command, "git push");
}

#[tokio::test]
async fn test_cancel_sends_no() {
    let mut ctrl = ApprovalController::new();
    let rx = ctrl.start("dangerous cmd".into(), ".".into(), None);

    ctrl.cancel();
    assert!(!ctrl.active());

    let decision = rx.await.unwrap();
    assert!(!decision.approved);
    assert_eq!(decision.choice, "3");
}

#[tokio::test]
async fn test_confirm_second_option() {
    let mut ctrl = ApprovalController::new();
    let rx = ctrl.start("npm install".into(), ".".into(), None);

    ctrl.move_selection(1); // Select "Yes, and don't ask again"
    ctrl.confirm();

    let decision = rx.await.unwrap();
    assert!(decision.approved);
    assert_eq!(decision.choice, "2");
}

#[test]
fn test_move_on_inactive_is_noop() {
    let mut ctrl = ApprovalController::new();
    ctrl.move_selection(1); // Should not panic
    assert!(!ctrl.active());
}

#[test]
fn test_confirm_on_inactive_is_noop() {
    let mut ctrl = ApprovalController::new();
    ctrl.confirm(); // Should not panic
}

#[test]
fn test_auto_desc_with_prefix() {
    let mut ctrl = ApprovalController::new();
    let _rx = ctrl.start("git push --force".into(), "/project".into(), None);
    let opt2 = &ctrl.options()[1];
    assert!(opt2.description.contains("git"));
    assert!(opt2.description.contains("/project"));
}
