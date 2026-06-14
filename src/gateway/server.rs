// Cortex Gate — Gateway Server
//
// Shared application state (`AppState`), initialization from env +
// config file, and the router builder with middleware stack
// (CORS, tracing, body limit).

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::gateway::routes;
use crate::governance::Database;
use crate::models::config::CortexConfig;

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Shared application state.
///
/// Constructed once in [`init_app_state`] and injected into each Axum handler
/// via `State<Arc<AppState>>`.
#[derive(Clone)]
pub struct AppState {
    /// Reusable HTTP client for all upstream provider calls.
    pub http_client: reqwest::Client,
    /// SQLite database for usage tracking, quotas, users, and persistent config.
    pub db: Arc<Database>,
    /// ONNX embedding classifier (once implemented).
    pub classifier: Option<()>,
    /// Configuration loaded from env + config.json.
    pub config: CortexConfig,
    /// Timestamp on creation (for uptime reporting).
    pub uptime: Instant,
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

/// Initialize the application state.
///
/// Load order:
/// 1. [`CortexConfig`] defaults
/// 2. `config.json` in the current directory
/// 3. Environment variables (highest precedence)
///
/// Then creates the HTTP client, opens the SQLite database, and wraps
/// everything in `Arc<AppState>`.
pub async fn init_app_state() -> Result<Arc<AppState>> {
    let config = CortexConfig::load();

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .connect_timeout(std::time::Duration::from_secs(10))
        .user_agent(concat!("cortex-gate/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Failed to build HTTP client")?;

    tracing::info!(
        target: "cortex_gate::gateway::server",
        "Opening database at: {}",
        config.db_path
    );

    let db = Arc::new(
        Database::open_or_create(&config.db_path)
            .await
            .context("Failed to initialize database")?,
    );

    tracing::info!(
        target: "cortex_gate::gateway::server",
        "Cortex Gate initialized — {} provider(s) registered",
        config.providers.len(),
    );

    Ok(Arc::new(AppState {
        http_client,
        db,
        classifier: None,
        config,
        uptime: Instant::now(),
    }))
}

// ---------------------------------------------------------------------------
// Router builder
// ---------------------------------------------------------------------------

/// Build the complete router with all routes and middleware.
///
/// Middleware stack (outer → inner):
/// 1. TraceLayer — request/response logging
/// 2. CORS — allow any origin/method/header
/// 3. DefaultBodyLimit — 2 MB max body size
///
/// ## Routes
/// | Method | Path                  | Handler               | Auth        |
/// |--------|-----------------------|-----------------------|-------------|
/// | GET    | `/health`             | `routes::health`      | None        |
/// | GET    | `/v1/models`          | `routes::models`      | None        |
/// | POST   | `/v1/chat/completions`| `routes::chat_completions` | API Key |
/// | GET    | `/admin/config`       | `routes::admin_config_get` | Admin  |
/// | POST   | `/admin/config`       | `routes::admin_config_post`| Admin  |
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", axum::routing::get(routes::health))
        .route("/v1/models", axum::routing::get(routes::models))
        .route(
            "/v1/chat/completions",
            axum::routing::post(routes::chat_completions),
        )
        .route(
            "/admin/config",
            axum::routing::get(routes::admin_config_get)
                .post(routes::admin_config_post),
        )
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(axum::extract::DefaultBodyLimit::max(2_000_000))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use serde_json::json;
    use tower::ServiceExt;

    /// Build a minimal AppState for unit tests.
    async fn test_app_state() -> Arc<AppState> {
        let config = CortexConfig {
            port: 0,
            host: "127.0.0.1".to_string(),
            db_path: ":memory:".to_string(),
            ..Default::default()
        };

        let http_client = reqwest::Client::new();
        let db = Arc::new(
            Database::open_or_create(":memory:")
                .await
                .expect("test db"),
        );

        Arc::new(AppState {
            http_client,
            db,
            classifier: None,
            config,
            uptime: Instant::now(),
        })
    }

    fn test_app(state: Arc<AppState>) -> Router {
        build_router(state)
    }

    // -- Health -----------------------------------------------------------

    #[tokio::test]
    async fn health_returns_200() {
        let state = test_app_state().await;
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // -- Models -----------------------------------------------------------

    #[tokio::test]
    async fn models_returns_200() {
        let state = test_app_state().await;
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // -- Chat Completions --------------------------------------------------

    #[tokio::test]
    async fn chat_completions_no_auth_returns_401() {
        let state = test_app_state().await;
        let app = test_app(state);

        let body = serde_json::to_vec(&json!({
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn chat_completions_with_auth_returns_200() {
        let state = test_app_state().await;
        let client_api_key = state.config.client_api_key.clone();
        let app = test_app(state);

        let body = serde_json::to_vec(&json!({
            "model": "gpt-4o-mini",
            "messages": [{"role": "user", "content": "Hello"}]
        }))
        .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .header("x-api-key", client_api_key.as_str())
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // -- Admin Config ------------------------------------------------------

    #[tokio::test]
    async fn admin_config_no_auth_returns_403() {
        let state = test_app_state().await;
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/admin/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn admin_config_with_auth_returns_200() {
        let state = test_app_state().await;
        let admin_token = state.config.admin_token.clone();
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/admin/config")
                    .header("x-admin-token", admin_token.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // -- CORS headers ------------------------------------------------------

    #[tokio::test]
    async fn cors_headers_present() {
        let state = test_app_state().await;
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/health")
                    .header("origin", "http://example.com")
                    .header("access-control-request-method", "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response
            .headers()
            .get("access-control-allow-origin")
            .is_some());
    }
}
