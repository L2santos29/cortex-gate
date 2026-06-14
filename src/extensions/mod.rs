//! Extension system for Cortex Gate.
//!
//! Defines the core traits and types for registering, loading, and
//! managing extensions. Extensions can contribute routes, providers,
//! middleware, and custom Tauri commands.

use axum::routing::MethodRouter;
use serde::{Deserialize, Serialize};

pub mod context;
pub mod event_bus;
pub mod manager;
pub mod provider_plugin;

pub use context::ExtensionContext;
pub use event_bus::EventBus;
pub use manager::ExtensionManager;
pub use provider_plugin::ProviderPlugin;

// ---------------------------------------------------------------------------
// Extension manifest
// ---------------------------------------------------------------------------

/// Metadata for an extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    /// Unique identifier (e.g., "com.cortex.prompt-router")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Semantic version
    pub version: String,
    /// Short description
    #[serde(default)]
    pub description: String,
    /// Author information
    #[serde(default)]
    pub author: Option<ExtensionAuthor>,
    /// Minimum compatible gateway version
    #[serde(default = "default_min_version")]
    pub min_app_version: String,
    /// Declared permissions
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Extensions this one depends on
    #[serde(default)]
    pub dependencies: Vec<String>,
}

fn default_min_version() -> String {
    "0.2.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionAuthor {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

impl ExtensionManifest {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            description: String::new(),
            author: None,
            min_app_version: default_min_version(),
            permissions: Vec::new(),
            dependencies: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Extension trait
// ---------------------------------------------------------------------------

/// Core trait implemented by all extensions.
///
/// Provides lifecycle hooks and contribution points.
#[async_trait::async_trait]
pub trait CortexExtension: Send + Sync + 'static {
    /// Extension metadata (id, name, version, permissions)
    fn manifest(&self) -> &ExtensionManifest;

    /// Shortcut for `self.manifest().id`
    fn id(&self) -> &str {
        &self.manifest().id
    }

    /// Called once when extension is loaded (before enable).
    /// Use for validation, config parsing, etc.
    async fn init(&mut self, _ctx: &ExtensionContext) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Called when extension is enabled.
    async fn on_enable(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Called when extension is disabled.
    async fn on_disable(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Custom HTTP routes this extension contributes (e.g., "/ext/my-ext/action").
    fn routes(&self) -> Vec<(String, MethodRouter)> {
        Vec::new()
    }

    /// Custom LLM provider implementations this extension registers.
    fn providers(&self) -> Vec<Box<dyn ProviderPlugin>> {
        Vec::new()
    }

    /// Custom Tower middleware layers.
    fn middleware(&self) -> Vec<Box<dyn tower::Layer<axum::body::Body, Service = axum::body::Body>>> {
        Vec::new()
    }
}
