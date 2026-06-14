// Cortex Gate — Tauri Commands + Server Controller
//
// Proxies requests to the Cortex Gate HTTP API and provides
// server start/stop/status controls from the desktop UI.

use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Gateway client
// ---------------------------------------------------------------------------

const GATEWAY_BASE: &str = "http://127.0.0.1:18801";

struct GatewayClient {
    client: reqwest::Client,
}

impl GatewayClient {
    fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("reqwest Client should build"),
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}/api{}", GATEWAY_BASE, path);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Gateway connection failed: {e}"))?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Gateway error: {text}"));
        }
        resp.json()
            .await
            .map_err(|e| format!("Failed to parse gateway response: {e}"))
    }

    async fn post_void(&self, path: &str, body: &impl serde::Serialize) -> Result<(), String> {
        let url = format!("{}/api{}", GATEWAY_BASE, path);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Gateway connection failed: {e}"))?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Gateway error: {text}"));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Backend process state
// ---------------------------------------------------------------------------

struct BackendProcess {
    child: Option<Child>,
}

impl BackendProcess {
    fn new() -> Self {
        Self { child: None }
    }

    fn backend_binary_path() -> String {
        // The Tauri binary lives in frontend/src-tauri/target/release/
        // The backend binary lives in target/release/ (project root)
        // So we go up 3 levels from the Tauri binary
        let exe = std::env::current_exe().ok();
        if let Some(path) = exe {
            // e.g. .../frontend/src-tauri/target/release/cortex-gate-tauri
            // We want .../target/release/cortex-gate
            if let Some(parent) = path.parent() {
                // up from release/
                if let Some(grandparent) = parent.parent() {
                    // up from target/
                    if let Some(great_grandparent) = grandparent.parent() {
                        // up from src-tauri/
                        if let Some(root) = great_grandparent.parent() {
                            // root is frontend/
                            if let Some(project_root) = root.parent() {
                                // project root
                                let backend = project_root.join("target").join("release").join("cortex-gate");
                                if backend.exists() {
                                    return backend.to_string_lossy().to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
        // Fallback: try relative path from CWD
        "../target/release/cortex-gate".to_string()
    }
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionConfig {
    pub key: String,
    pub label: String,
    pub weight: f32,
    pub intensity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcualizadorConfig {
    pub dimensions: Vec<DimensionConfig>,
    pub economy: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub tokens_today: u64,
    pub cost_today: f64,
    pub requests_today: u64,
    pub active_models: u32,
    pub models: Vec<ModelStat>,
    pub usage_by_user: Vec<UserUsageRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStat {
    pub model: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUsageRow {
    pub user_id: String,
    pub name: Option<String>,
    pub tokens: u64,
    pub cost: f64,
    pub requests: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub user_id: String,
    pub period: String,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub entries: Vec<UsageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub timestamp: String,
    pub model: String,
    pub provider: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost: f64,
}

// ---------------------------------------------------------------------------
// In-memory state (fallback when gateway is unreachable)
// ---------------------------------------------------------------------------

struct AppState {
    dimensions: Vec<DimensionConfig>,
    economy: f32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            dimensions: vec![
                DimensionConfig { key: "reasoning".into(),  label: "Reasoning".into(),  weight: 0.125, intensity: 0.0 },
                DimensionConfig { key: "code".into(),       label: "Code".into(),       weight: 0.125, intensity: 0.0 },
                DimensionConfig { key: "creativity".into(), label: "Creativity".into(), weight: 0.125, intensity: 0.0 },
                DimensionConfig { key: "math".into(),       label: "Math".into(),       weight: 0.125, intensity: 0.0 },
                DimensionConfig { key: "precision".into(),  label: "Precision".into(),  weight: 0.125, intensity: 0.0 },
                DimensionConfig { key: "speed".into(),      label: "Speed".into(),      weight: 0.125, intensity: 0.0 },
                DimensionConfig { key: "context".into(),    label: "Context".into(),    weight: 0.125, intensity: 0.0 },
                DimensionConfig { key: "safety".into(),     label: "Safety".into(),     weight: 0.125, intensity: 0.0 },
            ],
            economy: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Server Control Commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn start_backend(
    state: tauri::State<'_, Mutex<BackendProcess>>,
) -> Result<String, String> {
    // Check if port is already in use
    match reqwest::get("http://127.0.0.1:18801/health").await {
        Ok(resp) if resp.status().is_success() => {
            return Err("Backend is already running on port 18801".into());
        }
        _ => {} // port is free, proceed
    }

    let mut proc = state.lock().map_err(|e| e.to_string())?;
    if proc.child.is_some() {
        return Err("Backend server is already running".into());
    }

    let backend_path = BackendProcess::backend_binary_path();
    tracing::info!(target: "cortex_gate_tauri", "Starting backend: {}", backend_path);

    let child = Command::new(&backend_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start backend: {e}"))?;

    let pid = child.id();
    proc.child = Some(child);

    tracing::info!(target: "cortex_gate_tauri", "Backend started (PID: {})", pid);
    Ok(format!("Backend started (PID: {pid})"))
}

#[tauri::command]
async fn stop_backend(
    state: tauri::State<'_, Mutex<BackendProcess>>,
) -> Result<String, String> {
    // Scope the lock to avoid holding non-Send MutexGuard across .await
    {
        let mut proc = state.lock().map_err(|e| e.to_string())?;
        // 1. Try tracked child process first
        if let Some(mut child) = proc.child.take() {
            let pid = child.id();
            let _ = child.kill();
            let _ = child.wait();
            tracing::info!(target: "cortex_gate_tauri", "Backend stopped (PID: {})", pid);
            return Ok(format!("Backend stopped (PID: {pid})"));
        }
    } // lock released here

    // 2. No tracked child — try to kill whatever is on port 18801
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("fuser -k 18801/tcp 2>/dev/null || lsof -ti:18801 | xargs kill -9 2>/dev/null || true")
        .output()
        .map_err(|e| format!("Failed to kill process on port 18801: {e}"))?;

    if output.status.success() {
        tracing::info!(target: "cortex_gate_tauri", "Backend stopped via port kill");
        // Check that it's actually dead
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match reqwest::get("http://127.0.0.1:18801/health").await {
            Ok(_) => Err("Failed to stop backend — process still responding on port 18801".into()),
            Err(_) => Ok("Backend stopped (found and killed on port 18801)".to_string()),
        }
    } else {
        Err("No backend server is running on port 18801".into())
    }
}

#[tauri::command]
async fn get_backend_status(
    state: tauri::State<'_, Mutex<BackendProcess>>,
) -> Result<serde_json::Value, String> {
    let running = {
        let proc = state.lock().map_err(|e| e.to_string())?;
        proc.child.is_some()
    };

    // Check if the port is actually responding
    let healthy = if running {
        match reqwest::get("http://127.0.0.1:18801/health").await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    } else {
        false
    };

    Ok(serde_json::json!({
        "running": running,
        "healthy": healthy,
    }))
}

#[tauri::command]
async fn open_web_ui() -> Result<(), String> {
    let url = "http://127.0.0.1:18801";
    open::that(url).map_err(|e| format!("Failed to open browser: {e}"))
}

// ---------------------------------------------------------------------------
// Gateway API Commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_ecualizador() -> Result<EcualizadorConfig, String> {
    let client = GatewayClient::new();
    match client.get::<EcualizadorConfig>("/ecualizador").await {
        Ok(config) => Ok(config),
        Err(_) => {
            let s = AppState::default();
            Ok(EcualizadorConfig { dimensions: s.dimensions, economy: s.economy })
        }
    }
}

#[tauri::command]
async fn set_dimension_weight(dim: String, weight: f32) -> Result<(), String> {
    let weight = weight.clamp(0.0, 1.0);
    let client = GatewayClient::new();
    let body = serde_json::json!({ "dim": &dim, "weight": weight });
    client.post_void("/ecualizador/dimension", &body).await
}

#[tauri::command]
async fn set_economy(level: f32) -> Result<(), String> {
    let level = level.clamp(0.0, 1.0);
    let client = GatewayClient::new();
    let body = serde_json::json!({ "level": level });
    client.post_void("/ecualizador/economy", &body).await
}

#[tauri::command]
async fn get_dashboard_stats() -> Result<DashboardData, String> {
    let client = GatewayClient::new();
    match client.get::<DashboardData>("/dashboard/stats").await {
        Ok(data) => Ok(data),
        Err(e) => {
            tracing::warn!("Failed to fetch dashboard stats: {e}");
            Ok(DashboardData {
                tokens_today: 0,
                cost_today: 0.0,
                requests_today: 0,
                active_models: 0,
                models: vec![],
                usage_by_user: vec![],
            })
        }
    }
}

#[tauri::command]
async fn get_usage_report(user_id: String) -> Result<UsageReport, String> {
    let client = GatewayClient::new();
    let path = format!("/dashboard/usage/{}", urlencoding(&user_id));
    match client.get::<UsageReport>(&path).await {
        Ok(report) => Ok(report),
        Err(e) => {
            tracing::warn!("Failed to fetch usage report for {user_id}: {e}");
            Err(format!("Could not load report: {e}"))
        }
    }
}

#[tauri::command]
async fn get_config() -> Result<serde_json::Value, String> {
    let client = GatewayClient::new();
    client.get::<serde_json::Value>("/config").await
}

#[tauri::command]
async fn add_provider(name: String, base_url: String, provider_type: String) -> Result<(), String> {
    let client = GatewayClient::new();
    let body = serde_json::json!({"name": name, "base_url": base_url, "provider_type": provider_type});
    client.post_void("/config/providers", &body).await
}

#[tauri::command]
async fn remove_provider(index: usize) -> Result<(), String> {
    let client = GatewayClient::new();
    client.post_void(&format!("/config/providers/{index}/remove"), &serde_json::json!({})).await
}

#[tauri::command]
async fn get_api_keys() -> Result<serde_json::Value, String> {
    let client = GatewayClient::new();
    client.get::<serde_json::Value>("/config/api-keys").await
}

#[tauri::command]
async fn add_api_key(provider: String, name: String, key: String) -> Result<(), String> {
    let client = GatewayClient::new();
    let body = serde_json::json!({"provider": provider, "name": name, "key": key});
    client.post_void("/config/api-keys", &body).await
}

#[tauri::command]
async fn remove_api_key(id: String) -> Result<(), String> {
    let client = GatewayClient::new();
    client.post_void(&format!("/config/api-keys/{id}/remove"), &serde_json::json!({})).await
}

#[tauri::command]
async fn get_budgets() -> Result<serde_json::Value, String> {
    let client = GatewayClient::new();
    client.get::<serde_json::Value>("/config/budgets").await
}

#[tauri::command]
async fn set_budget(
    user_id: String, tokens_per_hour: u64, tokens_per_day: u64, tokens_per_month: u64,
) -> Result<(), String> {
    let client = GatewayClient::new();
    let body = serde_json::json!({
        "user_id": user_id, "tokens_per_hour": tokens_per_hour,
        "tokens_per_day": tokens_per_day, "tokens_per_month": tokens_per_month,
    });
    client.post_void("/config/budgets", &body).await
}

#[tauri::command]
async fn remove_budget(id: String) -> Result<(), String> {
    let client = GatewayClient::new();
    client.post_void(&format!("/config/budgets/{id}/remove"), &serde_json::json!({})).await
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn urlencoding(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Plugin registration
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(BackendProcess::new()))
        .invoke_handler(tauri::generate_handler![
            // Server control
            start_backend,
            stop_backend,
            get_backend_status,
            open_web_ui,
            // Gateway API
            get_ecualizador,
            set_dimension_weight,
            set_economy,
            get_dashboard_stats,
            get_usage_report,
            get_config,
            add_provider,
            remove_provider,
            get_api_keys,
            add_api_key,
            remove_api_key,
            get_budgets,
            set_budget,
            remove_budget,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cortex Gate Tauri UI");
}
