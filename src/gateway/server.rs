// Cortex Gate — Gateway Server
//
// Punto de entrada del servidor HTTP del gateway. Define el estado
// compartido de la aplicación (`AppState`), la inicialización desde
// entorno + archivo de configuración, y la construcción del router
// con todos los middleware (CORS, logging, etc.).

use std::sync::Arc;
use std::time::Instant;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};

use crate::gateway::routes;
use crate::governance::Database;
use crate::models::config::CortexConfig;

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Estado compartido de la aplicación.
///
///
/// Se construye una vez en [`init_app_state`] y se inyecta en cada
/// handler de Axum vía `State<Arc<AppState>>`.
pub struct AppState {
    /// Cliente HTTP reutilizable para todas las llamadas a proveedores
    /// upstream (OpenAI, Anthropic, OpenRouter, etc.).
    pub http_client: reqwest::Client,

    /// Base de datos SQLite para tracking de uso, cuotas, usuarios y
    /// configuración persistente.
    pub db: Database,

    /// TODO: Clasificador de embeddings ONNX.
    ///
    /// Una vez que el módulo `crate::classifier` esté completo, este
    /// campo se reemplazará por:
    ///   `pub classifier: Option<crate::classifier::Classifier>`
    pub classifier: Option<()>,

    /// Configuración cargada desde variables de entorno + `config.json`.
    pub config: CortexConfig,

    /// Instante de creación del estado (para calcular uptime).
    pub uptime: Instant,
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

/// Inicializa el estado de la aplicación.
///
/// Orden de carga:
/// 1. Valores por defecto de [`CortexConfig`]
/// 2. Archivo `config.json` en el directorio actual
/// 3. Variables de entorno (precedencia máxima)
///
/// Luego crea el cliente HTTP, abre la base de datos SQLite, y envuelve
/// todo en un `Arc` para compartir entre handlers.
pub async fn init_app_state() -> Arc<AppState> {
    let config = CortexConfig::load();

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .connect_timeout(std::time::Duration::from_secs(10))
        .user_agent(concat!("cortex-gate/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("Failed to build HTTP client");

    tracing::info!(
        target: "cortex_gate::gateway::server",
        "Opening database at: {}",
        config.db_path
    );

    let db = Database::open_or_create(&config.db_path)
        .await
        .expect("Failed to initialize database");

    tracing::info!(
        target: "cortex_gate::gateway::server",
        "Cortex Gate initialized — {} provider(s) registered",
        config.providers.len(),
    );

    Arc::new(AppState {
        http_client,
        db,
        classifier: None, // TODO: wire up ONNX classifier
        config,
        uptime: Instant::now(),
    })
}

// ---------------------------------------------------------------------------
// Router builder
// ---------------------------------------------------------------------------

/// Construye el router completo con todas las rutas y middleware.
///
/// Middleware aplicado (de fuera hacia dentro):
/// 1. CORS (permite cualquier origen/método/header)
/// 2. Tracing / logging (cortesía de Axum + tower-http)
///
/// ## Rutas
/// | Método | Path                  | Handler               | Auth        |
/// |--------|-----------------------|-----------------------|-------------|
/// | GET    | `/health`             | `routes::health`      | Ninguna     |
/// | GET    | `/v1/models`          | `routes::models`      | Ninguna     |
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
        .layer(cors)
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
    use tower::ServiceExt; // for oneshot

    /// Construye un AppState mínimo para tests unitarios.
    async fn test_app_state() -> Arc<AppState> {
        let config = CortexConfig {
            port: 0,       // no binding in tests
            host: "127.0.0.1".to_string(),
            db_path: ":memory:".to_string(),
            ..Default::default()
        };

        let http_client = reqwest::Client::new();
        let db = Database::open_or_create(":memory:")
            .await
            .expect("test db");

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

        let client_api_key = &client_api_key;

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

        // CORS preflight should include allow-origin
        assert!(response
            .headers()
            .get("access-control-allow-origin")
            .is_some());
    }
}
