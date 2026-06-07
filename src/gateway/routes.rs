// Cortex Gate — HTTP Route Handlers
//
// Define los handlers para todos los endpoints del gateway:
//   - /health              (GET)
//   - /v1/models           (GET)
//   - /v1/chat/completions (POST)
//   - /admin/config        (GET, POST)
//
// Los handlers que requieren lógica de negocio real (providers,
// streaming, clasificador) llevan marcadores // TODO para indicar
// dónde conectar los módulos cuando estén implementados.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::gateway::auth::{require_admin_auth, require_client_auth};
use crate::gateway::server::AppState;

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// `GET /health` — Health check básico.
///
/// Devuelve el estado del servidor, versión y uptime. Útil para
/// balancers, health checks de Kubernetes y monitorización.
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

/// `GET /v1/models` — Lista los modelos disponibles.
///
/// Devuelve un listado compatible con la API de OpenAI para que
/// herramientas como LangChain, OpenRouter o clientes HTTP estándar
/// puedan descubrir los modelos.
///
/// TODO: poblar la lista desde la configuración de proveedores activos
///       y filtrar por disponibilidad.
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

/// Tipo de error devuelto por los handlers compatibles con OpenAI.
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

/// `POST /v1/chat/completions` — Endpoint principal de inferencia.
///
/// Recibe un payload compatible con OpenAI Chat Completions, autentica
/// al cliente, clasifica el prompt, selecciona el proveedor y modelo
/// objetivo, y devuelve la respuesta.
///
/// ## TODO
/// - [ ] Clasificador de embeddings → modelo objetivo
/// - [ ] Selección de proveedor por disponibilidad/coste
/// - [ ] Llamada real al proveedor vía reqwest
/// - [ ] Streaming SSE (Server-Sent Events)
/// - [ ] Tracking de uso (tokens, coste) en base de datos
/// - [ ] Rate limiting y cuotas
pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    // 1. Autenticación de cliente
    require_client_auth(&headers, &state.config.client_api_key)
        .map_err(|e| ApiError {
            message: e.message,
            status: e.status,
            error_type: "authentication_error".to_string(),
        })?;

    // 2. Validar payload mínimo (debe tener model y messages)
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(&state.config.default_model);

    let messages = body
        .get("messages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ApiError {
            message: "Missing required field: 'messages'".to_string(),
            status: StatusCode::BAD_REQUEST,
            error_type: "invalid_request_error".to_string(),
        })?;

    if messages.is_empty() {
        return Err(ApiError {
            message: "'messages' array must not be empty".to_string(),
            status: StatusCode::BAD_REQUEST,
            error_type: "invalid_request_error".to_string(),
        });
    }

    // 3. TODO: Clasificar prompt → elegir modelo/proveedor
    #[allow(unused_variables)]
    let economy = state.config.economy;

    // 4. TODO: Llamar al proveedor seleccionado
    //
    //    let provider = select_provider(&state, model).await?;
    //    let response = call_provider(&state.http_client, &provider, &body).await?;
    //    track_usage(&state.db, &response).await?;

    tracing::info!(
        target: "cortex_gate::gateway::routes",
        "chat_completion request: model={}, messages={}",
        model,
        messages.len(),
    );

    // 5. TODO: Respuesta simulada mientras no hay providers
    //
    // Eliminar este bloque cuando se implemente la llamada real al
    // proveedor.
    let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());

    Ok(Json(json!({
        "id": response_id,
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": format!(
                    "[cortex-gate dev mode] Hello! You sent a request for model '{}' with {} message(s). \
                     Provider routing and streaming are not yet connected. \
                     Economy level: {:.1}",
                    model,
                    messages.len(),
                    economy,
                ),
            },
            "finish_reason": "stop",
        }],
        "usage": {
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0,
        },
    })))
}

// ---------------------------------------------------------------------------
// Admin: Config
// ---------------------------------------------------------------------------

/// `GET /admin/config` — Devuelve la configuración actual del gateway.
///
/// Requiere autenticación de administrador via `X-Admin-Token`.
/// Oculta los campos sensibles (api keys) en la respuesta.
pub async fn admin_config_get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    require_admin_auth(&headers, &state.config.admin_token).map_err(|e| ApiError {
        message: e.message,
        status: e.status,
        error_type: "authorization_error".to_string(),
    })?;

    // Serializar la configuración ocultando campos sensibles
    let mut config_json =
        serde_json::to_value(&state.config).map_err(|e| ApiError {
            message: format!("Failed to serialize config: {}", e),
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_type: "internal_error".to_string(),
        })?;

    // Ofuscar valores sensibles en la respuesta
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

/// `POST /admin/config` — Actualiza la configuración en caliente.
///
/// Recibe un JSON parcial con los campos a modificar. Los campos no
/// incluidos mantienen su valor actual.
///
/// ## TODO
/// - [ ] Validar que los cambios no dejen el sistema inconsistente
/// - [ ] Persistir en config.json
/// - [ ] Notificar a providers activos si cambia su configuración
/// - [ ] Versionar cambios de configuración
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

    // TODO: mergear body con la config actual y validar
    //
    //    let mut config = state.config.clone();
    //     apply_partial_config(&mut config, &body)?;
    //     validate_config(&config)?;
    //     config.save_to_file("config.json")?;

    // Por ahora solo devolvemos un ack
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

    // Helper: crea un AppState mínimo para tests
    async fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            http_client: reqwest::Client::new(),
            db: crate::governance::Database::open_or_create(":memory:").await.unwrap(),
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
    async fn test_chat_completions_missing_messages() {
        let state = test_state().await;
        let headers = HeaderMap::new();
        let body = json!({ "model": "gpt-4o" });

        let result = chat_completions(State(state), headers, Json(body)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, StatusCode::UNAUTHORIZED);
        // Falla primero por auth porque no enviamos API key
    }

    #[tokio::test]
    async fn test_chat_completions_empty_messages() {
        let state = test_state().await;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(state.config.client_api_key.as_str()).unwrap(),
        );
        let body = json!({ "model": "gpt-4o", "messages": [] });

        let result = chat_completions(State(state), headers, Json(body)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert!(err.message.contains("empty"));
    }

    #[tokio::test]
    async fn test_chat_completions_stub_success() {
        let state = test_state().await;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(state.config.client_api_key.as_str()).unwrap(),
        );
        let body = json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        });

        let result = chat_completions(State(state), headers, Json(body)).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.0["object"], "chat.completion");
        assert_eq!(response.0["model"], "gpt-4o-mini");
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
