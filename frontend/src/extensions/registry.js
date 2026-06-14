// ====================================================================
// ExtensionRegistry — Full lifecycle management
// ====================================================================

import { EventBus } from "./event-bus.js";

class ExtensionRegistry {
  constructor() {
    this.extensions = new Map();
    this.pages = new Map();
    this._builtinPages = [];
    this.hooks = { beforeCommand: [], afterPageLoad: [] };
    this.eventBus = new EventBus();
    this._reloadListeners = [];
  }

  register(ext) {
    if (this.extensions.has(ext.id)) {
      console.warn(`Extension "${ext.id}" already registered`);
      return;
    }

    this.extensions.set(ext.id, ext);

    // Register pages
    if (ext.pages) {
      for (const p of ext.pages) {
        this.pages.set(p.name, { extId: ext.id, ...p });
      }
    }

    // Register hooks
    if (ext.hooks) {
      if (ext.hooks.onBeforeCommand) this.hooks.beforeCommand.push(ext.hooks.onBeforeCommand);
      if (ext.hooks.onAfterPageLoad) this.hooks.afterPageLoad.push(ext.hooks.onAfterPageLoad);
    }

    // Load saved state
    const saved = localStorage.getItem(`cg:ext:${ext.id}`);
    ext.enabled = saved !== null ? saved === "true" : ext.enabledDefault !== false;

    // Call onInit if present
    if (typeof ext.onInit === "function") {
      try {
        ext.onInit(this._createContext(ext.id));
      } catch (e) {
        console.warn(`Extension "${ext.id}" onInit error:`, e);
      }
    }

    this.eventBus.emit("extension:registered", { id: ext.id });
    this._notifyReload();
  }

  unregister(id) {
    const ext = this.extensions.get(id);
    if (!ext) return;

    // Remove pages
    if (ext.pages) {
      for (const p of ext.pages) {
        this.pages.delete(p.name);
      }
    }

    // Remove hooks
    if (ext.hooks) {
      if (ext.hooks.onBeforeCommand) {
        this.hooks.beforeCommand = this.hooks.beforeCommand.filter(h => h !== ext.hooks.onBeforeCommand);
      }
      if (ext.hooks.onAfterPageLoad) {
        this.hooks.afterPageLoad = this.hooks.afterPageLoad.filter(h => h !== ext.hooks.onAfterPageLoad);
      }
    }

    this.extensions.delete(id);
    localStorage.removeItem(`cg:ext:${id}`);
    this.eventBus.emit("extension:unregistered", { id });
    this._notifyReload();
  }

  defineBuiltinPages(pages) {
    this._builtinPages = pages;
  }

  get enabledPages() {
    const result = [];
    for (const [name, p] of this.pages) {
      const ext = this.extensions.get(p.extId);
      if (ext && ext.enabled) result.push({ name, label: p.label, icon: p.icon });
    }
    return result;
  }

  get allPages() {
    return [...this._builtinPages, ...this.enabledPages];
  }

  getAllExtensions() {
    return Array.from(this.extensions.values()).map(e => ({
      id: e.id,
      name: e.name,
      description: e.description,
      version: e.version,
      author: e.author,
      pages: (e.pages || []).map(p => ({ name: p.name, label: p.label })),
      enabled: e.enabled,
    }));
  }

  getExtension(id) {
    const ext = this.extensions.get(id);
    if (!ext) return null;
    return {
      id: ext.id,
      name: ext.name,
      description: ext.description,
      version: ext.version,
      author: ext.author,
      pages: (ext.pages || []).map(p => ({ name: p.name, label: p.label })),
      enabled: ext.enabled,
    };
  }

  toggleExtension(id, enabled) {
    const ext = this.extensions.get(id);
    if (!ext) return;

    ext.enabled = enabled;
    localStorage.setItem(`cg:ext:${id}`, JSON.stringify(enabled));

    // Call lifecycle hooks
    if (enabled && typeof ext.onEnable === "function") {
      try { ext.onEnable(); } catch (e) { console.warn(`Extension "${id}" onEnable error:`, e); }
    }
    if (!enabled && typeof ext.onDisable === "function") {
      try { ext.onDisable(); } catch (e) { console.warn(`Extension "${id}" onDisable error:`, e); }
    }

    this.eventBus.emit(enabled ? "extension:enabled" : "extension:disabled", { id });
    this._notifyReload();

    // Redirect if current page is from this extension and we disabled it
    if (!enabled) {
      const current = window.location.hash.replace("#", "") || "extensions";
      for (const [name, p] of this.pages) {
        if (p.extId === id && name === current) {
          window.location.hash = "#extensions";
          return;
        }
      }
    }
  }

  async loadPageModule(name) {
    const pageInfo = this.pages.get(name);
    if (pageInfo) {
      const ext = this.extensions.get(pageInfo.extId);
      if (ext && ext.enabled && typeof pageInfo.load === "function") {
        return await pageInfo.load();
      }
    }
    return null;
  }

  onReload(fn) {
    this._reloadListeners.push(fn);
  }

  _createContext(extId) {
    return {
      extensionId: extId,
      eventBus: this.eventBus,
      storage: {
        get(key) { try { return JSON.parse(localStorage.getItem(`cg:ext:${extId}:${key}`)); } catch { return null; } },
        set(key, val) { localStorage.setItem(`cg:ext:${extId}:${key}`, JSON.stringify(val)); },
        remove(key) { localStorage.removeItem(`cg:ext:${extId}:${key}`); },
      },
    };
  }

  _notifyReload() {
    this._reloadListeners.forEach(fn => { try { fn(); } catch {} });
  }
}

export const registry = new ExtensionRegistry();
