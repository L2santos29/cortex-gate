// ====================================================================
// EventBus — Cross-extension communication
// ====================================================================

export class EventBus {
  constructor() {
    this._listeners = {};
    this._idCounter = 1;
  }

  on(event, handler) {
    const id = this._idCounter++;
    (this._listeners[event] ??= []).push({ id, handler });
    // Return unsubscribe function
    return () => this.off(event, id);
  }

  off(event, id) {
    if (this._listeners[event]) {
      this._listeners[event] = this._listeners[event].filter(l => l.id !== id);
    }
  }

  emit(event, data) {
    (this._listeners[event] ?? []).forEach(l => {
      try { l.handler(data); } catch (e) {
        console.warn(`EventBus handler for "${event}" threw:`, e);
      }
    });
  }

  clear() {
    this._listeners = {};
  }
}
