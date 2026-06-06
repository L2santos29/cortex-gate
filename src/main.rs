// Cortex Gate — Intelligent AI Gateway
// Main entry point. Runs the gateway server and optionally the Tauri desktop UI.

#[cfg(feature = "desktop")]
fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,cortex_gate=debug".into()),
        )
        .init();

    tracing::info!("Starting Cortex Gate with desktop UI...");

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // TODO: register Tauri commands
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cortex Gate");
}

#[cfg(not(feature = "desktop"))]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,cortex_gate=debug".into()),
        )
        .init();

    tracing::info!("Starting Cortex Gate (headless mode)...");

    let app = cortex_gate::create_app().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:18801")
        .await
        .expect("Failed to bind to address");

    tracing::info!("Cortex Gate listening on http://127.0.0.1:18801");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
