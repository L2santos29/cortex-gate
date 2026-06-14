//! Provider plugin trait — allows extensions to register new LLM provider types.
//!
//! Without modifying core code, extensions can add support for new providers
//! (Gemini, Cohere, Ollama, etc.) by implementing this trait.

use async_trait::async_trait;
use serde_json::Value;

/// Provider configuration passed to plugin methods.
#[derive(Debug, Clone)]
pub struct ProviderPluginConfig {
    /// Provider identifier
    pub id: String,
    /// Base URL for API calls
    pub base_url: String,
    /// API key
    pub api_key: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

/// Trait for implementing custom LLM providers as extensions.
#[async_trait]
pub trait ProviderPlugin: Send + Sync + 'static {
    /// Unique provider type identifier (e.g., "ollama", "gemini", "cohere")
    fn provider_type(&self) -> &str;

    /// Build a reqwest client with provider-specific headers/tls config.
    async fn build_client(&self, config: &ProviderPluginConfig) -> Result<reqwest::Client, String> {
        Ok(reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| e.to_string())?)
    }

    /// Build the request URL and JSON body for a chat completion call.
    fn build_request(&self, _config: &ProviderPluginConfig, _model: &str, _messages: &Value, _stream: bool) -> (String, Value) {
        (String::new(), Value::Null)
    }

    /// Normalize a provider-specific response to OpenAI-compatible format.
    fn normalize_response(&self, _raw: Value) -> Value {
        Value::Null
    }

    /// Check if this provider is healthy (optional, for health checks).
    async fn health_check(&self, _config: &ProviderPluginConfig) -> Result<bool, String> {
        Ok(true)
    }
}
