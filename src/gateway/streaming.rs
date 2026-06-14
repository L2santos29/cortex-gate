//! SSE streaming — proxy upstream streaming responses to downstream HTTP clients.
//!
//! # Architecture
//!
//! Each `stream_*_response` function takes a [`reqwest::Response`] from the
//! upstream (obtained via [`ProxyEngine::forward_chat_completion_raw`]) and
//! converts it into an SSE stream of [`axum::response::sse::Event`].
//!
//! ## Common pattern
//!
//! 1. Extract `bytes_stream()` from the reqwest response.
//! 2. `tokio::spawn` a background reader task.
//! 3. Parse SSE frames (`data: ...\n\n`).
//! 4. Transform events to OpenAI-compatible format (if needed).
//! 5. Push events through a [`tokio::sync::mpsc`] channel (buffer 64).
//! 6. Convert the receiver into a [`futures::stream::Stream`] via `unfold`.
//! 7. Return `Sse::new(stream)` with keepalive.
//!
//! ## Backpressure
//!
//! The `mpsc::channel(64)` provides backpressure: if the downstream client
//! consumes slower than the upstream produces, the channel fills up and
//! `try_send` falls back to async `send`, naturally throttling the reader.
//!
//! ## Supported providers
//!
//! | Provider   | Upstream SSE format       | Transformation |
//! |------------|---------------------------|----------------|
//! | OpenAI     | `data: {...}\n\n`         | Passthrough    |
//! | OpenRouter | `data: {...}\n\n`         | Passthrough    |
//! | Anthropic  | Events (content_block_*)  | → OpenAI delta |

use axum::response::sse::{Event, KeepAlive, Sse};
use bytes::Bytes;
use futures::stream::{unfold, Stream, StreamExt};
use reqwest::Response;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error};

/// Maximum SSE buffer size per stream (1 MB).
/// Prevents OOM from malicious or misconfigured upstreams.
const MAX_SSE_BUF_SIZE: usize = 1_000_000;

/// Alias for the SSE stream item type.
type SseResult = Result<Event, axum::BoxError>;

// ===========================================================================
// OpenAI / OpenRouter — passthrough SSE
// ===========================================================================

/// Convert an **OpenAI** streaming response to SSE events.
///
/// Parses `data: {...}\n\n` frames and re-emits them. Terminates on
/// `data: [DONE]\n\n` or connection close.
///
/// Includes a 15-second keepalive to prevent proxy timeouts.
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

/// Alias for [`stream_openai_response`]; OpenRouter uses the same SSE format.
pub fn stream_openrouter_response(upstream: Response) -> Sse<impl Stream<Item = SseResult>> {
    stream_openai_response(upstream)
}

/// Internal loop: read bytes, parse SSE frames, push into channel.
async fn run_openai_sse_loop(
    upstream: Response,
    tx: mpsc::Sender<SseResult>,
) -> Result<(), axum::BoxError> {
    let mut byte_stream = upstream.bytes_stream();
    let mut buf = String::new();
    let mut event_id: u64 = 0;

    while let Some(chunk) = byte_stream.next().await {
        let chunk: Bytes = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // SSE buffer overflow guard
        if buf.len() > MAX_SSE_BUF_SIZE {
            return Err("SSE buffer exceeded max size (1 MB)".into());
        }

        loop {
            let pos = match buf.find("\n\n") {
                Some(p) => p,
                None => break,
            };

            let block = buf[..pos].to_string();
            buf = buf[pos + 2..].to_string();

            let data = block
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .unwrap_or("")
                .to_string();

            if data.is_empty() {
                continue;
            }

            if data.trim() == "[DONE]" {
                debug!("OpenAI SSE: received [DONE], terminating");
                return Ok(());
            }

            event_id += 1;
            let event = Event::default()
                .id(format!("ev_{}", event_id))
                .data(data);

            if tx.send(Ok(event)).await.is_err() {
                debug!("OpenAI SSE: client disconnected");
                return Ok(());
            }
        }
    }

    debug!("OpenAI SSE: upstream closed");
    Ok(())
}

// ===========================================================================
// Anthropic — native SSE events → OpenAI delta format
// ===========================================================================

/// Translate an **Anthropic** streaming response to OpenAI-compatible SSE.
///
/// Anthropic uses native SSE events:
/// ```text
/// event: content_block_delta
/// data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}
/// ```
///
/// Converts to OpenAI chunk format:
/// ```text
/// data: {"choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}
/// ```
///
/// Includes a 15-second keepalive.
pub fn stream_anthropic_response(upstream: Response) -> Sse<impl Stream<Item = SseResult>> {
    let (tx, rx) = mpsc::channel::<SseResult>(64);

    tokio::spawn(async move {
        if let Err(e) = run_anthropic_sse_loop(upstream, tx.clone()).await {
            error!("Anthropic SSE translation error: {}", e);
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

/// Internal state for Anthropic → OpenAI translation.
struct AnthropicState {
    message_id: String,
    model: String,
    created: i64,
    seq: u64,
    has_sent_role: bool,
}

impl AnthropicState {
    fn new() -> Self {
        Self {
            message_id: String::new(),
            model: String::new(),
            created: chrono::Utc::now().timestamp(),
            seq: 0,
            has_sent_role: false,
        }
    }

    fn make_chunk(&mut self, delta: Value, finish: Option<&str>) -> Event {
        self.seq += 1;
        let chunk = serde_json::json!({
            "id": format!("chatcmpl-{}", &self.message_id[..self.message_id.len().min(12)]),
            "object": "chat.completion.chunk",
            "created": self.created,
            "model": self.model,
            "choices": [{
                "index": 0,
                "delta": delta,
                "finish_reason": finish,
            }],
        });
        Event::default()
            .id(format!("ev_{}", self.seq))
            .data(serde_json::to_string(&chunk).unwrap_or_default())
    }
}

/// Read Anthropic SSE stream, translate each event, send via channel.
async fn run_anthropic_sse_loop(
    upstream: Response,
    tx: mpsc::Sender<SseResult>,
) -> Result<(), axum::BoxError> {
    let mut byte_stream = upstream.bytes_stream();
    let mut buf = String::new();
    let mut state = AnthropicState::new();
    let mut current_event_type = String::new();

    while let Some(chunk) = byte_stream.next().await {
        let chunk: Bytes = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // SSE buffer overflow guard
        if buf.len() > MAX_SSE_BUF_SIZE {
            return Err("SSE buffer exceeded max size (1 MB)".into());
        }

        loop {
            let pos = match buf.find("\n\n") {
                Some(p) => p,
                None => break,
            };

            let block = buf[..pos].to_string();
            buf = buf[pos + 2..].to_string();

            // Parse event type line
            for line in block.lines() {
                if let Some(val) = line.strip_prefix("event: ") {
                    current_event_type = val.trim().to_string();
                }
            }

            // Parse data line
            let data_str = block
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .unwrap_or("")
                .to_string();

            if data_str.is_empty() || current_event_type.is_empty() {
                continue;
            }

            let data: Value = match serde_json::from_str(&data_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            match current_event_type.as_str() {
                "message_start" => {
                    if let Some(msg) = data.get("message") {
                        state.message_id = msg
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("msg_unknown")
                            .to_string();
                        state.model = msg
                            .get("model")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                    }
                    let ev = state.make_chunk(
                        serde_json::json!({"role": "assistant", "content": ""}),
                        None,
                    );
                    state.has_sent_role = true;
                    if tx.send(Ok(ev)).await.is_err() {
                        return Ok(());
                    }
                }

                "content_block_delta" => {
                    if let Some(delta) = data.get("delta") {
                        match delta.get("type").and_then(|t| t.as_str()) {
                            Some("text_delta") => {
                                let text = delta
                                    .get("text")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                if !text.is_empty() {
                                    let ev = state.make_chunk(
                                        serde_json::json!({"content": text}),
                                        None,
                                    );
                                    if tx.send(Ok(ev)).await.is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                            Some("input_json_delta") => {
                                let partial = delta
                                    .get("partial_json")
                                    .and_then(|p| p.as_str())
                                    .unwrap_or("");
                                if !partial.is_empty() {
                                    let ev = state.make_chunk(
                                        serde_json::json!({
                                            "tool_calls": [{
                                                "index": 0,
                                                "function": {"arguments": partial}
                                            }]
                                        }),
                                        None,
                                    );
                                    if tx.send(Ok(ev)).await.is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                "content_block_start" => {
                    if let Some(block) = data.get("content_block") {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            let id = block
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("toolu_unknown");
                            let name = block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let ev = state.make_chunk(
                                serde_json::json!({
                                    "tool_calls": [{
                                        "index": 0,
                                        "id": id,
                                        "type": "function",
                                        "function": {"name": name, "arguments": ""}
                                    }]
                                }),
                                None,
                            );
                            if tx.send(Ok(ev)).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                }

                "message_delta" => {
                    let stop_reason = data
                        .get("delta")
                        .and_then(|d| d.get("stop_reason"))
                        .and_then(|r| r.as_str())
                        .map(|r| match r {
                            "end_turn" => "stop",
                            "tool_use" => "tool_calls",
                            "max_tokens" => "length",
                            _ => "stop",
                        });
                    let ev = state.make_chunk(serde_json::json!({}), stop_reason);
                    if tx.send(Ok(ev)).await.is_err() {
                        return Ok(());
                    }
                }

                "message_stop" => {
                    let ev = Event::default().data("[DONE]");
                    let _ = tx.send(Ok(ev)).await;
                    return Ok(());
                }

                _ => {} // ping, content_block_stop → silent
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_sse_buf_size_defined() {
        assert!(MAX_SSE_BUF_SIZE > 0);
        assert_eq!(MAX_SSE_BUF_SIZE, 1_000_000);
    }
}
