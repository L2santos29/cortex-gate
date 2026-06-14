// Cortex Gate — Library root
//
// Re-exports main modules and provides the application constructor.

pub mod benchmark;
pub mod classifier;
pub mod extensions;
pub mod gateway;
pub mod governance;
pub mod models;
pub mod tools;

use std::sync::Arc;

use axum::Router;
use extensions::{EventBus, ExtensionContext};

/// Build the Cortex Gate application router.
///
/// Initializes shared state, the extension system, and builds the
/// router with all routes and middleware.
pub async fn create_app() -> anyhow::Result<Router> {
    let state = gateway::server::init_app_state().await?;

    // Initialize extension system
    let event_bus = EventBus::new();
    let mut ext_manager = extensions::ExtensionManager::new();

    // Register built-in extensions here as they are created

    // Create extension context
    let ctx = ExtensionContext::new("system", Arc::new(event_bus.clone()))
        .with_db(state.db.clone())
        .with_http_client(state.http_client.clone());

    // Initialize all extensions
    if let Err(errors) = ext_manager.init_all(&ctx).await {
        for (id, err) in &errors {
            tracing::error!("Extension '{}' failed to init: {}", id, err);
        }
    }

    Ok(gateway::server::build_router(state, ext_manager).await)
}
