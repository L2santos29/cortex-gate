//! Extension context — what extensions can access from the host.

use std::sync::{Arc, RwLock};

use serde_json::Value;

use crate::governance::Database;
use super::event_bus::EventBus;

/// Context provided to every extension at initialization.
///
/// Controls what the extension can access — follows the principle
/// of least privilege. Extensions only get what's declared in their
/// permissions.
pub struct ExtensionContext {
    /// The extension's unique identifier
    pub id: String,

    /// Extension-scoped configuration (from manifest defaults + user overrides)
    pub config: Arc<RwLock<serde_json::Map<String, Value>>>,

    /// Shared database handle (if permission "db:read" or "db:write" granted)
    pub db: Option<Arc<Database>>,

    /// Shared HTTP client (if permission "network:fetch" granted)
    pub http_client: Option<reqwest::Client>,

    /// Event bus for cross-extension communication
    pub event_bus: Arc<EventBus>,

    /// Directory for extension-scoped file storage
    pub data_dir: Option<std::path::PathBuf>,
}

impl ExtensionContext {
    /// Create a new extension context with minimal access.
    pub fn new(id: &str, event_bus: Arc<EventBus>) -> Self {
        Self {
            id: id.to_string(),
            config: Arc::new(RwLock::new(serde_json::Map::new())),
            db: None,
            http_client: None,
            event_bus,
            data_dir: None,
        }
    }

    /// Enable database access.
    pub fn with_db(mut self, db: Arc<Database>) -> Self {
        self.db = Some(db);
        self
    }

    /// Enable HTTP client access.
    pub fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Set extension data directory.
    pub fn with_data_dir(mut self, path: std::path::PathBuf) -> Self {
        self.data_dir = Some(path);
        self
    }

    /// Set configuration.
    pub fn with_config(mut self, config: serde_json::Map<String, Value>) -> Self {
        self.config = Arc::new(RwLock::new(config));
        self
    }

    /// Read a config value.
    pub fn get_config(&self, key: &str) -> Option<Value> {
        self.config.read().ok()?.get(key).cloned()
    }

    /// Write a config value.
    pub fn set_config(&self, key: &str, value: Value) -> Result<(), String> {
        self.config
            .write()
            .map_err(|e| e.to_string())?
            .insert(key.to_string(), value);
        Ok(())
    }
}
