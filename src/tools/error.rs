// Proxy Error Types — Unified error handling for upstream provider requests.
//
// Covers reqwest failures, upstream HTTP errors, SSE parse failures,
// and unknown provider lookups. All variants implement `std::error::Error`
// via thiserror so they plug into any `Into<BoxError>` boundary (axum,
// tower, etc.).

use thiserror::Error;

/// Errors originating from the multi-provider proxy layer.
#[derive(Debug, Error)]
pub enum ProxyError {
    /// The requested provider name does not exist in the engine.
    #[error("unknown provider: {0}")]
    UnknownProvider(String),

    /// A reqwest transport or protocol-level failure.
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// The upstream API responded with a non-2xx status.
    #[error("upstream returned {status}: {body}")]
    UpstreamError {
        status: reqwest::StatusCode,
        body: String,
    },

    /// Malformed or unparseable SSE data from the upstream.
    #[error("SSE parse error at line {line}: {detail}")]
    SseParse {
        line: usize,
        detail: String,
    },

    /// Input validation or conversion failed (messages, model, etc.).
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// Serialization or deserialization of JSON payloads.
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    /// The internal mpsc channel was closed before the stream completed.
    #[error("channel closed: {0}")]
    ChannelClosed(String),

    /// Catch-all for stream processing errors.
    #[error("stream error: {0}")]
    Stream(String),
}

// ---------------------------------------------------------------------------
// Convenience type aliases
// ---------------------------------------------------------------------------

/// Short-hand for proxy results.
pub type ProxyResult<T> = Result<T, ProxyError>;
