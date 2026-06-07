//! Proxy Engine — multi-provider request forwarding and response normalization.
//!
//! The [`ProxyEngine`] holds a map of configured providers, each with its own
//! pre-configured [`reqwest::Client`]. It provides methods for:
//!
//! - **`forward_chat_completion`** — Forward + parse JSON (non-streaming)
//! - **`forward_chat_completion_raw`** — Forward + return raw response (streaming)
//! - **`resolve_provider`** — Find which provider serves a given model

use crate::tools::error::ProxyError;
use crate::tools::provider::{build_client, ProviderConfig};
use reqwest::{Client, Response};
use serde_json::Value;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// ProxyEngine
// ---------------------------------------------------------------------------

/// Multi-provider proxy engine.
///
/// Stores pre-configured HTTP clients per provider and resolves models
/// to providers.
pub struct ProxyEngine {
    clients: HashMap<String, (ProviderConfig, Client)>,
}

impl ProxyEngine {
    /// Build a [`ProxyEngine`] from a list of provider configs.
    ///
    /// Each config gets a dedicated [`reqwest::Client`] with appropriate
    /// headers and timeouts for that provider.
    pub fn new(configs: Vec<ProviderConfig>) -> Self {
        let mut clients = HashMap::new();
        for cfg in configs {
            let client = build_client(&cfg);
            clients.insert(cfg.id.clone(), (cfg, client));
        }
        Self { clients }
    }

    /// Check if a provider exists in the engine.
    pub fn has_provider(&self, provider_id: &str) -> bool {
        self.clients.contains_key(provider_id)
    }

    /// Number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.clients.len()
    }

    /// List all registered provider IDs.
    pub fn provider_ids(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    /// Resolve provider by model name (exact match on model list).
    pub fn resolve_provider(&self, model_name: &str) -> Option<&ProviderConfig> {
        self.clients
            .values()
            .find(|(cfg, _)| cfg.has_model(model_name))
            .map(|(cfg, _)| cfg)
    }

    /// Get provider config by ID.
    pub fn get_provider(&self, provider_id: &str) -> Option<&ProviderConfig> {
        self.clients.get(provider_id).map(|(cfg, _)| cfg)
    }

    // ------------------------------------------------------------------
    // Forwarding — non-streaming
    // ------------------------------------------------------------------

    /// Forward a chat completion request and return the parsed JSON response.
    pub async fn forward_chat_completion(
        &self,
        provider_id: &str,
        body: Value,
    ) -> Result<Value, ProxyError> {
        let (config, client) = self
            .clients
            .get(provider_id)
            .ok_or_else(|| ProxyError::UnknownProvider(provider_id.to_string()))?;

        let url = format!(
            "{}/v1/chat/completions",
            config.base_url.trim_end_matches('/')
        );

        let response = client.post(&url).json(&body).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(ProxyError::UpstreamError {
                status,
                body: error_body,
            });
        }

        Ok(response.json::<Value>().await?)
    }

    // ------------------------------------------------------------------
    // Forwarding — streaming (raw HTTP response)
    // ------------------------------------------------------------------

    /// Forward a chat completion request and return the raw HTTP response
    /// for SSE streaming consumption.
    pub async fn forward_chat_completion_raw(
        &self,
        provider_id: &str,
        body: Value,
    ) -> Result<Response, ProxyError> {
        let (config, client) = self
            .clients
            .get(provider_id)
            .ok_or_else(|| ProxyError::UnknownProvider(provider_id.to_string()))?;

        let url = format!(
            "{}/v1/chat/completions",
            config.base_url.trim_end_matches('/')
        );

        let response = client.post(&url).json(&body).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(ProxyError::UpstreamError {
                status,
                body: error_body,
            });
        }

        Ok(response)
    }

    /// Convenience: build body, forward, and normalize response.
    pub async fn chat(
        &self,
        provider_id: &str,
        model: &str,
        messages: Value,
        stream: bool,
    ) -> Result<Value, ProxyError> {
        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": stream,
        });
        let raw = self.forward_chat_completion(provider_id, body).await?;

        // Normalize Anthropic response to OpenAI format
        if let Some((config, _)) = self.clients.get(provider_id) {
            Ok(crate::tools::provider::normalize_response(
                &config.provider_type,
                raw,
            ))
        } else {
            Ok(raw)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::provider::ProviderType;

    fn test_cfg(id: &str) -> ProviderConfig {
        ProviderConfig {
            id: id.to_string(),
            base_url: format!("https://{}.example.com/v1", id),
            api_key: "sk-test".into(),
            provider_type: ProviderType::OpenAI,
            models: vec!["gpt-4".into(), "gpt-3.5-turbo".into()],
            default_model: Some("gpt-3.5-turbo".into()),
            timeout_secs: 30,
            max_retries: 0,
        }
    }

    #[test]
    fn test_resolve_provider() {
        let configs = vec![
            test_cfg("openai"),
            ProviderConfig {
                id: "anthropic".into(),
                base_url: "https://api.anthropic.com".into(),
                api_key: "sk-ant".into(),
                provider_type: ProviderType::Anthropic,
                models: vec!["claude-3-opus".into()],
                default_model: None,
                timeout_secs: 60,
                max_retries: 2,
            },
        ];
        let engine = ProxyEngine::new(configs);
        assert_eq!(engine.resolve_provider("gpt-4").unwrap().id, "openai");
        assert_eq!(
            engine.resolve_provider("claude-3-opus").unwrap().id,
            "anthropic"
        );
        assert!(engine.resolve_provider("no-such-model").is_none());
    }

    #[test]
    fn test_provider_count() {
        let engine = ProxyEngine::new(vec![test_cfg("a"), test_cfg("b")]);
        assert_eq!(engine.provider_count(), 2);
        assert!(engine.has_provider("a"));
        assert!(!engine.has_provider("c"));
    }

    #[test]
    fn test_get_provider() {
        let engine = ProxyEngine::new(vec![test_cfg("openai")]);
        let cfg = engine.get_provider("openai").unwrap();
        assert_eq!(cfg.id, "openai");
    }
}
