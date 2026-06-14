// Cortex Gate — Library root
//
// Re-exporta los módulos principales y proporciona el constructor
// de la aplicación (`create_app`).
//
// ## Módulos
// - `gateway`     — Servidor HTTP, autenticación, rutas
// - `governance`  — Control de costes, cuotas, base de datos
// - `classifier`  — Clasificación de prompts por embeddings ONNX
// - `models`      — Tipos de datos compartidos, configuración
// - `benchmark`   — Benchmarking autónomo de modelos
// - `tools`       — Utilidades auxiliares

pub mod benchmark;
pub mod classifier;
pub mod gateway;
pub mod governance;
pub mod models;
pub mod tools;

use axum::Router;

/// Construye el router de la aplicación Cortex Gate.
///
/// Inicializa el estado compartido (config, DB, HTTP client) y
/// construye el router con todas las rutas y middleware.
///
/// ## Ejemplo
/// ```no_run
/// use cortex_gate::create_app;
///
/// #[tokio::main]
/// async fn main() -> Result<(), anyhow::Error> {
///     let app = create_app().await?;
///     let listener = tokio::net::TcpListener::bind("127.0.0.1:18801").await?;
///     axum::serve(listener, app).await?;
///     Ok(())
/// }
/// ```
pub async fn create_app() -> anyhow::Result<Router> {
    let state = gateway::server::init_app_state().await?;
    Ok(gateway::server::build_router(state))
}
