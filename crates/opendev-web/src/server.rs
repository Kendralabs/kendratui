//! Axum application builder and server startup.

use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum::http::header;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::info;

use crate::routes;
use crate::state::AppState;
use crate::websocket::ws_handler;

/// Build the Axum application router.
pub fn build_app(state: AppState, static_dir: Option<&Path>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:5173".parse().unwrap(),
            "http://localhost:3000".parse().unwrap(),
        ])
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            header::COOKIE,
        ])
        .allow_credentials(true);

    let mut app = Router::new()
        // API routes
        .merge(routes::auth::router())
        .merge(routes::config::router())
        .merge(routes::sessions::router())
        .merge(routes::chat::router())
        .merge(routes::mcp::router())
        .merge(routes::commands::router())
        // Health check
        .route("/api/health", axum::routing::get(health_check))
        // WebSocket
        .route("/ws", axum::routing::get(ws_handler))
        .layer(cors)
        .with_state(state);

    // Serve static files if the directory exists.
    if let Some(dir) = static_dir {
        if dir.exists() {
            let assets_dir = dir.join("assets");
            if assets_dir.exists() {
                app = app.nest_service("/assets", ServeDir::new(assets_dir));
            }
            // SPA fallback: serve index.html for all unmatched paths.
            app = app.fallback_service(ServeDir::new(dir));
        }
    }

    app
}

/// Health check endpoint.
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "opendev-web-ui",
    }))
}

/// Start the web server.
pub async fn start_server(
    state: AppState,
    host: &str,
    port: u16,
    static_dir: Option<&Path>,
) -> std::io::Result<()> {
    let app = build_app(state, static_dir);

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    info!("Starting web server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use opendev_config::ModelRegistry;
    use opendev_history::SessionManager;
    use opendev_http::UserStore;
    use opendev_models::AppConfig;
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn make_state() -> AppState {
        let tmp = TempDir::new().unwrap();
        let tmp_path = tmp.into_path();
        let session_manager = SessionManager::new(tmp_path.clone()).unwrap();
        let config = AppConfig::default();
        let user_store = UserStore::new(tmp_path).unwrap();
        let model_registry = ModelRegistry::new();
        AppState::new(session_manager, config, "/tmp/test".to_string(), user_store, model_registry)
    }

    #[tokio::test]
    async fn test_health_check() {
        let state = make_state();
        let app = build_app(state, None);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }
}
