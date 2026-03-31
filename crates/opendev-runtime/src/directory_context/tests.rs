use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tempfile::TempDir;

use super::DirectoryRegistry;
use crate::event_bus::{GlobalEventBus, RuntimeEvent, now_ms};

fn tmp_dir() -> TempDir {
    TempDir::new().unwrap()
}

fn canon(t: &TempDir) -> std::path::PathBuf {
    t.path().canonicalize().unwrap()
}

#[tokio::test]
async fn test_get_or_create_new() {
    let base = tmp_dir();
    let registry = DirectoryRegistry::new(canon(&base), Duration::from_secs(1800));

    let project = tmp_dir();
    let ctx = registry.get_or_create(&canon(&project)).await.unwrap();

    assert_eq!(ctx.working_dir(), canon(&project));
    assert_eq!(registry.active_count().await, 1);
}

#[tokio::test]
async fn test_get_or_create_returns_same() {
    let base = tmp_dir();
    let registry = DirectoryRegistry::new(canon(&base), Duration::from_secs(1800));

    let project = tmp_dir();
    let p = canon(&project);
    let ctx1 = registry.get_or_create(&p).await.unwrap();
    let ctx2 = registry.get_or_create(&p).await.unwrap();

    assert!(Arc::ptr_eq(&ctx1, &ctx2));
    assert_eq!(registry.active_count().await, 1);
}

#[tokio::test]
async fn test_dispose_removes_context() {
    let base = tmp_dir();
    let registry = DirectoryRegistry::new(canon(&base), Duration::from_secs(1800));

    let project = tmp_dir();
    let p = canon(&project);
    registry.get_or_create(&p).await.unwrap();
    assert_eq!(registry.active_count().await, 1);

    registry.dispose(&p).await;
    assert_eq!(registry.active_count().await, 0);
    assert!(registry.get(&p).await.is_none());
}

#[tokio::test]
async fn test_cleanup_idle_removes_old_contexts() {
    let base = tmp_dir();
    // Very short max_idle so we can trigger cleanup easily.
    let registry = DirectoryRegistry::new(canon(&base), Duration::from_millis(50));

    let project = tmp_dir();
    let p = canon(&project);
    let ctx = registry.get_or_create(&p).await.unwrap();

    // Manually backdate last_activity by 200ms.
    let old = now_ms().saturating_sub(200);
    ctx.last_activity.store(old, Ordering::Relaxed);

    let removed = registry.cleanup_idle().await;
    assert_eq!(removed, 1);
    assert_eq!(registry.active_count().await, 0);
}

#[tokio::test]
async fn test_cleanup_preserves_active_contexts() {
    let base = tmp_dir();
    let registry = DirectoryRegistry::new(canon(&base), Duration::from_secs(3600));

    let project = tmp_dir();
    let p = canon(&project);
    registry.get_or_create(&p).await.unwrap();

    // Context was just touched, max_idle is 1 hour -- nothing should be removed.
    let removed = registry.cleanup_idle().await;
    assert_eq!(removed, 0);
    assert_eq!(registry.active_count().await, 1);
}

#[tokio::test]
async fn test_active_count() {
    let base = tmp_dir();
    let registry = DirectoryRegistry::new(canon(&base), Duration::from_secs(1800));

    assert_eq!(registry.active_count().await, 0);

    let p1 = tmp_dir();
    let p2 = tmp_dir();
    registry.get_or_create(&canon(&p1)).await.unwrap();
    assert_eq!(registry.active_count().await, 1);

    registry.get_or_create(&canon(&p2)).await.unwrap();
    assert_eq!(registry.active_count().await, 2);

    registry.dispose(&canon(&p1)).await;
    assert_eq!(registry.active_count().await, 1);
}

#[tokio::test]
async fn test_directory_events_forwarded_to_global() {
    let base = tmp_dir();
    let global_bus = Arc::new(GlobalEventBus::new());
    let mut global_rx = global_bus.subscribe();

    let registry = DirectoryRegistry::new(canon(&base), Duration::from_secs(1800))
        .with_global_bus(Arc::clone(&global_bus));

    let project = tmp_dir();
    let ctx = registry.get_or_create(&canon(&project)).await.unwrap();

    // Publish an event on the directory-scoped bus.
    let event = RuntimeEvent::ToolCallStart {
        tool_name: "bash".into(),
        call_id: "c1".into(),
        timestamp_ms: now_ms(),
    };
    ctx.event_bus().publish(event);

    // The forwarding task should deliver it to the global bus.
    let received = tokio::time::timeout(Duration::from_secs(2), global_rx.recv())
        .await
        .expect("timed out waiting for forwarded event")
        .expect("recv failed");

    assert!(matches!(received, RuntimeEvent::ToolCallStart { .. }));
    if let RuntimeEvent::ToolCallStart { tool_name, .. } = received {
        assert_eq!(tool_name, "bash");
    }
}

#[tokio::test]
async fn test_registry_without_global_bus_works() {
    let base = tmp_dir();
    let registry = DirectoryRegistry::new(canon(&base), Duration::from_secs(1800));

    let project = tmp_dir();
    let ctx = registry.get_or_create(&canon(&project)).await.unwrap();

    // Publishing without a global bus should not panic.
    ctx.event_bus().publish(RuntimeEvent::ConfigReloaded {
        timestamp_ms: now_ms(),
    });
}
