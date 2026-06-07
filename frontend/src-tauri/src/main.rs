// Cortex Gate — Tauri Desktop Entry Point
//
// Prevents a console window on Windows in release and sets up tracing.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,cortex_gate_tauri=debug".into()),
        )
        .init();

    cortex_gate_tauri::run();
}
