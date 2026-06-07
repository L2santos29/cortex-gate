// Gateway module — HTTP server, routing engine, API endpoints.
//
// Proporciona:
// - Servidor HTTP con Axum 0.8
// - Endpoints compatibles con OpenAI Chat Completions API
// - API de administración para configuración y monitorización
// - Autenticación de cliente (x-api-key / Bearer)
// - Autenticación de administrador (X-Admin-Token)
// - Middleware CORS para integración con cualquier frontend
// - Multi-provider proxy engine (OpenAI, Anthropic, OpenRouter, Custom)
// - SSE streaming adapter con backpressure
//
// ## Módulos del proxy
// - [`providers`]  — ProxyEngine con forwarding a upstreams
// - [`streaming`]  — SSE streaming adapters (OpenAI/OpenRouter/Anthropic)
//
// ## Módulos del servidor
// - [`server`] — Estado compartido, inicialización, router builder
// - [`auth`]   — Autenticación y autorización
// - [`routes`] — Handlers de endpoints HTTP

pub mod auth;
pub mod providers;
pub mod routes;
pub mod server;
pub mod streaming;

pub use server::build_router;
