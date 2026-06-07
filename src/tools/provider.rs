//! Provider abstraction — configuration, client building, request construction.
//!
//! Define los tipos y funciones para interactuar con proveedores LLM externos:
//! - **OpenRouter** — `openrouter.ai/api/v1`, compatible con OpenAI
//! - **Anthropic** — `api.anthropic.com`, API de mensajes nativa
//! - **OpenAI** — `api.openai.com`, API estándar de chat completions
//! - **Custom** — Cualquier endpoint compatible con OpenAI
//!
//! # Tool Calling Translation
//!
//! Provee funciones para traducir tool calling entre formatos:
//! - `OpenAI tool_calls ↔ Anthropic tool_use`
//! - `OpenAI tools ↔ Anthropic tools (input_schema)`

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Provider types
// ---------------------------------------------------------------------------

/// Tipos de proveedor LLM soportados.
///
/// Cada variante determina el formato de API, headers, y endpoints usados.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderType {
    /// OpenRouter — proxy multi-model con formato OpenAI-compatible
    #[serde(rename = "openrouter")]
    OpenRouter,
    /// Anthropic — Messages API nativa (Claude)
    #[serde(rename = "anthropic")]
    Anthropic,
    /// OpenAI — Chat Completions API estándar
    #[serde(rename = "openai")]
    OpenAI,
    /// Cualquier proveedor compatible con OpenAI Chat Completions
    #[serde(rename = "custom")]
    Custom(String),
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::OpenRouter => write!(f, "openrouter"),
            ProviderType::Anthropic => write!(f, "anthropic"),
            ProviderType::OpenAI => write!(f, "openai"),
            ProviderType::Custom(name) => write!(f, "custom({})", name),
        }
    }
}

// ---------------------------------------------------------------------------
// Provider configuration
// ---------------------------------------------------------------------------

/// Configuración completa de un proveedor LLM.
///
/// Almacena credenciales, URL base, modelos disponibles, y parámetros de
/// conexión (timeout, reintentos).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Identificador único alfanumérico para este proveedor
    pub id: String,
    /// URL base del API (ej: `"https://openrouter.ai/api"`)
    pub base_url: String,
    /// API key de autenticación
    pub api_key: String,
    /// Tipo de proveedor (define formato de API)
    pub provider_type: ProviderType,
    /// Lista de modelos que ofrece este proveedor
    #[serde(default)]
    pub models: Vec<String>,
    /// Modelo por defecto (opcional, usado cuando no se especifica)
    pub default_model: Option<String>,
    /// Timeout en segundos para peticiones HTTP
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Número máximo de reintentos ante fallos transitorios
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_timeout() -> u64 {
    60
}

fn default_max_retries() -> u32 {
    3
}

impl ProviderConfig {
    /// Verifica si un modelo está listado en la configuración de este proveedor.
    pub fn has_model(&self, model: &str) -> bool {
        self.models.iter().any(|m| m == model)
    }

    /// Retorna el valor del header de autenticación.
    ///
    /// Anthropic usa `x-api-key` con la key directa; los demás usan
    /// `Authorization: Bearer <key>`.
    pub fn api_key_header_value(&self) -> String {
        match self.provider_type {
            ProviderType::Anthropic => self.api_key.clone(),
            _ => format!("Bearer {}", self.api_key),
        }
    }
}

// ---------------------------------------------------------------------------
// Client builder
// ---------------------------------------------------------------------------

/// Construye un [`reqwest::Client`] pre-configurado para el proveedor.
///
/// Configura:
/// - Header de autenticación según el tipo de proveedor
/// - `Content-Type: application/json`
/// - `Accept: text/event-stream` (para soportar streaming)
/// - Headers específicos por proveedor (Anthropic version, OpenRouter referer)
/// - Timeout configurable
pub fn build_client(config: &ProviderConfig) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();

    // -- Authorization header ------------------------------------------------
    // Anthropic usa x-api-key; OpenRouter, OpenAI, Custom usan Authorization: Bearer
    let (header_name, header_value) = match config.provider_type {
        ProviderType::Anthropic => {
            let name = reqwest::header::HeaderName::from_static("x-api-key");
            let value = reqwest::header::HeaderValue::from_str(&config.api_key)
                .expect("Invalid Anthropic API key");
            (name, value)
        }
        _ => {
            let value = reqwest::header::HeaderValue::from_str(
                &format!("Bearer {}", config.api_key),
            )
            .expect("Invalid API key");
            (reqwest::header::AUTHORIZATION, value)
        }
    };
    headers.insert(header_name, header_value);

    // -- Content type ---------------------------------------------------------
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );

    // -- Accept (soporte para streaming SSE) ---------------------------------
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("text/event-stream"),
    );

    // -- Headers específicos por proveedor -----------------------------------

    // Anthropic: versión del API y beta de tools
    if config.provider_type == ProviderType::Anthropic {
        headers.insert(
            reqwest::header::HeaderName::from_static("anthropic-version"),
            reqwest::header::HeaderValue::from_static("2023-06-01"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("anthropic-beta"),
            reqwest::header::HeaderValue::from_static("tools-2024-04-04"),
        );
    }

    // OpenRouter: headers de identificación (requeridos por ToS)
    if config.provider_type == ProviderType::OpenRouter {
        headers.insert(
            reqwest::header::HeaderName::from_static("http-referer"),
            reqwest::header::HeaderValue::from_static("https://cortex-gate.local"),
        );
        headers.insert(
            reqwest::header::HeaderName::from_static("x-title"),
            reqwest::header::HeaderValue::from_static("Cortex Gate"),
        );
    }

    // -- Construir cliente ---------------------------------------------------
    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
        .expect("Failed to build reqwest Client")
}

// ---------------------------------------------------------------------------
// Request builder
// ---------------------------------------------------------------------------

/// Construye URL, headers extra y body JSON para una request de chat completion.
///
/// # Argumentos
///
/// * `config` — Configuración del proveedor destino
/// * `model` — Nombre del modelo (ej: `"gpt-4"`, `"claude-3-opus"`)
/// * `messages` — Array de mensajes en formato OpenAI-compatible
/// * `stream` — `true` para streaming SSE, `false` para respuesta completa
///
/// # Returns
///
/// `(url, headers_extra, body)` donde:
/// - `url` es la URL completa del endpoint de chat
/// - `headers_extra` son headers adicionales específicos de la request
/// - `body` es el JSON body serializable
pub fn build_request(
    config: &ProviderConfig,
    model: &str,
    messages: Value,
    stream: bool,
) -> (String, reqwest::header::HeaderMap, Value) {
    let base = config.base_url.trim_end_matches('/');

    let url = match config.provider_type {
        ProviderType::Anthropic => format!("{}/v1/messages", base),
        _ => format!("{}/v1/chat/completions", base),
    };

    let body = match config.provider_type {
        ProviderType::Anthropic => build_anthropic_request_body(model, messages, stream),
        _ => {
            let body = serde_json::json!({
                "model": model,
                "messages": messages,
                "stream": stream,
            });

            // TODO: OpenRouter puede incluir `provider` preferences:
            //   body["provider"] = json!({"order": ["OpenAI", "Azure"], "allow_fallbacks": true});

            // TODO: Soporte para `response_format`, `stop` sequences, `temperature`, etc.
            // Estos parámetros deben ser extraídos del request original y re-aplicados.

            body
        }
    };

    (url, reqwest::header::HeaderMap::new(), body)
}

/// Traduce mensajes OpenAI-compatibles al formato Anthropic Messages API.
///
/// Diferencias clave convertidas:
/// - `role: "system"` → parámetro `system` separado
/// - `content` string → content blocks array `[{type: "text", text: "..."}]`
/// - `content` array de parts → Anthropic content blocks (text, image, tool_use, tool_result)
fn build_anthropic_request_body(model: &str, messages: Value, stream: bool) -> Value {
    let mut system: Option<String> = None;
    let mut anthropic_messages: Vec<Value> = Vec::new();

    if let Value::Array(msgs) = &messages {
        for msg in msgs {
            let role = msg
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("user");

            // System message → campo separado `system`
            if role == "system" {
                system = msg
                    .get("content")
                    .and_then(|c| c.as_str().map(|s| s.to_string()));
                continue;
            }

            let content = msg.get("content");
            let anthropic_content = convert_openai_content_to_anthropic(content);

            anthropic_messages.push(serde_json::json!({
                "role": role,
                "content": anthropic_content,
            }));
        }
    }

    let mut body = serde_json::json!({
        "model": model,
        "messages": anthropic_messages,
        "stream": stream,
        "max_tokens": 4096, // valor por defecto; downstream puede override
    });

    if let Some(sys) = system {
        body["system"] = serde_json::json!(sys);
    }

    body
}

/// Convierte contenido OpenAI (string o array de content parts) a Anthropic
/// content blocks array.
fn convert_openai_content_to_anthropic(content: Option<&Value>) -> Value {
    match content {
        Some(Value::String(text)) => {
            serde_json::json!([{"type": "text", "text": text}])
        }
        Some(Value::Array(parts)) => {
            let blocks: Vec<Value> = parts
                .iter()
                .filter_map(|part| {
                    match part.get("type").and_then(|t| t.as_str()) {
                        Some("text") => part
                            .get("text")
                            .and_then(|t| t.as_str())
                            .map(|text| serde_json::json!({"type": "text", "text": text})),
                        Some("image_url") => {
                            // TODO: convertir image_url a Anthropic image media_type + data
                            // Anthropic usa: {"type": "image", "source": {"type": "base64",
                            //   "media_type": "image/jpeg", "data": "..."}}
                            tracing::warn!("image_url content block not yet translated to Anthropic format");
                            Some(serde_json::json!({
                                "type": "text",
                                "text": "[image content — translation pending]"
                            }))
                        }
                        Some("tool_use") => {
                            // Pasar directamente: Anthropic usa el mismo formato tool_use
                            Some(part.clone())
                        }
                        Some("tool_result") => {
                            // TODO: convertir tool_result a Anthropic tool_result content block
                            // Anthropic usa: {"type": "tool_result", "tool_use_id": "...", "content": "..."}
                            Some(part.clone())
                        }
                        _ => {
                            // Fallback: extraer text de cualquier content part
                            part.get("text").and_then(|t| t.as_str()).map(|text| {
                                serde_json::json!({"type": "text", "text": text})
                            })
                        }
                    }
                })
                .collect();

            serde_json::json!(blocks)
        }
        _ => serde_json::json!([{"type": "text", "text": ""}]),
    }
}

// ---------------------------------------------------------------------------
// Tools — OpenAI tools ↔ Anthropic tools
// ---------------------------------------------------------------------------

/// Convierte tools del formato OpenAI al formato Anthropic.
///
/// | OpenAI | Anthropic |
/// |--------|-----------|
/// | `type: "function"` | *(omitido)* |
/// | `function.name` | `name` |
/// | `function.description` | `description` |
/// | `function.parameters` | `input_schema` |
pub fn openai_tools_to_anthropic(tools: &Value) -> Value {
    if let Value::Array(tool_list) = tools {
        let anthropic_tools: Vec<Value> = tool_list
            .iter()
            .filter_map(|tool| {
                let function = tool.get("function")?;
                let name = function.get("name")?.as_str()?;
                let description = function
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                let input_schema = function
                    .get("parameters")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                Some(serde_json::json!({
                    "name": name,
                    "description": description,
                    "input_schema": input_schema,
                }))
            })
            .collect();

        serde_json::json!(anthropic_tools)
    } else {
        serde_json::json!([])
    }
}

/// Convierte tools del formato Anthropic al formato OpenAI.
///
/// | Anthropic | OpenAI |
/// |-----------|--------|
/// | `name` | `function.name` |
/// | `description` | `function.description` |
/// | `input_schema` | `function.parameters` |
/// | *(nuevo)* | `type: "function"` |
pub fn anthropic_tools_to_openai(tools: &Value) -> Value {
    if let Value::Array(tool_list) = tools {
        let openai_tools: Vec<Value> = tool_list
            .iter()
            .map(|tool| {
                let name = tool
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                let description = tool
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                let parameters = tool
                    .get("input_schema")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": description,
                        "parameters": parameters,
                    }
                })
            })
            .collect();

        serde_json::json!(openai_tools)
    } else {
        serde_json::json!([])
    }
}

// ---------------------------------------------------------------------------
// Tool calls — OpenAI tool_calls ↔ Anthropic tool_use content blocks
// ---------------------------------------------------------------------------

/// Convierte `tool_calls` (OpenAI streaming response) a `tool_use` content
/// blocks (Anthropic).
///
/// OpenAI `tool_calls`:
/// ```json
/// [{"id": "call_xxx", "type": "function",
///   "function": {"name": "get_weather", "arguments": "{\"loc\": \"SF\"}"}}]
/// ```
///
/// Anthropic `tool_use`:
/// ```json
/// [{"type": "tool_use", "id": "toolu_xxx", "name": "get_weather",
///   "input": {"loc": "SF"}}]
/// ```
pub fn openai_tool_calls_to_anthropic(tool_calls: &Value) -> Vec<Value> {
    let mut blocks = Vec::new();

    if let Value::Array(calls) = tool_calls {
        for call in calls {
            let id = call
                .get("id")
                .and_then(|i| i.as_str())
                .unwrap_or("toolu_000000");
            let function = call.get("function");
            let name = function
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("unknown");
            let arguments = function
                .and_then(|f| f.get("arguments"))
                .and_then(|a| a.as_str())
                .unwrap_or("{}");

            let input: Value =
                serde_json::from_str(arguments).unwrap_or(serde_json::json!({}));

            blocks.push(serde_json::json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }));
        }
    }

    blocks
}

/// Convierte Anthropic `tool_use` content blocks a OpenAI `tool_calls`.
///
/// Se usa en responses no-streaming para normalizar a formato OpenAI.
pub fn anthropic_tool_use_to_openai(content_blocks: &Value) -> Vec<Value> {
    let mut tool_calls = Vec::new();

    if let Value::Array(blocks) = content_blocks {
        for block in blocks {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                let id = block
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("call_000000");
                let name = block
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                let input = block.get("input").cloned().unwrap_or(serde_json::json!({}));
                let arguments = serde_json::to_string(&input).unwrap_or_default();

                tool_calls.push(serde_json::json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": arguments,
                    }
                }));
            }
        }
    }

    tool_calls
}

// ---------------------------------------------------------------------------
// Response normalization (non-streaming)
// ---------------------------------------------------------------------------

/// Normaliza la respuesta de un proveedor a formato OpenAI-compatible.
///
/// Anthropic devuelve `content` blocks (text + tool_use). Esto se convierte
/// a `choices[0].message.content` + `choices[0].message.tool_calls`.
pub fn normalize_response(provider_type: &ProviderType, raw: Value) -> Value {
    match provider_type {
        ProviderType::Anthropic => normalize_anthropic_response(raw),
        _ => raw, // OpenAI/OpenRouter/Custom ya están en formato compatible
    }
}

fn normalize_anthropic_response(raw: Value) -> Value {
    let anthropic_id = raw
        .get("id")
        .and_then(|i| i.as_str())
        .unwrap_or("msg_unknown");
    let model = raw
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");

    // Extraer texto y tool_use de content blocks
    let mut content_text = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();

    if let Some(content_blocks) = raw.get("content").and_then(|c| c.as_array()) {
        for block in content_blocks {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        content_text.push_str(text);
                    }
                }
                Some("tool_use") => {
                    let tc = anthropic_tool_use_to_openai(&Value::Array(vec![block.clone()]));
                    tool_calls.extend(tc);
                }
                _ => {}
            }
        }
    }

    let stop_reason = match raw
        .get("stop_reason")
        .and_then(|r| r.as_str())
    {
        Some("end_turn") | None => "stop",
        Some("tool_use") => "tool_calls",
        Some("max_tokens") => "length",
        Some(other) => other,
    };

    let mut choice = serde_json::json!({
        "index": 0,
        "message": {
            "role": "assistant",
            "content": content_text,
        },
        "finish_reason": stop_reason,
    });

    if !tool_calls.is_empty() {
        choice["message"]["tool_calls"] = serde_json::json!(tool_calls);
    }

    // Calcular usage si está disponible
    let usage = raw.get("usage").cloned().unwrap_or(serde_json::json!({
        "prompt_tokens": null,
        "completion_tokens": null,
        "total_tokens": null,
    }));

    serde_json::json!({
        "id": format!("chatcmpl-{}", &anthropic_id[..anthropic_id.len().min(12)]),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [choice],
        "usage": usage,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_tools_to_anthropic() {
        let openai = serde_json::json!([
            {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get weather for a location",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "location": {"type": "string"}
                        }
                    }
                }
            }
        ]);

        let anthropic = openai_tools_to_anthropic(&openai);

        assert_eq!(anthropic[0]["name"], "get_weather");
        assert_eq!(anthropic[0]["description"], "Get weather for a location");
        assert!(anthropic[0].get("input_schema").is_some());
        assert!(anthropic[0].get("type").is_none());
    }

    #[test]
    fn test_anthropic_tools_to_openai() {
        let anthropic = serde_json::json!([
            {
                "name": "get_weather",
                "description": "Get weather for a location",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    }
                }
            }
        ]);

        let openai = anthropic_tools_to_openai(&anthropic);

        assert_eq!(openai[0]["type"], "function");
        assert_eq!(openai[0]["function"]["name"], "get_weather");
        assert!(openai[0]["function"].get("parameters").is_some());
    }

    #[test]
    fn test_tool_call_conversion_roundtrip() {
        let openai_calls = serde_json::json!([
            {
                "id": "call_abc123",
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "arguments": r#"{"location": "San Francisco"}"#
                }
            }
        ]);

        let anthropic_blocks = openai_tool_calls_to_anthropic(&openai_calls);
        assert_eq!(anthropic_blocks[0]["type"], "tool_use");
        assert_eq!(anthropic_blocks[0]["name"], "get_weather");
        assert_eq!(anthropic_blocks[0]["input"]["location"], "San Francisco");

        let back = anthropic_tool_use_to_openai(&Value::Array(anthropic_blocks));
        assert_eq!(back[0]["id"], "call_abc123");
        assert_eq!(back[0]["function"]["name"], "get_weather");
    }

    #[test]
    fn test_normalize_anthropic_response() {
        let raw = serde_json::json!({
            "id": "msg_0123456789abcdef",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Hello! "},
                {"type": "tool_use", "id": "toolu_001", "name": "get_weather",
                 "input": {"location": "NYC"}}
            ],
            "model": "claude-3-opus",
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 20}
        });

        let normalized = normalize_anthropic_response(raw);

        assert_eq!(normalized["object"], "chat.completion");
        assert_eq!(normalized["choices"][0]["message"]["content"], "Hello! ");
        assert_eq!(normalized["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(
            normalized["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            "get_weather"
        );
    }

    #[test]
    fn test_build_anthropic_request_body_system_message() {
        let messages = serde_json::json!([
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "Hello"}
        ]);

        let body = build_anthropic_request_body("claude-3-opus", messages, false);

        assert_eq!(body["system"], "You are a helpful assistant.");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"][0]["text"], "Hello");
    }
}
