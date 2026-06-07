// Cortex Gate — Intelligent AI Gateway
//
// Entry point. Initializes tracing, builds the application router,
// and starts the HTTP server on 127.0.0.1:18801.
//
// ## Usage
// ```bash
// cargo run
// # or with custom config
// CORTEX_PORT=18801 CORTEX_API_KEY=sk-my-key cargo run
// ```

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,cortex_gate=debug".into()),
        )
        .init();

    tracing::info!(
        target: "cortex_gate::main",
        "Cortex Gate v{} starting...",
        env!("CARGO_PKG_VERSION"),
    );

    let app = cortex_gate::create_app().await;

    let addr = "127.0.0.1:18801";
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener — is port 18801 already in use?");

    tracing::info!(
        target: "cortex_gate::main",
        "Cortex Gate listening on http://{}",
        addr,
    );

    axum::serve(listener, app)
        .await
        .expect("Server exited with error");
}
