# Code Suggestions — cortex-gate v0.1

> Generated: 2026-06-14
> Source: `.pi/best-practices.md` × actual codebase at `src/`
> Methodology: Each suggestion verified against real file contents

---

## P0 — Must Fix

---

### P0.1 Add TraceLayer to middleware stack

**File:** `src/gateway/server.rs` (L95–110)

**Current code:**
```rust
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", axum::routing::get(routes::health))
        .route("/v1/models", axum::routing::get(routes::models))
        .route(
            "/v1/chat/completions",
            axum::routing::post(routes::chat_completions),
        )
        .route(
            "/admin/config",
            axum::routing::get(routes::admin_config_get)
                .post(routes::admin_config_post),
        )
        .layer(cors)
        .with_state(state)
}
```

**Recommended code:**
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
        .route(
            "/v1/chat/completions",
            axum::routing::post(routes::chat_completions),
        )
        .route(
            "/admin/config",
            axum::routing::get(routes::admin_config_get)
                .post(routes::admin_config_post),
        )
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
```

**Dependency change:** `Cargo.toml` — add `"trace"` to `tower-http` features:
```toml
tower-http = { version = "0.6", features = ["cors", "trace"] }
```

**Priority:** P0 — Without TraceLayer, every request is opaque. No structured logging of method, path, status, latency. Debugging production issues becomes guesswork.

**Effort:** Low (3 lines + 1 Cargo.toml addition)

**Link:** Best Practices §1.3, §3.1

---

### P0.2 Add SSE keepalive to all stream functions

**File:** `src/gateway/streaming.rs` (L62–75, L175–190)

**Current code (`stream_openai_response`):**
```rust
pub fn stream_openai_response(upstream: Response) -> Sse<impl Stream<Item = SseResult>> {
    let (tx, rx) = mpsc::channel::<SseResult>(64);

    tokio::spawn(async move {
        if let Err(e) = run_openai_sse_loop(upstream, tx.clone()).await {
            error!("OpenAI SSE stream error: {}", e);
            let _ = tx.send(Err(e)).await;
        }
    });

    let stream = unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });

    Sse::new(stream)
}
```

**Recommended code:**
```rust
use axum::response::sse::KeepAlive;
use std::time::Duration;

pub fn stream_openai_response(upstream: Response) -> Sse<impl Stream<Item = SseResult>> {
    let (tx, rx) = mpsc::channel::<SseResult>(64);

    tokio::spawn(async move {
        if let Err(e) = run_openai_sse_loop(upstream, tx.clone()).await {
            error!("OpenAI SSE stream error: {}", e);
            let _ = tx.send(Err(e)).await;
        }
    });

    let stream = unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });

    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text(": keepalive\n"),
        )
}
```

**Same fix needed for `stream_anthropic_response`** at L175–190.

**Priority:** P0 — Without keepalive, proxies (Cloudflare, AWS ALB, Nginx) timeout idle SSE connections after 60–120s. Long-running streams (e.g., Claude with long thinking) get silently disconnected mid-response.

**Effort:** Low (4 lines, two functions = 8 lines total)

**Link:** Best Practices §2.1

---

### P0.3 Add DefaultBodyLimit to prevent oversized requests

**File:** `src/gateway/server.rs` (L95–110)

**Current code:** No body size limit exists. `axum` defaults to infernal limit, but without explicit `DefaultBodyLimit` a client can send a multi-GB payload.

**Recommended code:**
```rust
use axum::extract::DefaultBodyLimit;

pub fn build_router(state: Arc<AppState>) -> Router {
    // ...
    Router::new()
        .route("/health", axum::routing::get(routes::health))
        .route("/v1/models", axum::routing::get(routes::models))
        .route(
            "/v1/chat/completions",
            axum::routing::post(routes::chat_completions),
        )
        .route(
            "/admin/config",
            axum::routing::get(routes::admin_config_get)
                .post(routes::admin_config_post),
        )
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))  // 50 MB
        .layer(cors)
        .with_state(state)
}
```

**Priority:** P0 — Without this limit, any authenticated or public endpoint can receive arbitrary-sized payloads, risking OOM or disk exhaustion.

**Effort:** Low (2 lines)

**Link:** Best Practices §5.3

---

### P0.4 Replace `.expect()` / `.unwrap()` with graceful error handling

#### P0.4a — main.rs (L20–30)

**File:** `src/main.rs`

**Current code:**
```rust
#[tokio::main]
async fn main() {
    // ...
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener — is port 18801 already in use?");

    axum::serve(listener, app)
        .await
        .expect("Server exited with error");
}
```

**Recommended code:**
```rust
use anyhow::Context;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ...
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to {addr} — is port 18801 already in use?"))?;

    tracing::info!(
        target: "cortex_gate::main",
        "Cortex Gate listening on http://{}",
        addr,
    );

    axum::serve(listener, app).await?;
    Ok(())
}
```

**Dependency change:** `Cargo.toml` — add `anyhow`:
```toml
anyhow = "1"
```

#### P0.4b — server.rs (L55, L66)

**File:** `src/gateway/server.rs`

**Current code:**
```rust
        .build()
        .expect("Failed to build HTTP client");   // L55

        .expect("Failed to initialize database");  // L66
```

**Recommended code (convert `init_app_state` to return `Result`):**
```rust
pub async fn init_app_state() -> Result<Arc<AppState>, Box<dyn std::error::Error>> {
    let config = CortexConfig::load();

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .connect_timeout(std::time::Duration::from_secs(10))
        .user_agent(concat!("cortex-gate/", env!("CARGO_PKG_VERSION")))
        .build()?;  // Propagate error instead of expect

    // ...
    let db = Database::open_or_create(&config.db_path)
        .await?;  // Propagate error instead of expect

    Ok(Arc::new(AppState { /* ... */ }))
}
```

Then update `lib.rs`:
```rust
pub async fn create_app() -> Router {
    let state = gateway::server::init_app_state()
        .await
        .expect("Failed to initialize app state");  // Single panic point at the top
    gateway::server::build_router(state)
}
```

**Priority:** P0 — `.expect()` in library code (server.rs) prevents callers from handling failures. In binary code (main.rs), it gives poor UX. These are the top 4 panic points in production startup.

**Effort:** Medium (needs `anyhow` dependency, `Result` propagation through `init_app_state`, `lib.rs` update)

**Link:** Best Practices §5.4

---

## P1 — Should Fix

---

### P1.1 Typed `ChatCompletionRequest` replacing `Json<Value>`

**File:** `src/gateway/routes.rs` (L66–110), `src/models/api.rs` (entire file)

**Current code (routes.rs):**
```rust
pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,            // ← untyped
) -> Result<Json<Value>, ApiError> {    // ← untyped response
```

**Current code (models/api.rs):**
```rust
pub struct ApiPlaceholder;
```

**Recommended code (models/api.rs):**
```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: Option<bool>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Option<Vec<ToolDefinition>>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Value,  // string or array of content parts
    pub tool_calls: Option<Value>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<Value>,
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

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: MessageContent,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct MessageContent {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Value>>,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

Then in `routes.rs`:
```rust
use crate::models::api::ChatCompletionRequest;

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, ApiError> {
    // 1. Authentication
    require_client_auth(&headers, &state.config.client_api_key)
        .map_err(|e| ApiError { ... })?;

    // 2. Validate messages
    if body.messages.is_empty() {
        return Err(ApiError {
            message: "'messages' must not be empty".to_string(),
            status: StatusCode::BAD_REQUEST,
            error_type: "invalid_request_error".to_string(),
        });
    }

    // 3. Proceed with typed body.model, body.stream, etc.
    // ...
}
```

**Priority:** P1 — Not blocking current stub behavior, but every new feature (streaming, tool calls, provider routing) will need field access. `Json<Value>` bypasses compile-time validation: a typo in `"massages"` instead of `"messages"` silently becomes `None`.

**Effort:** Medium (~50 lines types + handler refactor)

**Link:** Best Practices §3.2

---

### P1.2 `From<ProviderEntry>` for `ProviderConfig`

**Files:** `src/models/config.rs` (L16–32), `src/tools/provider.rs` (L26–34)

**Current mismatch:**

| Field | `ProviderEntry` (config.rs) | `ProviderConfig` (provider.rs) |
|-------|-----------------------------|-------------------------------|
| id | `name: String` | `id: String` |
| base_url | `base_url: String` | `base_url: String` |
| api_key | `api_key: Option<String>` | `api_key: String` |
| provider_type | `provider_type: String` | `provider_type: ProviderType` (enum) |
| models | `models: Vec<String>` | `models: Vec<String>` |

**Current code (models/config.rs):**
```rust
pub struct ProviderEntry {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub provider_type: String,  // ← String, not ProviderType enum
    pub models: Vec<String>,
}
```

**Recommended code:**
```rust
use crate::tools::provider::ProviderType;

pub struct ProviderEntry {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub provider_type: ProviderType,  // ← use the enum
    pub models: Vec<String>,
}

impl From<ProviderEntry> for crate::tools::provider::ProviderConfig {
    fn from(entry: ProviderEntry) -> Self {
        Self {
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

**Note:** `ProviderType` needs `#[serde(rename = "...")]` for all variants that match config strings. Add `serde(tag = "...")` if needed. The existing enum in `provider.rs` already has `#[serde(rename = "...")]` attributes.

**Priority:** P1 — Manual conversion between these two types is error-prone and blocks dynamic provider registration from config.

**Effort:** Low (~20 lines)

**Link:** Best Practices §4.2

---

### P1.3 SSE buffer overflow guard

**File:** `src/gateway/streaming.rs` (L85–86)

**Current code:**
```rust
let mut buf = String::new();  // ← unbounded growth
```

**Recommended code:**
```rust
const MAX_SSE_BUF_SIZE: usize = 1_000_000; // 1 MB
let mut buf = String::new();

// Inside the while-let loop, after each chunk append:
if buf.len() > MAX_SSE_BUF_SIZE {
    return Err("SSE buffer exceeded max size (1 MB)".into());
}
```

Same pattern applies to `run_anthropic_sse_loop`.

**Priority:** P1 — Low probability in production (malformed upstream would need to send >1MB without `\n\n`), but if triggered it causes OOM. Easy to fix.

**Effort:** Low (5 lines across two functions)

**Link:** Best Practices §2.3

---

### P1.4 `#[derive(Clone)]` on `AppState`

**File:** `src/gateway/server.rs` (L17–30)

**Current code:**
```rust
pub struct AppState {
    pub http_client: reqwest::Client,
    pub db: Database,
    pub classifier: Option<()>,
    pub config: CortexConfig,
    pub uptime: Instant,
}
```

**Recommended code:**
```rust
#[derive(Clone)]
pub struct AppState {
    pub http_client: reqwest::Client,
    pub db: Database,
    pub classifier: Option<()>,
    pub config: CortexConfig,
    pub uptime: Instant,
}
```

**Note:** Verify `Database` implements `Clone`. If it doesn't (rusqlite `Connection` is not `Clone`), wrap `db` in `Arc`:
```rust
pub db: Arc<Database>,
```

**Priority:** P1 — Required for the extension system planned in Phase 2. Each extension gets its own reference to state. With `Arc<AppState>` everywhere this is technically cosmetic, but `#[derive(Clone)]` documents intent and enables patterns like per-request-owned state snapshots.

**Effort:** Low (1 line + possible `Arc` wrap)

**Link:** Best Practices Quick Wins #1

---

## P2 — Nice to Have

---

### P2.1 Remove stub modules or populate them

The following module files contain only placeholder structs:

| File | Content | Action |
|------|---------|--------|
| `src/models/api.rs` | `pub struct ApiPlaceholder;` | Populate with `ChatCompletionRequest` (see P1.1) or remove `pub mod api` from `models/mod.rs` |
| `src/models/types.rs` | `pub struct Placeholder;` | Remove file and `pub mod types` from `models/mod.rs` (content belongs in `api.rs`) |
| `src/tools/logging.rs` | `pub struct LoggingPlaceholder;` | Remove file and `pub mod logging` from `tools/mod.rs` |

**Recommended code (models/mod.rs):**
```rust
pub mod config;
pub mod api;
// pub mod types;  ← REMOVE unless needed
```

**Recommended code (tools/mod.rs):**
```rust
pub mod provider;
pub mod error;
// pub mod logging;  ← REMOVE, add back when real logging is implemented
```

**Priority:** P2 — No functional impact. Dead code adds cognitive load and the placeholders could mislead new contributors into thinking those modules are ready.

**Effort:** Low (remove 3 files, edit 2 mod.rs files)

**Link:** Best Practices §4.1

---

### P2.2 Add `CancellationToken` for client disconnect detection

**File:** `src/gateway/streaming.rs` (L62–75, L175–190)

**Current code:** The `tokio::spawn` tasks in both `stream_openai_response` and `stream_anthropic_response` run until the upstream stream ends. If the downstream client disconnects, the spawned task keeps running — wasting memory and connection slots.

**Recommended code:**
```rust
use tokio_util::sync::CancellationToken;

pub fn stream_openai_response(upstream: Response) -> Sse<impl Stream<Item = SseResult>> {
    let (tx, rx) = mpsc::channel::<SseResult>(64);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    tokio::spawn(async move {
        tokio::select! {
            _ = cancel_clone.cancelled() => {
                debug!("OpenAI SSE: client disconnected");
            }
            result = run_openai_sse_loop(upstream, tx.clone()) => {
                if let Err(e) = result {
                    error!("OpenAI SSE stream error: {}", e);
                    let _ = tx.send(Err(e)).await;
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

**Note:** The `CancellationToken` needs wiring so it fires when the downstream `Sse` stream is dropped. One approach: wrap `Sse` in a custom type that cancels on drop:
```rust
struct CancellableSse<S> {
    inner: Sse<S>,
    cancel: CancellationToken,
}

impl<S> Drop for CancellableSse<S> {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
```

**Dependency change:** `Cargo.toml` — add `tokio-util`:
```toml
tokio-util = { version = "0.7", features = ["rt"] }
```

**Priority:** P2 — Real problem in production (orphaned tasks accumulate), but the existing mpsc channel already provides backpressure: when the client disconnects, `tx.send()` starts failing, eventually stopping the loop. The `CancellationToken` approach is cleaner but not urgent.

**Effort:** Medium (~20 lines + dependency + drop wrapper)

**Link:** Best Practices §2.2

---

## Summary by Effort

| Priority | Suggestion | Effort | Dependencies |
|----------|-----------|--------|-------------|
| **P0** | TraceLayer | Low | +"trace" tower-http feature |
| **P0** | SSE keepalive | Low | none |
| **P0** | DefaultBodyLimit | Low | none |
| **P0** | Replace unwrap/expect | Medium | +anyhow |
| **P1** | Typed ChatCompletionRequest | Medium | none |
| **P1** | From<ProviderEntry> for ProviderConfig | Low | none |
| **P1** | SSE buffer overflow guard | Low | none |
| **P1** | #[derive(Clone)] on AppState | Low | maybe Arc<Database> |
| **P2** | Remove stub modules | Low | none |
| **P2** | CancellationToken select! | Medium | +tokio-util |

---

## Required dependency changes

```toml
# Cargo.toml — additions needed across suggestions
anyhow = "1"                                    # P0.4
tokio-util = { version = "0.7", features = ["rt"] }  # P2.2

# Modified:
tower-http = { version = "0.6", features = ["cors", "trace"] }  # P0.1 (+trace)
```
