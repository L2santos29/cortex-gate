// Cortex Gate — Library root
// Re-exports modules and provides the application builder.

pub mod benchmark;
pub mod classifier;
pub mod gateway;
pub mod governance;
pub mod models;
pub mod tools;

use axum::Router;

/// Build the Cortex Gate application router.
pub async fn create_app() -> Router {
    // Initialize SQLite database
    let db = governance::Database::open_or_create("cortex-gate.db")
        .await
        .expect("Failed to initialize database");

    // Build the gateway router with all middleware
    gateway::build_router(db)
}
