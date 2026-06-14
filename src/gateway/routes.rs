// Cortex Gate — HTTP Route Handlers
//
// Defines handlers for all gateway endpoints:
//   - /health              (GET)
//   - /v1/models           (GET)
//   - /v1/chat/completions (POST)
//   - /admin/config        (GET, POST)

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::gateway::auth::{require_admin_auth, require_client_auth};
use crate::gateway::server::AppState;
use crate::models::api::ChatCompletionRequest;
use crate::models::api::ChatCompletionResponse;
use crate::models::api::Choice;
use crate::models::api::ResponseMessage;
use crate::models::api::Usage;

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// `GET /health` — Basic health check.
pub async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "cortex-gate",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": state.uptime.elapsed().as_secs(),
    }))
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

/// `GET /v1/models` — List available models.
pub async fn models(State(state): State<Arc<AppState>>) -> Json<Value> {
    let models_list: Vec<Value> = state
        .config
        .providers
        .iter()
        .flat_map(|provider| {
            provider.models.iter().map(|model_name| {
                json!({
                    "id": model_name,
                    "object": "model",
                    "provider": provider.name,
                    "provider_type": provider.provider_type,
                    "owned_by": provider.name,
                    "created": 0,
                    "permission": [],
                    "root": model_name,
                    "parent": null,
                })
            })
        })
        .collect();

    Json(json!({
        "object": "list",
        "data": models_list,
    }))
}

// ---------------------------------------------------------------------------
// Chat Completions
// ---------------------------------------------------------------------------

/// API error type compatible with OpenAI error format.
#[derive(Debug)]
pub struct ApiError {
    pub message: String,
    pub status: StatusCode,
    pub error_type: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": {
                "message": self.message,
                "type": self.error_type,
                "code": self.status.as_u16(),
            }
        });
        (self.status, Json(body)).into_response()
    }
}

/// `POST /v1/chat/completions` — Main inference endpoint.
///
/// Receives an OpenAI-compatible Chat Completion payload, authenticates
/// the client, validates required fields, and returns a response.
///
/// ## TODO
/// - [ ] Embedding classifier → target model
/// - [ ] Provider selection by availability/cost
/// - [ ] Actual provider call via reqwest
/// - [ ] SSE streaming for `stream: true` requests
/// - [ ] Usage tracking (tokens, cost) in database
/// - [ ] Rate limiting and quotas
pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, ApiError> {
    // 1. Client authentication
    require_client_auth(&headers, &state.config.client_api_key)
        .map_err(|e| ApiError {
            message: e.message,
            status: e.status,
            error_type: "authentication_error".to_string(),
        })?;

    // 2. Validate messages
    if body.messages.is_empty() {
        return Err(ApiError {
            message: "'messages' array must not be empty".to_string(),
            status: StatusCode::BAD_REQUEST,
            error_type: "invalid_request_error".to_string(),
        });
    }

    // 3. Model selection
    let model = if body.model.is_empty() {
        &state.config.default_model
    } else {
        &body.model
    };

    // 4. TODO: Classify prompt → select model/provider
    #[allow(unused_variables)]
    let economy = state.config.economy;

    // 5. TODO: Call selected provider
    //
    //    let provider = select_provider(&state, model).await?;
    //    let response = call_provider(&state.http_client, &provider, &body).await?;
    //    track_usage(&state.db, &response).await?;

    tracing::info!(
        target: "cortex_gate::gateway::routes",
        "chat_completion request: model={}, messages={}",
        model,
        body.messages.len(),
    );

    // 6. Mock response (replace with real provider call)
    let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());

    Ok(Json(ChatCompletionResponse {
        id: response_id.clone(),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model: model.to_string(),
        choices: vec![Choice {
            index: 0,
            message: ResponseMessage {
                role: "assistant".to_string(),
                content: format!(
                    "[cortex-gate dev mode] Hello! You sent a request for model '{}' with {} message(s). \
                     Provider routing and streaming are not yet connected. \
                     Economy level: {:.1}",
                    model,
                    body.messages.len(),
                    economy,
                ),
                tool_calls: None,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Some(Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            }),
    }))
}

// ---------------------------------------------------------------------------
// Admin: Config
// ---------------------------------------------------------------------------

/// `GET /admin/config` — Return current gateway configuration.
pub async fn admin_config_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_admin_auth(&headers, &state.config.admin_token).map_err(|e| ApiError {
        message: e.message,
        status: e.status,
        error_type: "authorization_error".to_string(),
    })?;

    let mut config_json =
        serde_json::to_value(&state.config).map_err(|e| ApiError {
            message: format!("Failed to serialize config: {}", e),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_type: "internal_error".to_string(),
        })?;

    // Redact sensitive fields
    if let Some(obj) = config_json.as_object_mut() {
        if let Some(admin_token) = obj.get_mut("admin_token") {
            *admin_token = json!("***REDACTED***");
        }
        if let Some(api_key) = obj.get_mut("client_api_key") {
            *api_key = json!("***REDACTED***");
        }
        if let Some(providers) = obj.get_mut("providers") {
            if let Some(providers_arr) = providers.as_array_mut() {
                for provider in providers_arr.iter_mut() {
                    if let Some(p_api_key) = provider.get_mut("api_key") {
                        if p_api_key.as_str().map_or(false, |s| !s.is_empty()) {
                            *p_api_key = json!("***REDACTED***");
                        }
                    }
                }
            }
        }
    }

    Ok(Json(config_json))
}

/// `POST /admin/config` — Update configuration at runtime.
pub async fn admin_config_post(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    require_admin_auth(&headers, &state.config.admin_token).map_err(|e| ApiError {
        message: e.message,
        status: e.status,
        error_type: "authorization_error".to_string(),
    })?;

    tracing::info!(
        target: "cortex_gate::gateway::routes",
        "config update requested: {}",
        serde_json::to_string_pretty(&body).unwrap_or_default(),
    );

    Ok(Json(json!({
        "status": "accepted",
        "message": "Config update endpoint is a stub. No changes were applied.",
        "requested_fields": body.as_object().map(|o| o.keys().cloned().collect::<Vec<String>>()),
    })))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::time::Instant;

    async fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            http_client: reqwest::Client::new(),
            db: Arc::new(crate::governance::Database::open_or_create(":memory:").await.unwrap()),
            classifier: None,
            config: crate::models::config::CortexConfig::default(),
            uptime: Instant::now(),
        })
    }

    #[tokio::test]
    async fn test_health_returns_ok() {
        let state = test_state().await;
        let response = health(State(state)).await;
        assert_eq!(response.0["status"], "ok");
        assert_eq!(response.0["service"], "cortex-gate");
    }

    #[tokio::test]
    async fn test_chat_completions_empty_messages_fails() {
        let state = test_state().await;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(state.config.client_api_key.as_str()).unwrap(),
        );
        let body = ChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![],
            stream: None,
            temperature: None,
            max_tokens: None,
            tools: None,
        };

        let result = chat_completions(State(state), headers, Json(body)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("empty"), "expected 'empty' in error, got: {}", err.message);
    }

    #[tokio::test]
    async fn test_chat_completions_stub_success() {
        let state = test_state().await;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(state.config.client_api_key.as_str()).unwrap(),
        );
        let body = ChatCompletionRequest {
            model: "gpt-4o-mini".to_string(),
            messages: vec![
                crate::models::api::Message {
                    role: "user".to_string(),
                    content: Some(serde_json::json!("Hello")),
                    tool_calls: None,
                    tool_call_id: None,
                }
            ],
            stream: Some(false),
            temperature: None,
            max_tokens: None,
            tools: None,
        };

        let result = chat_completions(State(state), headers, Json(body)).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0.object, "chat.completion");
        assert_eq!(response.0.model, "gpt-4o-mini");
    }

    #[tokio::test]
    async fn test_admin_config_get_no_auth() {
        let state = test_state().await;
        let headers = HeaderMap::new();

        let result = admin_config_get(State(state), headers).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_admin_config_get_with_auth() {
        let state = test_state().await;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-admin-token",
            HeaderValue::from_str(state.config.admin_token.as_str()).unwrap(),
        );

        let result = admin_config_get(State(state), headers).await;
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.0["admin_token"], "***REDACTED***");
        assert_eq!(config.0["client_api_key"], "***REDACTED***");
    }
}
