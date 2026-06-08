// Cortex Gate — Tauri Commands
//
// These commands are called from the frontend via `invoke()` and proxy
// requests to the Cortex Gate HTTP API at http://127.0.0.1:18801.
//
// When the gateway is not running, commands return sensible defaults
// so the UI remains functional for configuration.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Gateway client
// ---------------------------------------------------------------------------

const GATEWAY_BASE: &str = "http://127.0.0.1:18801";

/// A lightweight HTTP client for calling the gateway API.
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

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, String> {
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

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T, String> {
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

        resp.json()
            .await
            .map_err(|e| format!("Failed to parse gateway response: {e}"))
    }

    /// POST without expecting a meaningful JSON response body.
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
// Shared types
// ---------------------------------------------------------------------------

/// A single dimension with its configurable weight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionConfig {
    pub key: String,
    pub label: String,
    pub weight: f32,
    pub intensity: f32,
}

/// The full ecualizador configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcualizadorConfig {
    pub dimensions: Vec<DimensionConfig>,
    pub economy: f32,
}

/// Dashboard statistics returned by the gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub tokens_today: u64,
    pub cost_today: f64,
    pub requests_today: u64,
    pub active_models: u32,
    pub models: Vec<ModelStat>,
    pub usage_by_user: Vec<UserUsageRow>,
}

/// Per-model token stats for the bar chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStat {
    pub model: String,
    pub tokens: u64,
}

/// Per-user usage row for the table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUsageRow {
    pub user_id: String,
    pub name: Option<String>,
    pub tokens: u64,
    pub cost: f64,
    pub requests: u64,
}

/// Detailed usage report for a single user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub user_id: String,
    pub period: String,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub entries: Vec<UsageEntry>,
}

/// A single usage log entry.
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
// Tauri commands
// ---------------------------------------------------------------------------

/// Retrieve the current ecualizador configuration (dimension weights + economy).
#[tauri::command]
async fn get_ecualizador() -> Result<EcualizadorConfig, String> {
    let client = GatewayClient::new();
    match client.get::<EcualizadorConfig>("/ecualizador").await {
        Ok(config) => Ok(config),
        Err(_) => {
            // Fallback to defaults if gateway is unreachable
            let state = AppState::default();
            Ok(EcualizadorConfig {
                dimensions: state.dimensions,
                economy: state.economy,
            })
        }
    }
}

/// Set the weight of a single dimension.
#[tauri::command]
async fn set_dimension_weight(dim: String, weight: f32) -> Result<(), String> {
    let weight = weight.clamp(0.0, 1.0);
    let client = GatewayClient::new();
    let body = serde_json::json!({ "dim": &dim, "weight": weight });
    client.post_void("/ecualizador/dimension", &body).await
}

/// Set the global economy level.
#[tauri::command]
async fn set_economy(level: f32) -> Result<(), String> {
    let level = level.clamp(0.0, 1.0);
    let client = GatewayClient::new();
    let body = serde_json::json!({ "level": level });
    client.post_void("/ecualizador/economy", &body).await
}

/// Get dashboard statistics (tokens, cost, requests, per-model breakdown).
#[tauri::command]
async fn get_dashboard_stats() -> Result<DashboardData, String> {
    let client = GatewayClient::new();
    match client.get::<DashboardData>("/dashboard/stats").await {
        Ok(data) => Ok(data),
        Err(e) => {
            tracing::warn!("Failed to fetch dashboard stats: {e}");
            // Return zeroed data so the UI doesn't break
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

/// Get a detailed usage report for a specific user.
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

/// Retrieve the full gateway config (providers, etc.).
#[tauri::command]
async fn get_config() -> Result<serde_json::Value, String> {
    let client = GatewayClient::new();
    client.get::<serde_json::Value>("/config").await
}

/// Add a new provider.
#[tauri::command]
async fn add_provider(name: String, base_url: String, provider_type: String) -> Result<(), String> {
    let client = GatewayClient::new();
    let body = serde_json::json!({
        "name": name,
        "base_url": base_url,
        "provider_type": provider_type,
    });
    client.post_void("/config/providers", &body).await
}

/// Remove a provider by index.
#[tauri::command]
async fn remove_provider(index: usize) -> Result<(), String> {
    let client = GatewayClient::new();
    client
        .post_void(
            &format!("/config/providers/{index}/remove"),
            &serde_json::json!({}),
        )
        .await
}

/// Get all stored API keys (previews only, never plaintext).
#[tauri::command]
async fn get_api_keys() -> Result<serde_json::Value, String> {
    let client = GatewayClient::new();
    client.get::<serde_json::Value>("/config/api-keys").await
}

/// Store a new API key for a provider.
#[tauri::command]
async fn add_api_key(provider: String, name: String, key: String) -> Result<(), String> {
    let client = GatewayClient::new();
    let body = serde_json::json!({ "provider": provider, "name": name, "key": key });
    client.post_void("/config/api-keys", &body).await
}

/// Remove an API key by id.
#[tauri::command]
async fn remove_api_key(id: String) -> Result<(), String> {
    let client = GatewayClient::new();
    client
        .post_void(
            &format!("/config/api-keys/{id}/remove"),
            &serde_json::json!({}),
        )
        .await
}

/// Get all budget configurations.
#[tauri::command]
async fn get_budgets() -> Result<serde_json::Value, String> {
    let client = GatewayClient::new();
    client.get::<serde_json::Value>("/config/budgets").await
}

/// Set or update budget for a user.
#[tauri::command]
async fn set_budget(
    user_id: String,
    tokens_per_hour: u64,
    tokens_per_day: u64,
    tokens_per_month: u64,
) -> Result<(), String> {
    let client = GatewayClient::new();
    let body = serde_json::json!({
        "user_id": user_id,
        "tokens_per_hour": tokens_per_hour,
        "tokens_per_day": tokens_per_day,
        "tokens_per_month": tokens_per_month,
    });
    client.post_void("/config/budgets", &body).await
}

/// Remove a budget by id.
#[tauri::command]
async fn remove_budget(id: String) -> Result<(), String> {
    let client = GatewayClient::new();
    client
        .post_void(
            &format!("/config/budgets/{id}/remove"),
            &serde_json::json!({}),
        )
        .await
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal URL path encoding (only for the user_id segment).
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
        .invoke_handler(tauri::generate_handler![
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
