//! Shared application state.
//!
//! Thread-safe state shared between HTTP handlers and WebSocket connections.
//! Uses `tokio::sync::oneshot` channels for approval, ask-user, and plan-approval
//! notification so that waiting agent tasks are woken immediately on resolution
//! (no polling).

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, RwLock};

use opendev_config::ModelRegistry;
use opendev_history::SessionManager;
use opendev_http::UserStore;
use opendev_models::AppConfig;

/// WebSocket broadcast message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WsBroadcast {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// Shared application state wrapped in Arc for use with Axum.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    /// Session manager for persistence.
    session_manager: RwLock<SessionManager>,
    /// Application configuration.
    config: RwLock<AppConfig>,
    /// Working directory for the current project.
    working_dir: String,
    /// Broadcast channel for WebSocket messages.
    ws_tx: broadcast::Sender<WsBroadcast>,
    /// Pending approval requests: approval_id -> (metadata, oneshot sender).
    pending_approvals: Mutex<HashMap<String, PendingApprovalSlot>>,
    /// Pending ask-user requests: request_id -> (metadata, oneshot sender).
    pending_ask_users: Mutex<HashMap<String, PendingAskUserSlot>>,
    /// Pending plan approval requests: request_id -> (metadata, oneshot sender).
    pending_plan_approvals: Mutex<HashMap<String, PendingPlanApprovalSlot>>,
    /// Current operation mode (normal/plan).
    mode: RwLock<OperationMode>,
    /// Autonomy level.
    autonomy_level: RwLock<String>,
    /// Thinking level.
    thinking_level: RwLock<String>,
    /// Interrupt flag.
    interrupt_requested: Mutex<bool>,
    /// Running sessions: session_id -> status.
    running_sessions: Mutex<HashMap<String, String>>,
    /// Live message injection queues: session_id -> bounded mpsc sender.
    injection_queues: Mutex<HashMap<String, mpsc::Sender<String>>>,
    /// Agent executor (trait-object, set once on first query).
    agent_executor: Mutex<Option<Arc<dyn AgentExecutor>>>,
    /// User store for authentication.
    user_store: Arc<UserStore>,
    /// Model/provider registry from models.dev cache.
    model_registry: RwLock<ModelRegistry>,
    /// Bridge mode state.
    bridge: RwLock<BridgeState>,
}

/// Bridge mode state: when the TUI owns agent execution and
/// the Web UI mirrors it.
#[derive(Debug, Default)]
struct BridgeState {
    /// Session ID currently owned by the TUI bridge.
    session_id: Option<String>,
    /// Whether bridge mode is active.
    active: bool,
}

/// Operation mode for the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationMode {
    Normal,
    Plan,
}

impl std::fmt::Display for OperationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationMode::Normal => write!(f, "normal"),
            OperationMode::Plan => write!(f, "plan"),
        }
    }
}

/// Metadata for a pending approval request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingApproval {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub session_id: Option<String>,
}

/// Internal slot holding approval metadata and the oneshot sender.
struct PendingApprovalSlot {
    pub meta: PendingApproval,
    pub tx: Option<oneshot::Sender<ApprovalResult>>,
}

/// Result sent through the oneshot channel when an approval is resolved.
#[derive(Debug, Clone)]
pub struct ApprovalResult {
    pub approved: bool,
    pub auto_approve: bool,
}

/// Metadata for a pending ask-user request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingAskUser {
    pub prompt: String,
    pub session_id: Option<String>,
}

/// Internal slot holding ask-user metadata and the oneshot sender.
struct PendingAskUserSlot {
    pub meta: PendingAskUser,
    pub tx: Option<oneshot::Sender<AskUserResult>>,
}

/// Result sent through the oneshot channel when ask-user is resolved.
#[derive(Debug, Clone)]
pub struct AskUserResult {
    pub answers: Option<serde_json::Value>,
    pub cancelled: bool,
}

/// Metadata for a pending plan approval request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingPlanApproval {
    pub data: serde_json::Value,
    pub session_id: Option<String>,
}

/// Internal slot holding plan-approval metadata and the oneshot sender.
struct PendingPlanApprovalSlot {
    pub meta: PendingPlanApproval,
    pub tx: Option<oneshot::Sender<PlanApprovalResult>>,
}

/// Result sent through the oneshot channel when a plan approval is resolved.
#[derive(Debug, Clone)]
pub struct PlanApprovalResult {
    pub action: String,
    pub feedback: String,
}

/// Trait for agent execution -- injected into AppState for testability.
#[async_trait::async_trait]
pub trait AgentExecutor: Send + Sync + 'static {
    /// Execute a query for a given session. Called as a background task.
    async fn execute_query(
        &self,
        message: String,
        session_id: String,
        state: AppState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Injection queue capacity per session.
const INJECTION_QUEUE_CAPACITY: usize = 10;

impl AppState {
    /// Create a new AppState.
    pub fn new(
        session_manager: SessionManager,
        config: AppConfig,
        working_dir: String,
        user_store: UserStore,
        model_registry: ModelRegistry,
    ) -> Self {
        let (ws_tx, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(AppStateInner {
                session_manager: RwLock::new(session_manager),
                config: RwLock::new(config),
                working_dir,
                ws_tx,
                pending_approvals: Mutex::new(HashMap::new()),
                pending_ask_users: Mutex::new(HashMap::new()),
                pending_plan_approvals: Mutex::new(HashMap::new()),
                mode: RwLock::new(OperationMode::Normal),
                autonomy_level: RwLock::new("Manual".to_string()),
                thinking_level: RwLock::new("Medium".to_string()),
                interrupt_requested: Mutex::new(false),
                running_sessions: Mutex::new(HashMap::new()),
                injection_queues: Mutex::new(HashMap::new()),
                agent_executor: Mutex::new(None),
                user_store: Arc::new(user_store),
                model_registry: RwLock::new(model_registry),
                bridge: RwLock::new(BridgeState::default()),
            }),
        }
    }

    // --- Session management ---

    /// Get read access to the session manager.
    pub async fn session_manager(&self) -> tokio::sync::RwLockReadGuard<'_, SessionManager> {
        self.inner.session_manager.read().await
    }

    /// Get write access to the session manager.
    pub async fn session_manager_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, SessionManager> {
        self.inner.session_manager.write().await
    }

    /// Get the current session ID.
    pub async fn current_session_id(&self) -> Option<String> {
        let mgr = self.inner.session_manager.read().await;
        mgr.current_session().map(|s| s.id.clone())
    }

    // --- Config ---

    /// Get read access to the configuration.
    pub async fn config(&self) -> tokio::sync::RwLockReadGuard<'_, AppConfig> {
        self.inner.config.read().await
    }

    /// Get write access to the configuration.
    pub async fn config_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, AppConfig> {
        self.inner.config.write().await
    }

    /// Get the working directory.
    pub fn working_dir(&self) -> &str {
        &self.inner.working_dir
    }

    // --- User store ---

    /// Get a reference to the user store.
    pub fn user_store(&self) -> &UserStore {
        &self.inner.user_store
    }

    // --- Model registry ---

    /// Get read access to the model registry.
    pub async fn model_registry(&self) -> tokio::sync::RwLockReadGuard<'_, ModelRegistry> {
        self.inner.model_registry.read().await
    }

    /// Get write access to the model registry (e.g. for refreshing).
    pub async fn model_registry_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, ModelRegistry> {
        self.inner.model_registry.write().await
    }

    // --- WebSocket broadcasting ---

    /// Get a broadcast sender for WebSocket messages.
    pub fn ws_sender(&self) -> broadcast::Sender<WsBroadcast> {
        self.inner.ws_tx.clone()
    }

    /// Subscribe to WebSocket broadcasts.
    pub fn ws_subscribe(&self) -> broadcast::Receiver<WsBroadcast> {
        self.inner.ws_tx.subscribe()
    }

    /// Broadcast a message to all WebSocket clients.
    pub fn broadcast(&self, msg: WsBroadcast) {
        // Ignore send errors (no subscribers).
        let _ = self.inner.ws_tx.send(msg);
    }

    // --- Mode ---

    /// Get the current operation mode.
    pub async fn mode(&self) -> OperationMode {
        *self.inner.mode.read().await
    }

    /// Set the operation mode.
    pub async fn set_mode(&self, mode: OperationMode) {
        *self.inner.mode.write().await = mode;
    }

    // --- Autonomy level ---

    /// Get the current autonomy level.
    pub async fn autonomy_level(&self) -> String {
        self.inner.autonomy_level.read().await.clone()
    }

    /// Set the autonomy level.
    pub async fn set_autonomy_level(&self, level: String) {
        *self.inner.autonomy_level.write().await = level;
    }

    // --- Thinking level ---

    /// Get the current thinking level.
    pub async fn thinking_level(&self) -> String {
        self.inner.thinking_level.read().await.clone()
    }

    /// Set the thinking level.
    pub async fn set_thinking_level(&self, level: String) {
        *self.inner.thinking_level.write().await = level;
    }

    // --- Interrupt ---

    /// Request interruption of ongoing task.
    ///
    /// Also denies all pending approvals, ask-user, and plan-approval requests
    /// by sending rejection through their oneshot channels so blocked tasks wake up.
    pub async fn request_interrupt(&self) {
        *self.inner.interrupt_requested.lock().await = true;

        // Deny all pending approvals.
        {
            let mut approvals = self.inner.pending_approvals.lock().await;
            for (_id, slot) in approvals.iter_mut() {
                if let Some(tx) = slot.tx.take() {
                    let _ = tx.send(ApprovalResult {
                        approved: false,
                        auto_approve: false,
                    });
                }
            }
            approvals.clear();
        }

        // Cancel all pending ask-user requests.
        {
            let mut ask_users = self.inner.pending_ask_users.lock().await;
            for (_id, slot) in ask_users.iter_mut() {
                if let Some(tx) = slot.tx.take() {
                    let _ = tx.send(AskUserResult {
                        answers: None,
                        cancelled: true,
                    });
                }
            }
            ask_users.clear();
        }

        // Reject all pending plan approvals.
        {
            let mut plan_approvals = self.inner.pending_plan_approvals.lock().await;
            for (_id, slot) in plan_approvals.iter_mut() {
                if let Some(tx) = slot.tx.take() {
                    let _ = tx.send(PlanApprovalResult {
                        action: "reject".to_string(),
                        feedback: "Interrupted".to_string(),
                    });
                }
            }
            plan_approvals.clear();
        }
    }

    /// Clear the interrupt flag.
    pub async fn clear_interrupt(&self) {
        *self.inner.interrupt_requested.lock().await = false;
    }

    /// Check if interrupt has been requested.
    pub async fn is_interrupt_requested(&self) -> bool {
        *self.inner.interrupt_requested.lock().await
    }

    // --- Running sessions ---

    /// Mark a session as running.
    pub async fn set_session_running(&self, session_id: String) {
        self.inner
            .running_sessions
            .lock()
            .await
            .insert(session_id, "running".to_string());
    }

    /// Mark a session as idle.
    pub async fn set_session_idle(&self, session_id: &str) {
        self.inner.running_sessions.lock().await.remove(session_id);
    }

    /// Check if a session is running.
    pub async fn is_session_running(&self, session_id: &str) -> bool {
        self.inner
            .running_sessions
            .lock()
            .await
            .contains_key(session_id)
    }

    // --- Approvals (oneshot-based) ---

    /// Add a pending approval request.
    ///
    /// Returns a `oneshot::Receiver` that the caller can `.await` to block
    /// until the approval is resolved (or the state is torn down / interrupted).
    pub async fn add_pending_approval(
        &self,
        id: String,
        approval: PendingApproval,
    ) -> oneshot::Receiver<ApprovalResult> {
        let (tx, rx) = oneshot::channel();
        self.inner.pending_approvals.lock().await.insert(
            id,
            PendingApprovalSlot {
                meta: approval,
                tx: Some(tx),
            },
        );
        rx
    }

    /// Resolve a pending approval by sending through the oneshot channel.
    ///
    /// Returns the approval metadata if found, `None` if not found or already resolved.
    pub async fn resolve_approval(
        &self,
        id: &str,
        approved: bool,
        auto_approve: bool,
    ) -> Option<PendingApproval> {
        let mut approvals = self.inner.pending_approvals.lock().await;
        if let Some(mut slot) = approvals.remove(id) {
            if let Some(tx) = slot.tx.take() {
                let _ = tx.send(ApprovalResult {
                    approved,
                    auto_approve,
                });
            }
            Some(slot.meta)
        } else {
            None
        }
    }

    /// Get metadata for a pending approval (without resolving it).
    pub async fn get_pending_approval(&self, id: &str) -> Option<PendingApproval> {
        self.inner
            .pending_approvals
            .lock()
            .await
            .get(id)
            .map(|slot| slot.meta.clone())
    }

    /// Clear all pending approvals for a session (e.g. when session ends).
    ///
    /// Sends rejection through the oneshot channels so any blocked agent
    /// tasks wake up rather than hanging forever.
    pub async fn clear_session_approvals(&self, session_id: &str) {
        let mut approvals = self.inner.pending_approvals.lock().await;
        let to_remove: Vec<String> = approvals
            .iter()
            .filter(|(_, slot)| slot.meta.session_id.as_deref() == Some(session_id))
            .map(|(id, _)| id.clone())
            .collect();

        for id in to_remove {
            if let Some(mut slot) = approvals.remove(&id) {
                if let Some(tx) = slot.tx.take() {
                    let _ = tx.send(ApprovalResult {
                        approved: false,
                        auto_approve: false,
                    });
                }
            }
        }
    }

    // --- Ask-user (oneshot-based) ---

    /// Add a pending ask-user request.
    ///
    /// Returns a `oneshot::Receiver` that the agent can `.await`.
    pub async fn add_pending_ask_user(
        &self,
        id: String,
        ask_user: PendingAskUser,
    ) -> oneshot::Receiver<AskUserResult> {
        let (tx, rx) = oneshot::channel();
        self.inner.pending_ask_users.lock().await.insert(
            id,
            PendingAskUserSlot {
                meta: ask_user,
                tx: Some(tx),
            },
        );
        rx
    }

    /// Resolve a pending ask-user request.
    pub async fn resolve_ask_user(
        &self,
        id: &str,
        answers: Option<serde_json::Value>,
        cancelled: bool,
    ) -> Option<PendingAskUser> {
        let mut ask_users = self.inner.pending_ask_users.lock().await;
        if let Some(mut slot) = ask_users.remove(id) {
            if let Some(tx) = slot.tx.take() {
                let _ = tx.send(AskUserResult { answers, cancelled });
            }
            Some(slot.meta)
        } else {
            None
        }
    }

    /// Get metadata for a pending ask-user request.
    pub async fn get_pending_ask_user(&self, id: &str) -> Option<PendingAskUser> {
        self.inner
            .pending_ask_users
            .lock()
            .await
            .get(id)
            .map(|slot| slot.meta.clone())
    }

    // --- Plan approval (oneshot-based) ---

    /// Add a pending plan approval request.
    ///
    /// Returns a `oneshot::Receiver` that the agent can `.await` to block
    /// until the plan is approved, rejected, or revised.
    pub async fn add_pending_plan_approval(
        &self,
        id: String,
        plan_approval: PendingPlanApproval,
    ) -> oneshot::Receiver<PlanApprovalResult> {
        let (tx, rx) = oneshot::channel();
        self.inner.pending_plan_approvals.lock().await.insert(
            id,
            PendingPlanApprovalSlot {
                meta: plan_approval,
                tx: Some(tx),
            },
        );
        rx
    }

    /// Resolve a pending plan approval.
    ///
    /// `action` is typically "approve", "reject", or "revise".
    /// `feedback` is optional textual feedback from the user.
    ///
    /// Returns the plan-approval metadata if found, `None` if already resolved.
    pub async fn resolve_plan_approval(
        &self,
        id: &str,
        action: String,
        feedback: String,
    ) -> Option<PendingPlanApproval> {
        let mut plan_approvals = self.inner.pending_plan_approvals.lock().await;
        if let Some(mut slot) = plan_approvals.remove(id) {
            if let Some(tx) = slot.tx.take() {
                let _ = tx.send(PlanApprovalResult { action, feedback });
            }
            Some(slot.meta)
        } else {
            None
        }
    }

    /// Get metadata for a pending plan approval.
    pub async fn get_pending_plan_approval(&self, id: &str) -> Option<PendingPlanApproval> {
        self.inner
            .pending_plan_approvals
            .lock()
            .await
            .get(id)
            .map(|slot| slot.meta.clone())
    }

    /// Clear all pending plan approvals for a session.
    pub async fn clear_session_plan_approvals(&self, session_id: &str) {
        let mut plan_approvals = self.inner.pending_plan_approvals.lock().await;
        let to_remove: Vec<String> = plan_approvals
            .iter()
            .filter(|(_, slot)| slot.meta.session_id.as_deref() == Some(session_id))
            .map(|(id, _)| id.clone())
            .collect();

        for id in to_remove {
            if let Some(mut slot) = plan_approvals.remove(&id) {
                if let Some(tx) = slot.tx.take() {
                    let _ = tx.send(PlanApprovalResult {
                        action: "reject".to_string(),
                        feedback: "Session ended".to_string(),
                    });
                }
            }
        }
    }

    // --- Bridge mode ---

    /// Check if bridge mode is active (TUI owns execution, Web UI mirrors).
    pub async fn is_bridge_mode(&self) -> bool {
        self.inner.bridge.read().await.active
    }

    /// Get the bridge session ID, if bridge mode is active.
    pub async fn bridge_session_id(&self) -> Option<String> {
        let bridge = self.inner.bridge.read().await;
        if bridge.active {
            bridge.session_id.clone()
        } else {
            None
        }
    }

    /// Activate bridge mode for a given session.
    ///
    /// While active, the Web UI should not start its own agent execution
    /// for this session; instead it should route messages to the TUI injector.
    pub async fn set_bridge_session(&self, session_id: String) {
        let mut bridge = self.inner.bridge.write().await;
        bridge.active = true;
        bridge.session_id = Some(session_id);
    }

    /// Deactivate bridge mode.
    pub async fn clear_bridge_session(&self) {
        let mut bridge = self.inner.bridge.write().await;
        bridge.active = false;
        bridge.session_id = None;
    }

    /// Check whether a mutation on a session should be blocked because
    /// the TUI owns it in bridge mode.
    ///
    /// Returns `true` if the session is bridge-owned and should not be
    /// mutated by the web server's own agent executor.
    pub async fn is_bridge_guarded(&self, session_id: &str) -> bool {
        let bridge = self.inner.bridge.read().await;
        bridge.active && bridge.session_id.as_deref() == Some(session_id)
    }

    // --- Injection queues ---

    /// Get or create the injection queue sender for a session.
    ///
    /// Returns `(sender, Option<receiver>)`. The receiver is `Some` only when the
    /// queue was first created -- the caller that creates the session's agent loop
    /// should take the receiver. Subsequent callers get `None` for the receiver.
    pub async fn get_or_create_injection_queue(
        &self,
        session_id: &str,
    ) -> (mpsc::Sender<String>, Option<mpsc::Receiver<String>>) {
        let mut queues = self.inner.injection_queues.lock().await;
        if let Some(tx) = queues.get(session_id) {
            (tx.clone(), None)
        } else {
            let (tx, rx) = mpsc::channel(INJECTION_QUEUE_CAPACITY);
            queues.insert(session_id.to_string(), tx.clone());
            (tx, Some(rx))
        }
    }

    /// Try to inject a message into a running session's queue.
    ///
    /// Returns `Ok(())` on success, `Err(message)` if queue is full or not found.
    pub async fn try_inject_message(
        &self,
        session_id: &str,
        message: String,
    ) -> Result<(), String> {
        let queues = self.inner.injection_queues.lock().await;
        if let Some(tx) = queues.get(session_id) {
            tx.try_send(message)
                .map_err(|e| format!("Injection queue full or closed: {}", e))
        } else {
            Err("No injection queue for session".to_string())
        }
    }

    /// Remove the injection queue for a session.
    pub async fn clear_injection_queue(&self, session_id: &str) {
        self.inner
            .injection_queues
            .lock()
            .await
            .remove(session_id);
    }

    // --- Agent executor ---

    /// Set the agent executor implementation.
    pub async fn set_agent_executor(&self, executor: Arc<dyn AgentExecutor>) {
        *self.inner.agent_executor.lock().await = Some(executor);
    }

    /// Get the agent executor (if set).
    pub async fn agent_executor(&self) -> Option<Arc<dyn AgentExecutor>> {
        self.inner.agent_executor.lock().await.clone()
    }

    // --- Git branch ---

    /// Get the git branch for the working directory.
    pub fn git_branch(&self) -> Option<String> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&self.inner.working_dir)
            .output()
            .ok()?;

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_state() -> AppState {
        let tmp = TempDir::new().unwrap();
        let tmp_path = tmp.into_path();
        let session_manager = SessionManager::new(tmp_path.clone()).unwrap();
        let config = AppConfig::default();
        let user_store = UserStore::new(tmp_path.clone()).unwrap();
        let model_registry = ModelRegistry::new();
        AppState::new(
            session_manager,
            config,
            "/tmp/test".to_string(),
            user_store,
            model_registry,
        )
    }

    #[tokio::test]
    async fn test_mode_default() {
        let state = make_state();
        assert_eq!(state.mode().await, OperationMode::Normal);
    }

    #[tokio::test]
    async fn test_set_mode() {
        let state = make_state();
        state.set_mode(OperationMode::Plan).await;
        assert_eq!(state.mode().await, OperationMode::Plan);
    }

    #[tokio::test]
    async fn test_autonomy_level() {
        let state = make_state();
        assert_eq!(state.autonomy_level().await, "Manual");
        state.set_autonomy_level("Auto".to_string()).await;
        assert_eq!(state.autonomy_level().await, "Auto");
    }

    #[tokio::test]
    async fn test_thinking_level() {
        let state = make_state();
        assert_eq!(state.thinking_level().await, "Medium");
        state.set_thinking_level("High".to_string()).await;
        assert_eq!(state.thinking_level().await, "High");
    }

    #[tokio::test]
    async fn test_interrupt_flag() {
        let state = make_state();
        assert!(!state.is_interrupt_requested().await);
        state.request_interrupt().await;
        assert!(state.is_interrupt_requested().await);
        state.clear_interrupt().await;
        assert!(!state.is_interrupt_requested().await);
    }

    #[tokio::test]
    async fn test_session_running() {
        let state = make_state();
        assert!(!state.is_session_running("s1").await);
        state.set_session_running("s1".to_string()).await;
        assert!(state.is_session_running("s1").await);
        state.set_session_idle("s1").await;
        assert!(!state.is_session_running("s1").await);
    }

    #[tokio::test]
    async fn test_approval_oneshot_lifecycle() {
        let state = make_state();
        let approval = PendingApproval {
            tool_name: "bash".to_string(),
            arguments: serde_json::json!({"command": "ls"}),
            session_id: Some("s1".to_string()),
        };

        // Add approval and get receiver.
        let rx = state
            .add_pending_approval("a1".to_string(), approval)
            .await;

        // Verify pending.
        let pending = state.get_pending_approval("a1").await;
        assert!(pending.is_some());
        assert_eq!(pending.unwrap().tool_name, "bash");

        // Resolve it.
        let resolved = state.resolve_approval("a1", true, false).await;
        assert!(resolved.is_some());

        // Receiver should get the result.
        let result = rx.await.unwrap();
        assert!(result.approved);
        assert!(!result.auto_approve);

        // Second resolve returns None (already consumed).
        assert!(state.resolve_approval("a1", false, false).await.is_none());
    }

    #[tokio::test]
    async fn test_interrupt_denies_pending_approvals() {
        let state = make_state();
        let approval = PendingApproval {
            tool_name: "bash".to_string(),
            arguments: serde_json::json!({}),
            session_id: Some("s1".to_string()),
        };

        let rx = state
            .add_pending_approval("a1".to_string(), approval)
            .await;

        // Interrupt should deny all pending approvals.
        state.request_interrupt().await;

        let result = rx.await.unwrap();
        assert!(!result.approved);
    }

    #[tokio::test]
    async fn test_clear_session_approvals() {
        let state = make_state();

        let approval_s1 = PendingApproval {
            tool_name: "bash".to_string(),
            arguments: serde_json::json!({}),
            session_id: Some("s1".to_string()),
        };
        let approval_s2 = PendingApproval {
            tool_name: "edit".to_string(),
            arguments: serde_json::json!({}),
            session_id: Some("s2".to_string()),
        };

        let rx_s1 = state
            .add_pending_approval("a1".to_string(), approval_s1)
            .await;
        let _rx_s2 = state
            .add_pending_approval("a2".to_string(), approval_s2)
            .await;

        // Clear only s1's approvals.
        state.clear_session_approvals("s1").await;

        // s1 approval should be rejected.
        let result = rx_s1.await.unwrap();
        assert!(!result.approved);

        // s2 approval should still be pending.
        assert!(state.get_pending_approval("a2").await.is_some());
    }

    #[tokio::test]
    async fn test_ask_user_oneshot_lifecycle() {
        let state = make_state();
        let ask = PendingAskUser {
            prompt: "What is your name?".to_string(),
            session_id: Some("s1".to_string()),
        };

        let rx = state
            .add_pending_ask_user("q1".to_string(), ask)
            .await;

        let pending = state.get_pending_ask_user("q1").await;
        assert!(pending.is_some());

        let resolved = state
            .resolve_ask_user("q1", Some(serde_json::json!({"name": "Alice"})), false)
            .await;
        assert!(resolved.is_some());

        let result = rx.await.unwrap();
        assert!(!result.cancelled);
        assert_eq!(
            result.answers.unwrap(),
            serde_json::json!({"name": "Alice"})
        );
    }

    #[tokio::test]
    async fn test_interrupt_cancels_ask_users() {
        let state = make_state();
        let ask = PendingAskUser {
            prompt: "question".to_string(),
            session_id: None,
        };

        let rx = state.add_pending_ask_user("q1".to_string(), ask).await;

        state.request_interrupt().await;

        let result = rx.await.unwrap();
        assert!(result.cancelled);
    }

    #[tokio::test]
    async fn test_plan_approval_oneshot_lifecycle() {
        let state = make_state();
        let plan = PendingPlanApproval {
            data: serde_json::json!({"plan": "do something"}),
            session_id: Some("s1".to_string()),
        };

        let rx = state
            .add_pending_plan_approval("p1".to_string(), plan)
            .await;

        // Verify pending.
        let pending = state.get_pending_plan_approval("p1").await;
        assert!(pending.is_some());

        // Resolve it.
        let resolved = state
            .resolve_plan_approval("p1", "approve".to_string(), "looks good".to_string())
            .await;
        assert!(resolved.is_some());

        // Receiver should get the result.
        let result = rx.await.unwrap();
        assert_eq!(result.action, "approve");
        assert_eq!(result.feedback, "looks good");

        // Second resolve returns None.
        assert!(state
            .resolve_plan_approval("p1", "reject".to_string(), String::new())
            .await
            .is_none());
    }

    #[tokio::test]
    async fn test_interrupt_rejects_plan_approvals() {
        let state = make_state();
        let plan = PendingPlanApproval {
            data: serde_json::json!({"plan": "test"}),
            session_id: None,
        };

        let rx = state
            .add_pending_plan_approval("p1".to_string(), plan)
            .await;

        state.request_interrupt().await;

        let result = rx.await.unwrap();
        assert_eq!(result.action, "reject");
    }

    #[tokio::test]
    async fn test_clear_session_plan_approvals() {
        let state = make_state();

        let plan_s1 = PendingPlanApproval {
            data: serde_json::json!({}),
            session_id: Some("s1".to_string()),
        };
        let plan_s2 = PendingPlanApproval {
            data: serde_json::json!({}),
            session_id: Some("s2".to_string()),
        };

        let rx_s1 = state
            .add_pending_plan_approval("p1".to_string(), plan_s1)
            .await;
        let _rx_s2 = state
            .add_pending_plan_approval("p2".to_string(), plan_s2)
            .await;

        state.clear_session_plan_approvals("s1").await;

        let result = rx_s1.await.unwrap();
        assert_eq!(result.action, "reject");

        // s2 should still be pending.
        assert!(state.get_pending_plan_approval("p2").await.is_some());
    }

    #[tokio::test]
    async fn test_bridge_mode() {
        let state = make_state();

        // Initially not in bridge mode.
        assert!(!state.is_bridge_mode().await);
        assert!(state.bridge_session_id().await.is_none());
        assert!(!state.is_bridge_guarded("s1").await);

        // Activate bridge mode.
        state.set_bridge_session("s1".to_string()).await;
        assert!(state.is_bridge_mode().await);
        assert_eq!(state.bridge_session_id().await.unwrap(), "s1");
        assert!(state.is_bridge_guarded("s1").await);
        assert!(!state.is_bridge_guarded("s2").await);

        // Deactivate.
        state.clear_bridge_session().await;
        assert!(!state.is_bridge_mode().await);
        assert!(!state.is_bridge_guarded("s1").await);
    }

    #[tokio::test]
    async fn test_injection_queue() {
        let state = make_state();

        // First call creates the queue and returns the receiver.
        let (tx, rx) = state.get_or_create_injection_queue("s1").await;
        assert!(rx.is_some());
        let mut rx = rx.unwrap();

        // Second call returns the sender but no new receiver.
        let (tx2, rx2) = state.get_or_create_injection_queue("s1").await;
        assert!(rx2.is_none());

        // Send through either sender.
        tx.try_send("hello".to_string()).unwrap();
        tx2.try_send("world".to_string()).unwrap();

        assert_eq!(rx.recv().await.unwrap(), "hello");
        assert_eq!(rx.recv().await.unwrap(), "world");

        // try_inject_message works too.
        state
            .try_inject_message("s1", "via state".to_string())
            .await
            .unwrap();
        assert_eq!(rx.recv().await.unwrap(), "via state");

        // Clear and verify injection fails.
        state.clear_injection_queue("s1").await;
        assert!(state
            .try_inject_message("s1", "fail".to_string())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_broadcast() {
        let state = make_state();
        let mut rx = state.ws_subscribe();

        state.broadcast(WsBroadcast {
            msg_type: "test".to_string(),
            data: serde_json::json!({"hello": "world"}),
        });

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.msg_type, "test");
    }

    #[tokio::test]
    async fn test_user_store_access() {
        let state = make_state();
        // Verify user store is accessible.
        assert_eq!(state.user_store().count(), 0);
    }

    #[tokio::test]
    async fn test_model_registry_access() {
        let state = make_state();
        let registry = state.model_registry().await;
        // Empty registry by default.
        assert!(registry.providers.is_empty());
    }
}
