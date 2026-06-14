//! Extension manager — lifecycle orchestrator.
//!
//! Manages registration, initialization, and contribution collection
//! for all extensions.

use std::collections::HashMap;

use axum::routing::MethodRouter;
use tracing::info;

use super::context::ExtensionContext;
use super::provider_plugin::ProviderPlugin;
use super::{CortexExtension, ExtensionManifest};

/// Manages all extensions: registration, lifecycle, contribution collection.
pub struct ExtensionManager {
    extensions: HashMap<String, Box<dyn CortexExtension>>,
    enabled: HashMap<String, bool>,
}

impl ExtensionManager {
    /// Create a new, empty extension manager.
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
            enabled: HashMap::new(),
        }
    }

    /// Register a compiled-in extension.
    pub fn register(&mut self, ext: Box<dyn CortexExtension>) {
        let id = ext.id().to_string();
        info!(
            target: "cortex_gate::extensions",
            "Registered extension: {} v{}",
            id,
            ext.manifest().version,
        );
        self.enabled.insert(id.clone(), true);
        self.extensions.insert(id, ext);
    }

    /// Initialize all registered extensions.
    pub async fn init_all(&mut self, ctx: &ExtensionContext) -> Result<(), Vec<(String, Box<dyn std::error::Error>)>> {
        let mut errors = Vec::new();

        for (id, ext) in self.extensions.iter_mut() {
            if !self.enabled.get(id).copied().unwrap_or(false) {
                continue;
            }
            match ext.init(ctx).await {
                Ok(()) => {
                    info!(target: "cortex_gate::extensions", "Initialized extension: {}", id);
                }
                Err(e) => {
                    tracing::error!(target: "cortex_gate::extensions", "Failed to init extension {}: {}", id, e);
                    errors.push((id.clone(), e));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Enable an extension by ID.
    pub async fn enable(&mut self, id: &str) -> Result<(), String> {
        let ext = self.extensions.get_mut(id).ok_or_else(|| format!("Extension '{id}' not found"))?;
        self.enabled.insert(id.to_string(), true);
        ext.on_enable().await.map_err(|e| e.to_string())?;
        info!(target: "cortex_gate::extensions", "Enabled extension: {}", id);
        Ok(())
    }

    /// Disable an extension by ID.
    pub async fn disable(&mut self, id: &str) -> Result<(), String> {
        let ext = self.extensions.get_mut(id).ok_or_else(|| format!("Extension '{id}' not found"))?;
        ext.on_disable().await.map_err(|e| e.to_string())?;
        self.enabled.insert(id.to_string(), false);
        info!(target: "cortex_gate::extensions", "Disabled extension: {}", id);
        Ok(())
    }

    /// Collect all HTTP routes from enabled extensions.
    pub fn collect_routes(&self) -> Vec<(String, MethodRouter)> {
        let mut routes = Vec::new();
        for (id, ext) in &self.extensions {
            if !self.enabled.get(id).copied().unwrap_or(false) {
                continue;
            }
            routes.extend(ext.routes());
        }
        routes
    }

    /// Collect all provider plugins from enabled extensions.
    pub fn collect_providers(&self) -> Vec<Box<dyn ProviderPlugin>> {
        let mut providers = Vec::new();
        for (id, ext) in &self.extensions {
            if !self.enabled.get(id).copied().unwrap_or(false) {
                continue;
            }
            providers.extend(ext.providers());
        }
        providers
    }

    /// Collect all middleware layers from enabled extensions.
    pub fn collect_middleware(&self) -> Vec<Box<dyn tower::Layer<axum::body::Body, Service = axum::body::Body>>> {
        let mut layers = Vec::new();
        for (id, ext) in &self.extensions {
            if !self.enabled.get(id).copied().unwrap_or(false) {
                continue;
            }
            layers.extend(ext.middleware());
        }
        layers
    }

    /// Get extension manifest by ID.
    pub fn get_manifest(&self, id: &str) -> Option<&ExtensionManifest> {
        self.extensions.get(id).map(|ext| ext.manifest())
    }

    /// List all extension manifests with their enabled status.
    pub fn list_extensions(&self) -> Vec<(ExtensionManifest, bool)> {
        self.extensions
            .iter()
            .map(|(id, ext)| (ext.manifest().clone(), *self.enabled.get(id).unwrap_or(&false)))
            .collect()
    }

    /// Check if an extension is enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.get(id).copied().unwrap_or(false)
    }

    /// Get the number of registered extensions.
    pub fn len(&self) -> usize {
        self.extensions.len()
    }

    /// Check if any extensions are registered.
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }
}

impl Default for ExtensionManager {
    fn default() -> Self {
        Self::new()
    }
}
