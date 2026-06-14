// Cortex Gate — Gateway Server
//
// Shared application state (`AppState`), initialization from env +
// config file, and the router builder with middleware stack
// (CORS, tracing, body limit).

use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Instant;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::{get, post};
use axum::routing::get_service;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::extensions::ExtensionManager;
use crate::gateway::routes;
use crate::governance::Database;
use crate::models::config::CortexConfig;

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub http_client: reqwest::Client,
    pub db: Arc<Database>,
    pub classifier: Option<()>,
    pub config: CortexConfig,
    pub uptime: Instant,
    pub extensions: Arc<Mutex<ExtensionManager>>,
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

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
        "Cortex Gate initialized",
    );

    Ok(Arc::new(AppState {
        http_client,
        db,
        classifier: None,
        config,
        uptime: Instant::now(),
        extensions: Arc::new(Mutex::new(ExtensionManager::new())),
    }))
}

// ---------------------------------------------------------------------------
// Router builder
// ---------------------------------------------------------------------------

pub async fn build_router(state: Arc<AppState>, ext_manager: ExtensionManager) -> Router {
    // Store extension manager in state
    {
        let mut ext = state.extensions.lock().await;
        *ext = ext_manager;
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut router = Router::new()
        .route("/health", get(routes::health))
        .route("/v1/models", get(routes::models))
        .route("/v1/chat/completions", post(routes::chat_completions))
        .route("/admin/config", get(routes::admin_config_get).post(routes::admin_config_post))
        .route("/extensions", get(routes::extensions_list))
        .route("/extensions/:id/enable", post(routes::extension_enable))
        .route("/extensions/:id/disable", post(routes::extension_disable));

    let router = router
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .fallback(get_service(
            ServeDir::new("frontend/dist")
                .not_found_service(get_service(
                    ServeDir::new("frontend/dist").append_index_html_on_directories(true)
                ))
        ))
        .layer(axum::extract::DefaultBodyLimit::max(2_000_000))
        .with_state(state.clone());

    // Add extension routes (MethodRouter<()>, no state needed)
    let ext_routes = state.extensions.lock().await.collect_routes();
    let mut sealed = router;
    for (path, method_router) in ext_routes {
        sealed = sealed.route(&path, method_router);
    }
    sealed
}
