//! Event bus for cross-extension communication.
//!
//! Extensions communicate through a shared event bus rather than
//! direct imports. This decouples extensions from each other and
//! from the host.

use std::sync::{Arc, RwLock};

use serde_json::Value;

/// A handle returned by `EventBus::on()` that unsubscribes when dropped.
pub struct EventHandle {
    event: String,
    bus: EventBus,
    handler_id: u64,
}

impl Drop for EventHandle {
    fn drop(&mut self) {
        self.bus.remove_listener(&self.event, self.handler_id);
    }
}

struct EventBusInner {
    listeners: RwLock<std::collections::HashMap<String, Vec<(u64, Box<dyn Fn(&Value) + Send + Sync>)>>>,
    next_id: RwLock<u64>,
}

/// Simple event bus for pub/sub communication.
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

impl EventBus {
    /// Create a new event bus.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(EventBusInner {
                listeners: RwLock::new(std::collections::HashMap::new()),
                next_id: RwLock::new(1),
            }),
        }
    }

    /// Emit an event to all listeners.
    pub fn emit(&self, event: &str, data: &Value) {
        if let Ok(listeners) = self.inner.listeners.read() {
            if let Some(handlers) = listeners.get(event) {
                for (_, handler) in handlers {
                    handler(data);
                }
            }
        }
    }

    /// Subscribe to an event. Returns an `EventHandle` that unsubscribes on drop.
    pub fn on<F>(&self, event: &str, handler: F) -> EventHandle
    where
        F: Fn(&Value) + Send + Sync + 'static,
    {
        let mut id = self.inner.next_id.write().unwrap();
        let handler_id = *id;
        *id += 1;

        let mut listeners = self.inner.listeners.write().unwrap();
        listeners
            .entry(event.to_string())
            .or_default()
            .push((handler_id, Box::new(handler)));

        EventHandle {
            event: event.to_string(),
            bus: self.clone(),
            handler_id,
        }
    }

    fn remove_listener(&self, event: &str, handler_id: u64) {
        if let Ok(mut listeners) = self.inner.listeners.write() {
            if let Some(handlers) = listeners.get_mut(event) {
                handlers.retain(|(id, _)| *id != handler_id);
            }
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
