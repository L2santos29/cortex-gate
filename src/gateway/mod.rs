// Gateway module — HTTP server, routing engine, API endpoints.
//
// Provides:
// - OpenAI-compatible /v1/chat/completions endpoint
// - Admin API for configuration and monitoring
// - Request authentication (admin + client tokens)
// - SSE streaming proxy to upstream providers
// - Multi-provider support (OpenRouter, Anthropic, OpenAI, etc.)

use axum::Router;
use crate::governance::Database;

pub mod auth;
pub mod handlers;
pub mod routes;
pub mod streaming;
pub mod middleware;

/// Build the gateway router with all middleware and routes.
pub fn build_router(db: Database) -> Router {
    Router::new()
        // TODO: register routes
        .with_state(db)
}
