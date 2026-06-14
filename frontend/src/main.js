// ====================================================================
// Cortex Gate — Extensible Platform Shell
// Extension registry + SPA router + backend bridge
// ====================================================================

import { invoke } from "@tauri-apps/api/core";

// ---- Gateway HTTP fallback ----
const GATEWAY_BASE = "http://127.0.0.1:18801";

// ---- Icon SVGs for nav (simplified paths for leaner bundle) ----
const ICONS = {
  puzzle:
    '<path stroke-linecap="round" stroke-linejoin="round" d="M13.5 16.5h-3m0 0a1.5 1.5 0 0 1-1.5-1.5v-3a1.5 1.5 0 0 1 1.5-1.5m0 0V6.75a.75.75 0 0 1 .75-.75h6a.75.75 0 0 1 .75.75v3.75m-7.5 0a1.5 1.5 0 0 1 1.5 1.5v3a1.5 1.5 0 0 1-1.5 1.5m0 0v3.75a.75.75 0 0 1-.75.75h-6a.75.75 0 0 1-.75-.75v-6a.75.75 0 0 1 .75-.75h.75m7.5 0h.75a.75.75 0 0 1 .75.75v.75"/>',
  equalizer:
    '<path stroke-linecap="round" stroke-linejoin="round" d="M10.5 6h9.75M10.5 6a1.5 1.5 0 1 1-3 0m3 0a1.5 1.5 0 1 0-3 0M3.75 6H7.5m3 12h9.75m-9.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-3.75 0H7.5m9-6h3.75m-3.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-9.75 0h9.75"/>',
};

// ====================================================================
// EventBus — cross-extension pub/sub
// ====================================================================

class EventBus {
  constructor() {
    this._listeners = new Map();
    this._id = 1;
  }

  on(event, handler) {
    const id = this._id++;
    if (!this._listeners.has(event)) this._listeners.set(event, new Map());
    this._listeners.get(event).set(id, handler);
    return () => this._listeners.get(event)?.delete(id);
  }

  emit(event, data) {
    this._listeners.get(event)?.forEach((h) => h(data));
  }
}

const eventBus = new EventBus();

// ====================================================================
// Extension Context — provided via init(ctx)
// ====================================================================

class ExtensionContext {
  constructor(extId, opts = {}) {
    this.id = extId;
    this.eventBus = opts.eventBus || eventBus;
    this.showToast = opts.showToast || (() => {});
    this.navigate = opts.navigate || (() => {});
  }

  getStorage(key) {
    const raw = localStorage.getItem(`cg:ext:${this.id}:${key}`);
    return raw ? JSON.parse(raw) : undefined;
  }

  setStorage(key, value) {
    localStorage.setItem(`cg:ext:${this.id}:${key}`, JSON.stringify(value));
  }
}

// ====================================================================
// Extension Registry
// ====================================================================

class ExtensionRegistry {
  constructor() {
    this.extensions = new Map();
    this.pages = new Map();
    this._builtinPages = [];
    this.hooks = { beforeCommand: [], afterPageLoad: [] };
  }

  register(ext, ctx) {
    this.extensions.set(ext.id, ext);
    if (!this._contexts) this._contexts = new Map();
    this._contexts.set(ext.id, ctx);
    if (ext.pages) {
      for (const p of ext.pages) {
        this.pages.set(p.name, { extId: ext.id, ...p });
      }
    }
    if (ext.hooks) {
      if (ext.hooks.onBeforeCommand)
        this.hooks.beforeCommand.push(ext.hooks.onBeforeCommand);
      if (ext.hooks.onAfterPageLoad)
        this.hooks.afterPageLoad.push(ext.hooks.onAfterPageLoad);
    }
    const saved = localStorage.getItem(`cg:ext:${ext.id}`);
    ext.enabled =
      saved !== null ? saved === "true" : ext.enabledDefault !== false;
    if (typeof ext.onInit === "function") {
      try {
        ext.onInit(ctx);
      } catch (e) {
        console.warn(`Extension "${ext.id}" init error:`, e);
      }
    }
  }

  defineBuiltinPages(pages) {
    this._builtinPages = pages;
  }

  get enabledPages() {
    const result = [];
    for (const [name, p] of this.pages) {
      const ext = this.extensions.get(p.extId);
      if (ext && ext.enabled)
        result.push({ name, label: p.label, icon: p.icon });
    }
    return result;
  }

  get allPages() {
    return [...this._builtinPages, ...this.enabledPages];
  }

  getAllExtensions() {
    return Array.from(this.extensions.values()).map((e) => ({
      id: e.id,
      name: e.name,
      description: e.description,
      version: e.version,
      author: e.author,
      enabled: e.enabled,
    }));
  }

  toggleExtension(id, enabled) {
    const ext = this.extensions.get(id);
    if (ext) {
      ext.enabled = enabled;
      localStorage.setItem(`cg:ext:${id}`, JSON.stringify(enabled));
      rebuildNav();
      if (!enabled) {
        const current = window.location.hash.replace("#", "") || "extensions";
        for (const [name, p] of this.pages) {
          if (p.extId === id && name === current) {
            navigate("extensions");
            return;
          }
        }
      }
    }
  }

  async loadPageModule(name) {
    const pageInfo = this.pages.get(name);
    if (pageInfo) {
      const ext = this.extensions.get(pageInfo.extId);
      if (ext && ext.enabled) return await pageInfo.load();
    }
    return null;
  }
}

const registry = new ExtensionRegistry();

// ====================================================================
// Backend Bridge
// ====================================================================

async function tauriCmd(cmd, args = {}) {
  await runHooks("beforeCommand", cmd, args);

  // Server control commands are Tauri-only — never fallback to HTTP
  const SERVER_COMMANDS = ["start_backend", "stop_backend", "get_backend_status", "open_web_ui"];
  const isServerCmd = SERVER_COMMANDS.includes(cmd);

  // Try Tauri invoke first
  try {
    if (window.__TAURI_INTERNALS__) {
      return await invoke(cmd, args);
    }
  } catch (_) {}

  // Server commands that failed in Tauri context should not fallback to HTTP
  if (isServerCmd) {
    throw new Error(`Command "${cmd}" is only available in Tauri desktop mode`);
  }

  // HTTP fallback for API commands
  const method = cmd.startsWith("get_") ? "GET" : "POST";
  let url = `${GATEWAY_BASE}/api/${cmd.replace(/_/g, "-")}`;
  const opts = { headers: { "Content-Type": "application/json" } };

  if (method === "GET" && Object.keys(args).length > 0) {
    url += `?${new URLSearchParams(args)}`;
  }
  if (method === "POST") {
    opts.method = "POST";
    opts.body = JSON.stringify(args);
  }

  const res = await fetch(url, opts);
  if (!res.ok) {
    const err = await res.text().catch(() => "Unknown error");
    throw new Error(`Gateway error (${res.status}): ${err}`);
  }
  return res.json();
}

async function runHooks(name, ...args) {
  for (const hook of registry.hooks[name] || []) {
    try {
      await hook(...args);
    } catch (e) {
      console.warn(`Hook ${name} error:`, e);
    }
  }
}

// ====================================================================
// Toast System
// ====================================================================

let toastTimer = null;

function showToast(msg, type = "info") {
  const toast = document.getElementById("toast");
  const icon = document.getElementById("toast-icon");
  const text = document.getElementById("toast-msg");
  if (!toast || !icon || !text) return;

  text.textContent = msg;
  icon.className = "w-2 h-2 rounded-full";
  const dotColors = {
    success: "bg-emerald-500",
    error: "bg-red-500",
    info: "bg-cyan-500",
  };
  icon.classList.add(dotColors[type] || dotColors.info);

  const borderColors = {
    success: "border-emerald-200",
    error: "border-red-200",
    info: "border-cyan-200",
  };
  toast.className =
    "fixed bottom-6 right-6 px-4 py-3 rounded-xl bg-white border shadow-lg text-sm text-slate-700 flex items-center gap-3 transition-all duration-300";
  toast.classList.add(borderColors[type] || borderColors.info);

  toast.classList.remove("pointer-events-none", "translate-y-4", "opacity-0");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toast.classList.add("translate-y-4", "opacity-0");
    setTimeout(() => toast.classList.add("pointer-events-none"), 300);
  }, 3000);
}

// ====================================================================
// Modal System
// ====================================================================

function showModal({ title, fields, onSubmit }) {
  const backdrop = document.getElementById("modal-backdrop");
  const box = document.getElementById("modal-box");
  const content = document.getElementById("modal-content");
  if (!backdrop || !box || !content) return Promise.resolve(null);

  let html = `<div class="modal-header"><h3 class="text-lg font-semibold text-slate-800">${title}</h3></div>`;
  html += `<div class="modal-body space-y-3">`;

  const inputs = [];
  if (fields && fields.length > 0) {
    fields.forEach((f, i) => {
      const value = f.value || "";
      html += `<div>
        <label class="block text-xs font-medium text-slate-500 mb-1">${f.label}</label>
        ${
          f.type === "textarea"
            ? `<textarea id="modal-field-${i}" class="config-input" rows="2" placeholder="${f.placeholder || ""}">${value}</textarea>`
            : `<input id="modal-field-${i}" type="${f.type || "text"}" class="config-input" placeholder="${f.placeholder || ""}" value="${value}" />`
        }
      </div>`;
      inputs.push(i);
    });
  }

  html += `</div>`;
  html += `<div class="modal-footer">
    <button id="modal-cancel" class="btn btn-secondary">Cancel</button>
    <button id="modal-confirm" class="btn btn-primary">${fields?.length ? "Save" : "OK"}</button>
  </div>`;

  content.innerHTML = html;
  backdrop.classList.remove("pointer-events-none", "opacity-0");
  box.classList.remove("scale-95");
  box.classList.add("scale-100");

  setTimeout(() => document.getElementById("modal-field-0")?.focus(), 100);

  return new Promise((resolve) => {
    document
      .getElementById("modal-cancel")
      .addEventListener("click", () => {
        hideModal();
        resolve(null);
      });
    document
      .getElementById("modal-confirm")
      .addEventListener("click", () => {
        const values = inputs.map(
          (i) => document.getElementById(`modal-field-${i}`)?.value || ""
        );
        hideModal();
        resolve(values);
      });
    backdrop.addEventListener("click", (e) => {
      if (e.target === backdrop) {
        hideModal();
        resolve(null);
      }
    });
    document.addEventListener("keydown", function escHandler(e) {
      if (e.key === "Escape") {
        hideModal();
        resolve(null);
        document.removeEventListener("keydown", escHandler);
      }
    });
  });
}

function hideModal() {
  const backdrop = document.getElementById("modal-backdrop");
  const box = document.getElementById("modal-box");
  backdrop.classList.add("opacity-0", "pointer-events-none");
  box.classList.add("scale-95");
  box.classList.remove("scale-100");
}

// ====================================================================
// SPA Router
// ====================================================================

const injectedStyles = new Set();
let currentPage = null;

function navigate(name) {
  window.location.hash = `#${name}`;
}

async function loadPage(name) {
  if (name === currentPage) return;
  currentPage = name;

  const app = document.getElementById("app");
  app.innerHTML = `<div class="flex items-center justify-center py-20"><div class="skeleton w-10 h-10 rounded-full"></div></div>`;

  try {
    let pageModule = null;

    // 1. Try extension-registered pages
    pageModule = await registry.loadPageModule(name);

    // 2. Try built-in pages
    if (!pageModule) {
      try {
        pageModule = await import(`./pages/${name}.js`);
      } catch (_) {}
    }

    if (!pageModule) throw new Error(`Page "${name}" not found`);

    const { html = "", css = "", init } = pageModule;

    // Inject page-specific CSS once
    if (css && !injectedStyles.has(name)) {
      const styleEl = document.createElement("style");
      styleEl.textContent = css;
      document.head.appendChild(styleEl);
      injectedStyles.add(name);
    }

    app.innerHTML = `<div class="page-enter">${html}</div>`;

    if (typeof init === "function") {
      requestAnimationFrame(() => {
        // Pass context if page init accepts it (determined by arity)
        const ctx = registry._contexts?.get(
          [...(registry.pages?.entries() || [])].find(([_, p]) => p.name === name)?.[1]?.extId
        );
        init(ctx);
      });
    }

    // Update active nav button
    document.querySelectorAll(".nav-btn").forEach((btn) => {
      btn.classList.toggle("active", btn.dataset.page === name);
    });

    await runHooks("afterPageLoad", name);
  } catch (err) {
    console.error(`Failed to load page "${name}":`, err);
    app.innerHTML = `
      <div class="flex flex-col items-center justify-center py-20 text-slate-400">
        <svg class="w-12 h-12 mb-4 text-slate-300" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9 3.75h.008v.008H12v-.008Z"/>
        </svg>
        <p class="text-lg font-medium mb-1 text-slate-500">Error loading ${name}</p>
        <p class="text-sm text-slate-400">${err.message}</p>
        <button onclick="location.reload()" class="btn btn-secondary mt-6">Reload</button>
      </div>`;
  }
}

// ====================================================================
// Navigation Builder
// ====================================================================

function rebuildNav() {
  const nav = document.getElementById("sidebar-nav");
  if (!nav) return;
  nav.innerHTML = "";

  for (const page of registry.allPages) {
    const btn = document.createElement("button");
    btn.dataset.page = page.name;
    btn.className =
      "nav-btn w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium text-slate-500 hover:text-slate-700 hover:bg-slate-100 transition-all duration-150";
    btn.innerHTML = `<svg class="w-5 h-5 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">${
      ICONS[page.icon] || ICONS.puzzle
    }</svg><span>${page.label}</span>`;
    btn.addEventListener("click", () => navigate(page.name));
    nav.appendChild(btn);
  }

  // Highlight current
  const current = window.location.hash.replace("#", "") || "extensions";
  nav.querySelector(`[data-page="${current}"]`)?.classList.add("active");
}

// ====================================================================
// URL Routing
// ====================================================================

window.addEventListener("hashchange", () => {
  const hash = window.location.hash.replace("#", "") || "extensions";
  const validPages = registry.allPages.map((p) => p.name);
  loadPage(validPages.includes(hash) ? hash : "extensions");
});

// ====================================================================
// Shared UI Helpers
// ====================================================================

function createVerticalSlider(container, opts = {}) {
  const {
    value = 0.5,
    min = 0,
    max = 1,
    step = 0.01,
    color = "#06b6d4",
    onChange = () => {},
  } = opts;

  const wrapper = document.createElement("div");
  wrapper.style.cssText = "position:relative;width:100%;height:100%";

  const track = document.createElement("div");
  track.className = "vertical-slider-track";

  const fill = document.createElement("div");
  fill.className = "vertical-slider-fill";
  fill.style.background = `linear-gradient(to top, ${color}, ${color}dd)`;

  const thumb = document.createElement("div");
  thumb.className = "vertical-slider-thumb";
  thumb.style.background = color;

  track.appendChild(fill);
  track.appendChild(thumb);
  wrapper.appendChild(track);
  container.appendChild(wrapper);

  let currentVal = value;
  let dragging = false;

  function clamp(v) {
    return Math.min(max, Math.max(min, v));
  }

  function setValue(v, fireChange = true) {
    currentVal = clamp(v);
    if (step > 0) currentVal = Math.round(currentVal / step) * step;
    const pct = ((currentVal - min) / (max - min)) * 100;
    fill.style.height = `${pct}%`;
    thumb.style.bottom = `calc(${pct}% - 9px)`;
    if (fireChange) onChange(currentVal);
  }

  function posToValue(e) {
    const rect = track.getBoundingClientRect();
    const y =
      (e.clientY ?? e.touches?.[0]?.clientY ?? 0) - rect.top;
    return (
      min +
      (1 - Math.max(0, Math.min(1, y / rect.height))) * (max - min)
    );
  }

  function onStart(e) {
    dragging = true;
    setValue(posToValue(e));
  }
  function onMove(e) {
    if (dragging) {
      e.preventDefault();
      setValue(posToValue(e));
    }
  }
  function onEnd() {
    dragging = false;
  }

  track.addEventListener("mousedown", onStart);
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onEnd);
  track.addEventListener("touchstart", onStart, { passive: false });
  window.addEventListener("touchmove", onMove, { passive: false });
  window.addEventListener("touchend", onEnd);

  setValue(value, false);

  return {
    setValue,
    getValue: () => currentVal,
    destroy: () => {
      track.removeEventListener("mousedown", onStart);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onEnd);
      track.removeEventListener("touchstart", onStart);
      window.removeEventListener("touchmove", onMove);
      window.removeEventListener("touchend", onEnd);
    },
  };
}

function createHorizontalSlider(container, opts = {}) {
  const {
    value = 0.5,
    min = 0,
    max = 1,
    step = 0.01,
    color = "#f59e0b",
    onChange = () => {},
  } = opts;

  const wrapper = document.createElement("div");
  wrapper.style.cssText = "position:relative;width:100%;padding:4px 0";

  const track = document.createElement("div");
  track.className = "economy-track";

  const fill = document.createElement("div");
  fill.className = "economy-fill";
  fill.style.background = `linear-gradient(to right, #10b981, ${color})`;

  const thumb = document.createElement("div");
  thumb.className = "economy-thumb";

  track.appendChild(fill);
  track.appendChild(thumb);
  wrapper.appendChild(track);
  container.appendChild(wrapper);

  let currentVal = value;
  let dragging = false;

  function clamp(v) {
    return Math.min(max, Math.max(min, v));
  }

  function setValue(v, fireChange = true) {
    currentVal = clamp(v);
    if (step > 0) currentVal = Math.round(currentVal / step) * step;
    const pct = ((currentVal - min) / (max - min)) * 100;
    fill.style.width = `${pct}%`;
    thumb.style.left = `${pct}%`;
    if (fireChange) onChange(currentVal);
  }

  function posToValue(e) {
    const rect = track.getBoundingClientRect();
    const x =
      (e.clientX ?? e.touches?.[0]?.clientX ?? 0) - rect.left;
    return (
      min + Math.max(0, Math.min(1, x / rect.width)) * (max - min)
    );
  }

  function onStart(e) {
    dragging = true;
    setValue(posToValue(e));
  }
  function onMove(e) {
    if (dragging) {
      e.preventDefault();
      setValue(posToValue(e));
    }
  }
  function onEnd() {
    dragging = false;
  }

  track.addEventListener("mousedown", onStart);
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onEnd);
  track.addEventListener("touchstart", onStart, { passive: false });
  window.addEventListener("touchmove", onMove, { passive: false });
  window.addEventListener("touchend", onEnd);

  setValue(value, false);

  return {
    setValue,
    getValue: () => currentVal,
    destroy: () => {
      track.removeEventListener("mousedown", onStart);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onEnd);
      track.removeEventListener("touchstart", onStart);
      window.removeEventListener("touchmove", onMove);
      window.removeEventListener("touchend", onEnd);
    },
  };
}

function formatNum(n) {
  if (!n || isNaN(n)) return "0";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toLocaleString();
}

function debounce(fn, ms = 300) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

// ====================================================================
// Server Control (Tauri backend management)
// ====================================================================

function isTauri() {
  return typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;
}

let serverPollInterval = null;

async function startServer() {
  if (!isTauri()) { showToast("Server control only available in desktop app", "info"); return; }
  try {
    const result = await invoke("start_backend");
    showToast(result || "Server started", "success");
    await updateServerStatus();
  } catch (err) {
    showToast(`Error: ${err}`, "error");
  }
}

async function stopServer() {
  if (!isTauri()) { showToast("Server control only available in desktop app", "info"); return; }
  try {
    const result = await invoke("stop_backend");
    showToast(result || "Server stopped", "success");
    await updateServerStatus();
  } catch (err) {
    showToast(`Error: ${err}`, "error");
  }
}

async function openWebUI() {
  const url = "http://127.0.0.1:18801";
  if (isTauri()) {
    try {
      await invoke("open_web_ui");
      return;
    } catch (_) {}
  }
  window.open(url, "_blank");
}

async function updateServerStatus() {
  const dot = document.getElementById("server-dot");
  const text = document.getElementById("server-status-text");
  const startBtn = document.getElementById("btn-start-server");
  const stopBtn = document.getElementById("btn-stop-server");
  const webBtn = document.getElementById("btn-open-web");

  if (!dot || !text) return;

  // In browser mode: hide Tauri buttons, show info
  if (!isTauri()) {
    dot.className = "w-2 h-2 rounded-full bg-cyan-400";
    text.textContent = "Web Mode";
    if (startBtn) startBtn.classList.add("hidden");
    if (stopBtn) stopBtn.classList.add("hidden");
    if (webBtn) webBtn.textContent = "Open App";
    return;
  }

  // Tauri mode: check backend health
  try {
    const healthResp = await fetch("http://127.0.0.1:18801/health", { method: "GET", signal: AbortSignal.timeout(2000) });
    if (healthResp.ok) {
      dot.className = "w-2 h-2 rounded-full bg-emerald-500 shadow-sm shadow-emerald-500/30";
      text.textContent = "Server Online";
      if (startBtn) startBtn.classList.add("hidden");
      if (stopBtn) stopBtn.classList.remove("hidden");
      return;
    }
  } catch (_) {}

  // Server not responding
  dot.className = "w-2 h-2 rounded-full bg-slate-300";
  text.textContent = "Server Offline";
  if (startBtn) startBtn.classList.remove("hidden");
  if (stopBtn) stopBtn.classList.add("hidden");
}

function initServerControl() {
  const startBtn = document.getElementById("btn-start-server");
  const stopBtn = document.getElementById("btn-stop-server");
  const webBtn = document.getElementById("btn-open-web");

  if (startBtn) startBtn.addEventListener("click", startServer);
  if (stopBtn) stopBtn.addEventListener("click", stopServer);
  if (webBtn) webBtn.addEventListener("click", openWebUI);

  updateServerStatus();
  serverPollInterval = setInterval(updateServerStatus, 5000);
}

// ====================================================================
// Init
// ====================================================================

async function init() {
  // Register built-in pages
  registry.defineBuiltinPages([
    { name: "extensions", label: "Extensions", icon: "puzzle" },
  ]);

  // Auto-discover extensions via Vite glob
  const extModules = import.meta.glob("./extensions/*/manifest.js", { eager: true });
  for (const [path, mod] of Object.entries(extModules)) {
    if (mod.manifest) {
      const extCtx = new ExtensionContext(mod.manifest.id, {
        showToast,
        navigate: (name) => { window.location.hash = `#${name}`; },
      });
      registry.register(mod.manifest, extCtx);
    }
  }

  // Build nav
  rebuildNav();

  // Init server control
  initServerControl();

  // Start router
  const hash = window.location.hash.replace("#", "") || "extensions";
  const validPages = registry.allPages.map((p) => p.name);
  loadPage(validPages.includes(hash) ? hash : "extensions");
}

// Expose public API globally for page modules
window.__cg = {
  registry,
  tauriCmd,
  showToast,
  showModal,
  navigate,
  createVerticalSlider,
  createHorizontalSlider,
  formatNum,
  debounce,
};

document.addEventListener("DOMContentLoaded", init);
