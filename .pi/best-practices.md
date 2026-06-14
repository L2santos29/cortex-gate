# Best Practices — cortex-gate v0.1

> Generated: 2026-06-14
> Stack: Rust (Axum 0.8, tokio, reqwest, Tauri 2, rusqlite)
> Architecture: Monolithic gateway → Extensible platform (in progress)

---

## Executive Summary

cortex-gate is an AI gateway that proxies requests to multiple LLM providers (OpenAI, Anthropic, OpenRouter) with embedding-based prompt routing, cost governance, and autonomous benchmarking. The codebase is **well-structured with clean architectural boundaries** (gateway/governance/classifier/benchmark/models/tools) but has several **P0 issues that block production readiness**:

### Top 3 Opportunities

1. **P0: Error handling unification** — 3 separate error types (`AuthError`, `ApiError`, `ProxyError`) spread across modules, no `IntoResponse` for `ProxyError`, silent fallback responses in `chat_completions` stub handler.
2. **P0: SSE streaming lacks safety guards** — No backpressure limits, no keepalive, no per-chunk timeout, no client disconnect detection in `streaming.rs` loops. mpsc buffer of 64 is arbitrary.
3. **P1: Code structure redundancy** — `ProviderEntry` ↔ `ProviderConfig` duplicate fields with different types, `models/api.rs` and `models/types.rs` are placeholder stubs, no OpenAPI documentation, `tools/logging.rs` is empty.

### Confidence

| Area | Confidence | Rationale |
|------|-----------|-----------|
| Error handling | alta | Code fully read, error types visible in all files |
| Async/streaming | alta | Full streaming.rs implementation reviewed |
| API design | alta | Routes, auth, middleware stack all analyzed |
| Code structure | media | Some module boundaries inferred from mod.rs re-exports |
| Governance/DB | media | Database schema not read in detail |

---

## Best Practices by Domain

---

### 1. Error Handling (P0)

**Practice 1.1: Unified error type with IntoResponse**

src currently has `AuthError` (auth.rs), `ApiError` (routes.rs), and `ProxyError` (tools/error.rs). Each implements `IntoResponse` differently. Axum's idiomatic pattern is a single `GatewayError` enum with `#[derive(thiserror::Error)]` and one `IntoResponse` impl.

**How it applies to THIS project:**

`ProxyError` is the most mature error type but has no `IntoResponse`. `ApiError` has `IntoResponse` but is a struct, not an enum (can't match on variants). `AuthError` has its own `IntoResponse`. Result: 3 separate error response paths.

**File paths:** `src/gateway/auth.rs:17-28`, `src/gateway/routes.rs:48-56`, `src/tools/error.rs:8-47`

```rust
// src/tools/error.rs — Unified GatewayError
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("authentication failed: {0}")]
    Auth(String, #[source] Option<anyhow::Error>),

    #[error("authorization failed: {0}")]
    Forbidden(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("upstream provider error: {0}")]
    Upstream(#[from] ProxyError),

    #[error("internal error: {0}")]
    Internal(String, #[source] Option<anyhow::Error>),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, error_type) = match &self {
            GatewayError::Auth(..) => (StatusCode::UNAUTHORIZED, "authentication_error"),
            GatewayError::Forbidden(..) => (StatusCode::FORBIDDEN, "authorization_error"),
            GatewayError::InvalidRequest(..) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
            GatewayError::Upstream(ProxyError::UnknownProvider(..)) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
            GatewayError::Upstream(ProxyError::UpstreamError { status, .. }) => (*status, "upstream_error"),
            GatewayError::Upstream(..) => (StatusCode::BAD_GATEWAY, "upstream_error"),
            GatewayError::Internal(..) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };
        let body = json!({
            "error": { "message": self.to_string(), "type": error_type, "code": status.as_u16() }
        });
        (status, Json(body)).into_response()
    }
}
```

**Practice 1.2: Error chaining with anyhow + context**

`ProxyError` (tools/error.rs) uses thiserror but doesn't propagate context from reqwest errors. Add `.context()` calls in providers.rs where errors cross module boundaries.

**File path:** `src/gateway/providers.rs:80-93`

```rust
// Before (providers.rs:80-93)
// Error from reqwest is just #[from] — no context
pub async fn forward_chat_completion(&self, provider_id: &str, body: Value) -> Result<Value, ProxyError> {
    let response = client.post(&url).json(&body).send().await?;  // reqwest::Error -> ProxyError::Reqwest
    // ...
    return Err(ProxyError::UpstreamError { status, body: error_body });
}

// After — add context for debugging
use anyhow::Context;
pub async fn forward_chat_completion(&self, provider_id: &str, body: Value) -> Result<Value, ProxyError> {
    let response = client.post(&url).json(&body).send().await
        .map_err(|e| ProxyError::Reqwest(e.context(format!("POST to {provider_id} at {url} failed"))))?;
    // ...
}
```

**Practice 1.3: Structured error logging with TraceLayer**

server.rs doesn't use `TraceLayer` from tower-http. Add request/response logging with span propagation.

**File path:** `src/gateway/server.rs:95-110`

```rust
use tower_http::trace::TraceLayer;

pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", axum::routing::get(routes::health))
        .route("/v1/models", axum::routing::get(routes::models))
        .route("/v1/chat/completions", axum::routing::post(routes::chat_completions))
        .route("/admin/config", axum::routing::get(routes::admin_config_get).post(routes::admin_config_post))
        .layer(TraceLayer::new_for_http())  // ← ADD: request logging with span
        .layer(cors)
        .with_state(state)
}
```

**Priority rationale:** Without unified error handling, every new route or extension must reimplement error serialization. This blocks the extension system because extensions need a consistent error contract.

**References:**
- https://docs.rs/axum/latest/axum/response/trait.IntoResponse.html#implementing-intoresponse-for-enums
- https://docs.rs/tower-http/latest/tower_http/trace/index.html

---

### 2. Async & Streaming (P0)

**Practice 2.1: SSE keepalive on all streams**

Both `stream_openai_response` and `stream_anthropic_response` (streaming.rs) begin streaming immediately but have no keepalive pings. If upstream is slow or pauses, the downstream client has no way to distinguish "still processing" from "connection lost."

**File path:** `src/gateway/streaming.rs:62-75`, `streaming.rs:175-190`

```rust
// After — add keepalive
use axum::response::sse::KeepAlive;
use std::time::Duration;

pub fn stream_openai_response(upstream: Response) -> Sse<impl Stream<Item = SseResult>> {
    let (tx, rx) = mpsc::channel::<SseResult>(64);
    tokio::spawn(async move {
        if let Err(e) = run_openai_sse_loop(upstream, tx.clone()).await {
            error!("OpenAI SSE stream error: {}", e);
        }
    });
    let stream = unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text(": keepalive\n"))  // SSE comment — ignored by client, keeps connection alive
}
```

**Practice 2.2: select! with client disconnect detection**

The SSE loops in streaming.rs read from the upstream byte stream but never check if the downstream has disconnected. If the client disconnects, the spawned tokio task keeps running until the upstream stream ends.

**File path:** `src/gateway/streaming.rs:82-115` (run_openai_sse_loop)

```rust
// After — add cancellation on client disconnect
use tokio_util::sync::CancellationToken;

pub fn stream_openai_response(upstream: Response) -> Sse<impl Stream<Item = SseResult>> {
    let (tx, rx) = mpsc::channel::<SseResult>(64);
    let cancel = CancellationToken::new();

    tokio::spawn(async move {
        tokio::select! {
            _ = cancel.cancelled() => {
                debug!("OpenAI SSE: client disconnected");
            }
            result = run_openai_sse_loop(upstream, tx.clone()) => {
                if let Err(e) = result {
                    error!("OpenAI SSE stream error: {}", e);
                }
            }
        }
    });

    let stream = unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text(": keepalive\n"))
}
```

Note: The `CancellationToken` needs to be sent back to the caller so it can be cancelled when the response stream reader completes. Alternatively, wrap the stream and cancel on drop.

**Practice 2.3: Max SSE buffer size guard**

The `buf: String` in `run_openai_sse_loop` grows unbounded. If a malicious upstream sends no `\n\n` delimiters, memory grows until OOM.

**File path:** `src/gateway/streaming.rs:85-86`

```rust
// Before
let mut buf = String::new();

// After
const MAX_SSE_BUF_SIZE: usize = 1_000_000; // 1MB
let mut buf = String::new();

// In the while loop:
if buf.len() > MAX_SSE_BUF_SIZE {
    return Err("SSE buffer exceeded max size (1MB)".into());
}
```

**Practice 2.4: Per-chunk idle timeout**

If upstream stops sending data mid-stream, the tokio task hangs forever. Add a timeout on each `byte_stream.next()`.

**References:**
- https://docs.rs/tokio-stream/latest/tokio_stream/wrappers/struct.ReceiverStream.html
- https://docs.rs/axum/latest/axum/response/sse/struct.KeepAlive.html
- https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html

---

### 3. API Design & Middleware (P0)

**Practice 3.1: Tower ServiceBuilder middleware stack**

server.rs currently applies CORS and state. Axum's idiomatic approach is a structured middleware stack via `ServiceBuilder`.

**File path:** `src/gateway/server.rs:86-115`

```rust
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    timeout::TimeoutLayer,
    request_id::SetRequestIdLayer,
    propagate::PropagateRequestIdLayer,
};
use std::time::Duration;

pub fn build_router(state: Arc<AppState>) -> Router {
    let middleware_stack = ServiceBuilder::new()
        .layer(SetRequestIdLayer::new(
            uuid::Uuid::new_v4().to_string().into(),
        ))
        .layer(PropagateRequestIdLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(60)))
        .into_inner();

    Router::new()
        .route("/health", axum::routing::get(routes::health))
        .route("/v1/models", axum::routing::get(routes::models))
        .route("/v1/chat/completions", axum::routing::post(routes::chat_completions))
        .route("/admin/config", axum::routing::get(routes::admin_config_get).post(routes::admin_config_post))
        .layer(middleware_stack)
        .with_state(state)
}
```

**Practice 3.2: Typed request/response structs**

`chat_completions` in routes.rs uses `Json<Value>` for both request and response. This bypasses compile-time validation. Define typed structs for the OpenAI-compatible API.

**File path:** `src/gateway/routes.rs:66-110`, `src/models/api.rs`

```rust
// Add to src/models/api.rs
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: Option<bool>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Option<Vec<ToolDef>>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: serde_json::Value,  // string or array of content parts
    pub tool_calls: Option<Value>,
    pub tool_call_id: Option<String>,
}

impl Default for ChatCompletionRequest {
    fn default() -> Self {
        Self {
            model: "gpt-4o-mini".to_string(),
            messages: vec![],
            stream: Some(false),
            temperature: None,
            max_tokens: None,
            tools: None,
        }
    }
}
```

Then use it in the handler:

```rust
pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ChatCompletionRequest>,
) -> Result<Json<Value>, GatewayError> {
    if body.messages.is_empty() {
        return Err(GatewayError::InvalidRequest("'messages' must not be empty".into()));
    }
    // ... proceed with typing
}
```

**Practice 3.3: utoipa-axum for OpenAPI docs**

The project has no API documentation. `utoipa-axum` generates OpenAPI 3.1 from typed handlers with minimal annotations.

```rust
#[utoipa::path(
    get,
    path = "/v1/models",
    responses(
        (status = 200, description = "List of available models", body = ModelsListResponse)
    )
)]
pub async fn models(State(state): State<Arc<AppState>>) -> Json<Value> {
    // ...
}
```

**Practice 3.4: Module-per-feature router nesting**

As the project grows toward an extensible platform, use `.nest()` to group routes by feature:

```rust
Router::new()
    .nest("/api/v1", api_routes())
    .nest("/admin", admin_routes())
    .route("/health", axum::routing::get(routes::health))
```

For the extension system, each extension registers its own sub-router:

```rust
// In the extension registry:
pub fn register_extension_router(router: Router, ext: &dyn Extension) -> Router {
    router.nest(&format!("/ext/{}/api", ext.id()), ext.router())
}
```

**References:**
- https://docs.rs/axum/latest/axum/struct.Router.html#method.nest
- https://docs.rs/utoipa-axum/latest/utoipa_axum/
- https://docs.rs/tower-http/latest/tower_http/timeout/index.html

---

### 4. Code Structure (P1)

**Practice 4.1: Eliminate stub/placeholder modules**

Several modules are declared but contain only TODO comments or are empty:

- `src/models/api.rs` — should hold ChatCompletionRequest/Response types
- `src/models/types.rs` — should re-export or be removed
- `src/tools/logging.rs` — declared in mod.rs but probably empty

```rust
// If a module file is empty or just re-exports, remove it:
// Before: src/tools/mod.rs
pub mod provider;
pub mod error;
pub mod logging;  // <-- empty stub

// After: remove logging.rs + pub mod logging from mod.rs
pub mod provider;
pub mod error;
```

**Practice 4.2: DRY ProviderEntry ↔ ProviderConfig duplication**

`models/config.rs::ProviderEntry` and `tools/provider.rs::ProviderConfig` have overlapping fields but different structures:

| Field | ProviderEntry | ProviderConfig |
|-------|--------------|----------------|
| name/id | `name: String` | `id: String` |
| base_url | `base_url: String` | `base_url: String` |
| api_key | `api_key: Option<String>` | `api_key: String` |
| provider_type | `provider_type: String` | `provider_type: ProviderType` (enum) |
| models | `models: Vec<String>` | `models: Vec<String>` |

**Fix:** Convert `ProviderEntry` to use the same types, or derive `From<ProviderEntry>` for `ProviderConfig`. Use `ProviderType` enum in `ProviderEntry` too.

**File paths:** `src/models/config.rs:16-32`, `src/tools/provider.rs:26-34`

```rust
// In models/config.rs — use ProviderType instead of String
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub provider_type: ProviderType,   // ← was String
    pub models: Vec<String>,
}

impl From<ProviderEntry> for ProviderConfig {
    fn from(entry: ProviderEntry) -> Self {
        ProviderConfig {
            id: entry.name,
            base_url: entry.base_url,
            api_key: entry.api_key.unwrap_or_default(),
            provider_type: entry.provider_type,
            models: entry.models,
            default_model: None,
            timeout_secs: 60,
            max_retries: 3,
        }
    }
}
```

**Practice 4.3: DRY provider call logic**

`forward_chat_completion` and `forward_chat_completion_raw` in providers.rs are almost identical — same URL construction, same error handling, same POST. Keep one, let the caller decide whether to consume JSON or raw.

**File path:** `src/gateway/providers.rs:65-108`

```rust
// Refactor into one method that returns the Response:
async fn forward_to_provider(&self, provider_id: &str, body: Value) -> Result<Response, ProxyError> {
    let (config, client) = self.clients.get(provider_id)
        .ok_or_else(|| ProxyError::UnknownProvider(provider_id.to_string()))?;
    let url = format!("{}/v1/chat/completions", config.base_url.trim_end_matches('/'));
    let response = client.post(&url).json(&body).send().await?;
    if !response.status().is_success() {
        return Err(ProxyError::UpstreamError {
            status: response.status(),
            body: response.text().await.unwrap_or_default(),
        });
    }
    Ok(response)
}

pub async fn forward_chat_completion(&self, provider_id: &str, body: Value) -> Result<Value, ProxyError> {
    let response = self.forward_to_provider(provider_id, body).await?;
    Ok(response.json().await?)
}

pub async fn forward_chat_completion_raw(&self, provider_id: &str, body: Value) -> Result<Response, ProxyError> {
    self.forward_to_provider(provider_id, body).await
}
```

**Practice 4.4: Implement From traits instead of manual mapping**

The `normalize_anthropic_response` function manually maps fields from Anthropic to OpenAI. Use `From` traits for testable, composable conversions.

```rust
// In tools/provider.rs
impl From<AnthropicMessage> for OpenAIChatCompletion {
    fn from(msg: AnthropicMessage) -> Self {
        // structured conversion, testable in isolation
    }
}
```

**Practice 4.5: DRY budget period checks**

Check governance/quota module for repeated period calculation logic. Extract into a helper function in `governance/tracking.rs`.

**References:**
- https://doc.rust-lang.org/std/convert/trait.From.html
- https://serde.rs/attr-flatten.html (for config merge patterns)

---

### 5. Safety & Monitoring (P1)

**Practice 5.1: Reqwest retry + circuit breaker per provider**

providers.rs has a `max_retries` field in `ProviderConfig` but never uses it. Add retry logic with exponential backoff.

**File path:** `src/tools/provider.rs:105-125`, `src/gateway/providers.rs:65-108`

```rust
// In providers.rs — retry with backoff
use backon::Retryable;  // or implement manually

pub async fn forward_to_provider(&self, provider_id: &str, body: Value) -> Result<Response, ProxyError> {
    let (config, client) = self.clients.get(provider_id)
        .ok_or_else(|| ProxyError::UnknownProvider(provider_id.to_string()))?;
    let url = format!("{}/v1/chat/completions", config.base_url.trim_end_matches('/'));

    let retries = if config.max_retries > 0 { config.max_retries } else { 0 };
    let mut attempt = 0;
    loop {
        let result = client.post(&url).json(&body).send().await;
        match result {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) if attempt < retries && response.status().is_server_error() => {
                attempt += 1;
                let wait = Duration::from_millis(500 * 2u64.pow(attempt));
                tokio::time::sleep(wait).await;
                continue;
            }
            Ok(response) => return Err(ProxyError::UpstreamError {
                status: response.status(),
                body: response.text().await.unwrap_or_default(),
            }),
            Err(e) if attempt < retries && e.is_timeout() => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(500 * 2u64.pow(attempt))).await;
                continue;
            }
            Err(e) => return Err(ProxyError::Reqwest(e)),
        }
    }
}
```

**Practice 5.2: HandleErrorLayer for middleware error boundaries**

server.rs doesn't catch panics or errors from middleware. Add `HandleErrorLayer` from tower-http:

```rust
use tower_http::set_status::SetStatus;

let middleware_stack = ServiceBuilder::new()
    .layer(HandleErrorLayer::new(|_: axum::BoxError| async move {
        StatusCode::INTERNAL_SERVER_ERROR
    }))
    .layer(TimeoutLayer::new(Duration::from_secs(60)))
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http());
```

**Practice 5.3: DefaultBodyLimit for request size**

No `DefaultBodyLimit` is set. A client could send a multi-gigabyte request body.

```rust
// Add to server.rs
use axum::extract::DefaultBodyLimit;

Router::new()
    .layer(DefaultBodyLimit::max(50 * 1024 * 1024))  // 50MB max
    // ...
```

**Practice 5.4: Replace .unwrap()/.expect() with fallible alternatives**

Several places use `unwrap()` or `expect()` where graceful error handling is possible:

| File | Line | Current | Alternative |
|------|------|---------|-------------|
| src/main.rs | 15 | `.init()` | Already infallible — OK |
| src/main.rs | 25 | `.expect("Failed to bind...") | Convert to eprintln + exit with context |
| src/main.rs | 30 | `.expect("Server exited...") | Convert to eprintln + exit |
| src/gateway/server.rs | 55 | `.expect("Failed to build HTTP client") | Propagate as InitError |
| src/gateway/server.rs | 66 | `.expect("Failed to initialize database") | Convert to panic or graceful shutdown |
| src/tools/provider.rs | 97 | `.expect("Invalid API key") | Return Result<Client, String> |
| src/tools/provider.rs | 104 | `.expect("Invalid Anthropic API key") | Same — propagate instead of panicking |

```rust
// Better pattern in main.rs:
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,cortex_gate=debug".into()),
        )
        .init();

    let app = cortex_gate::create_app().await;

    let addr = "127.0.0.1:18801";
    let listener = tokio::net::TcpListener::bind(addr).await
        .with_context(|| format!("Failed to bind to {addr} — is it already in use?"))?;

    tracing::info!("Cortex Gate listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
```

**References:**
- https://docs.rs/tower-http/latest/tower_http/timeout/index.html
- https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html
- https://docs.rs/tower-http/latest/tower_http/set_status/index.html

---

### 6. Extension System Architecture (P0 — New)

This is the core architectural change requested: cortex-gate as an extensible platform.

**Practice 6.1: Extension trait definition**

Define a Rust trait for extensions that can register routes, hooks, and frontend pages:

```rust
// In src/extensions/mod.rs
#[async_trait]
pub trait Extension: Send + Sync + 'static {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn router(&self) -> Option<Router> {
        None
    }
    fn frontend_pages(&self) -> Vec<FrontendPage> {
        vec![]
    }
    async fn on_load(&self, state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
    async fn on_unload(&self, state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct FrontendPage {
    pub name: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
}
```

**Practice 6.2: Extension registry in AppState**

```rust
// In gateway/server.rs
pub struct AppState {
    pub http_client: reqwest::Client,
    pub db: Database,
    pub config: CortexConfig,
    pub uptime: Instant,
    pub extensions: Vec<Box<dyn Extension>>,  // ← NEW
}
```

**Practice 6.3: Extension registration in create_app**

```rust
// In lib.rs
pub async fn create_app() -> Router {
    let state = gateway::server::init_app_state().await;
    let router = gateway::server::build_router(state.clone());

    // Load extensions
    let mut router = router;
    for ext in &state.extensions {
        if let Some(ext_router) = ext.router() {
            router = router.nest(&format!("/ext/{}", ext.id()), ext_router);
        }
        if let Err(e) = ext.on_load(&state).await {
            tracing::error!("Extension {} failed to load: {}", ext.id(), e);
        }
    }

    router
}
```

**Practice 6.4: Prompt Router as first extension**

Encapsulates the current classifier + provider selection logic:

```rust
// In src/extensions/prompt_router/mod.rs
pub struct PromptRouterExtension;

#[async_trait]
impl Extension for PromptRouterExtension {
    fn id(&self) -> &'static str { "prompt-router" }
    fn name(&self) -> &'static str { "Prompt Router" }
    fn description(&self) -> &'static str { "Routes prompts to optimal AI models based on embedding classification" }
    fn version(&self) -> &'static str { "0.1.0" }
    fn router(&self) -> Option<Router> {
        Some(Router::new()
            .route("/classify", axum::routing::post(handle_classify))
            .route("/providers", axum::routing::get(handle_providers))
            .route("/economy", axum::routing::post(handle_economy)))
    }
    fn frontend_pages(&self) -> Vec<FrontendPage> {
        vec![FrontendPage {
            name: "prompt-router",
            label: "Prompt Router",
            icon: "equalizer",
        }]
    }
    async fn on_load(&self, state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Prompt Router extension loaded");
        Ok(())
    }
}
```

**References:**
- https://doc.rust-lang.org/book/ch17-02-trait-objects.html
- https://docs.rs/axum/latest/axum/struct.Router.html#method.nest

---

## Quick Wins (High Impact, Low Effort)

| # | Change | File | Priority | Effort | Justification |
|---|--------|------|----------|--------|---------------|
| 1 | Add `#[derive(Clone)]` to `AppState` | `src/gateway/server.rs` | P0 | 1 line | Required for extensions sharing state |
| 2 | Add `TraceLayer` to middleware stack | `src/gateway/server.rs` | P0 | 3 lines | Enables structured request logging |
| 3 | Add SSE keepalive to both stream functions | `src/gateway/streaming.rs` | P1 | 4 lines | Prevents proxy timeouts on long streams |
| 4 | Replace `Json<Value>` with typed `ChatCompletionRequest` | `src/gateway/routes.rs`, `src/models/api.rs` | P1 | 30 lines | Compile-time validation, required for OpenAPI |
| 5 | Add `From<ProviderEntry>` for `ProviderConfig` | `src/models/config.rs` | P1 | 15 lines | Eliminates manual conversion, enables dynamic provider registration |

---

## Architecture Roadmap

### Phase 1 — Stabilize (Now)
- [ ] Unified `GatewayError` enum with `IntoResponse`
- [ ] `TraceLayer` for request logging
- [ ] SSE keepalive on all streams
- [ ] `DefaultBodyLimit` (50MB)
- [ ] Replace `.unwrap()` in `main.rs` with graceful error

### Phase 2 — Extensibility (Next)
- [ ] Define `Extension` trait in `src/extensions/mod.rs`
- [ ] Add `Vec<Box<dyn Extension>>` to `AppState`
- [ ] Register prompt-router as first extension
- [ ] Frontend extension registry (JS → Rust bridge via Tauri)
- [ ] Typed request/response structs for OpenAI API
- [ ] `utoipa-axum` OpenAPI annotations

### Phase 3 — Production (Future)
- [ ] Circuit breaker per provider (backon + health checks)
- [ ] Request retry with exponential backoff
- [ ] Dynamic extension loading via config file
- [ ] Extension hot-reload during development
- [ ] Provider health check endpoint
- [ ] Prometheus metrics for request latency, token usage, error rates

---

## References

1. https://docs.rs/axum/latest/axum/response/trait.IntoResponse.html — Unified error responses
2. https://docs.rs/tower-http/latest/tower_http/trace/index.html — Request tracing middleware
3. https://docs.rs/axum/latest/axum/response/sse/struct.KeepAlive.html — SSE keepalive
4. https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html — Task cancellation
5. https://docs.rs/utoipa-axum/latest/utoipa_axum/ — OpenAPI from axum handlers
6. https://docs.rs/tower-http/latest/tower_http/timeout/index.html — Timeout middleware
7. https://docs.rs/tower-http/latest/tower_http/set_status/index.html — Error boundary middleware
8. https://doc.rust-lang.org/book/ch17-02-trait-objects.html — Trait objects for extension system
9. https://docs.rs/axum/latest/axum/struct.Router.html#method.nest — Router nesting for extensions
10. https://docs.rs/backon/latest/backon/ — Retry with exponential backoff

---

## Meta-prompt

This is the canonical best-practices document for cortex-gate.
USE THIS TO:
1. Check before writing new code — follow the patterns documented here
2. Prioritize changes by P0/P1/P2
3. Start with Quick Wins for immediate improvements
4. Use Architecture Roadmap for long-term planning
INPUT FOR: Code suggestions and future /study runs.
