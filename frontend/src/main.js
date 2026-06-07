// ==========================================================================
// Cortex Gate — Router principal (SPA ligero) + Tauri invoke wrapper
// ==========================================================================

import { invoke } from "@tauri-apps/api/core";

// ---- Gateway HTTP fallback (when not running inside Tauri) ----
const GATEWAY_BASE = "http://127.0.0.1:18801";

/**
 * Llama a un comando Tauri. Si no está disponible (dev en navegador),
 * fallback a HTTP directo al gateway.
 */
export async function tauriCmd(cmd, args = {}) {
  // Try Tauri invoke first
  try {
    if (window.__TAURI_INTERNALS__) {
      return await invoke(cmd, args);
    }
  } catch (_) {
    // Tauri invoke failed, fall through to HTTP
  }

  // Fallback: HTTP call to gateway
  const method = cmd.startsWith("get_") ? "GET" : "POST";
  let url = `${GATEWAY_BASE}/api/${cmd.replace(/_/g, "-")}`;

  const opts = { headers: { "Content-Type": "application/json" } };

  if (method === "GET" && Object.keys(args).length > 0) {
    const params = new URLSearchParams(args);
    url += `?${params}`;
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

// ---- Toast ----
let toastTimer = null;

export function showToast(msg, type = "info") {
  const toast = document.getElementById("toast");
  const icon = document.getElementById("toast-icon");
  const text = document.getElementById("toast-msg");
  if (!toast || !icon || !text) return;

  text.textContent = msg;

  icon.className = "w-2 h-2 rounded-full";
  const colors = {
    success: "bg-emerald-500",
    error: "bg-red-500",
    info: "bg-cyan-500",
  };
  icon.classList.add(colors[type] || colors.info);

  toast.className =
    "fixed bottom-6 right-6 px-4 py-3 rounded-xl bg-slate-800/90 backdrop-blur border shadow-2xl text-sm text-slate-300 flex items-center gap-3 transition-all duration-300";
  const borderColors = {
    success: "border-emerald-700/50",
    error: "border-red-700/50",
    info: "border-cyan-700/50",
  };
  toast.classList.add(borderColors[type] || borderColors.info);

  toast.classList.remove("pointer-events-none", "translate-y-4", "opacity-0");

  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toast.classList.add("translate-y-4", "opacity-0");
    setTimeout(() => toast.classList.add("pointer-events-none"), 300);
  }, 3000);
}

// ---- Router ----
const PAGES = ["ecualizador", "dashboard", "config"];

// Track injected style tags so we don't duplicate
const injectedStyles = new Set();

let currentPage = null;

async function loadPage(name) {
  if (name === currentPage) return;
  currentPage = name;

  const app = document.getElementById("app");
  app.innerHTML = `<div class="flex items-center justify-center py-20"><div class="skeleton w-8 h-8 rounded-full"></div></div>`;

  try {
    const pageModule = await import(`./pages/${name}.js`);

    const { html = "", css = "", init } = pageModule;

    // Inject page CSS (once)
    if (css && !injectedStyles.has(name)) {
      const styleEl = document.createElement("style");
      styleEl.textContent = css;
      document.head.appendChild(styleEl);
      injectedStyles.add(name);
    }

    // Render
    app.innerHTML = html;

    // Init page
    if (typeof init === "function") {
      // Small delay to ensure DOM is settled
      requestAnimationFrame(() => init());
    }

    // Update nav active state
    document.querySelectorAll(".nav-btn").forEach((btn) => {
      btn.classList.toggle("active", btn.dataset.page === name);
    });
  } catch (err) {
    console.error(`Failed to load page "${name}":`, err);
    app.innerHTML = `
      <div class="flex flex-col items-center justify-center py-20 text-slate-500">
        <svg class="w-12 h-12 mb-4 text-slate-700" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9 3.75h.008v.008H12v-.008Z"/>
        </svg>
        <p class="text-lg font-medium mb-1">Error al cargar ${name}</p>
        <p class="text-sm">${err.message}</p>
        <button onclick="location.reload()" class="btn-secondary mt-6">Recargar</button>
      </div>
    `;
  }
}

// ---- Navigation ----
document.querySelectorAll(".nav-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    const page = btn.dataset.page;
    if (page && PAGES.includes(page)) {
      loadPage(page);
      window.location.hash = `#${page}`;
    }
  });
});

// ---- Initial load based on hash ----
function initRouter() {
  const hash = window.location.hash.replace("#", "") || "ecualizador";
  const page = PAGES.includes(hash) ? hash : "ecualizador";
  loadPage(page);
}

window.addEventListener("hashchange", () => {
  const hash = window.location.hash.replace("#", "") || "ecualizador";
  if (PAGES.includes(hash)) {
    loadPage(hash);
  }
});

// ---- Boot ----
document.addEventListener("DOMContentLoaded", initRouter);

// ============================================================
// Shared UI helpers (used across pages)
// ============================================================

/**
 * Create a vertical slider control
 * Returns { setValue, getValue, setColor, destroy }
 */
export function createVerticalSlider(container, opts = {}) {
  const {
    value = 0.5,
    min = 0,
    max = 1,
    step = 0.01,
    color = "#06b6d4",
    onChange = () => {},
  } = opts;

  const track = document.createElement("div");
  track.className = "vertical-slider-track";

  const fill = document.createElement("div");
  fill.className = "vertical-slider-fill";
  fill.style.background = color;

  const thumb = document.createElement("div");
  thumb.className = "vertical-slider-thumb";
  thumb.style.background = color;

  track.appendChild(fill);
  track.appendChild(thumb);
  container.appendChild(track);

  let currentVal = value;
  let dragging = false;

  function clamp(v) {
    return Math.min(max, Math.max(min, v));
  }

  function setValue(v, fireChange = true) {
    currentVal = clamp(v);
    if (step > 0) {
      currentVal = Math.round(currentVal / step) * step;
    }
    const pct = ((currentVal - min) / (max - min)) * 100;
    fill.style.height = `${pct}%`;
    thumb.style.bottom = `${pct}%`;
    if (fireChange) onChange(currentVal);
  }

  function posToValue(e) {
    const rect = track.getBoundingClientRect();
    const clientY = e.clientY ?? e.touches?.[0]?.clientY ?? 0;
    const y = clientY - rect.top;
    const pct = Math.max(0, Math.min(1, 1 - y / rect.height));
    return min + pct * (max - min);
  }

  function onMouseDown(e) {
    dragging = true;
    setValue(posToValue(e));
  }
  function onMouseMove(e) {
    if (!dragging) return;
    setValue(posToValue(e));
  }
  function onMouseUp() {
    dragging = false;
  }
  function onTouchStart(e) {
    e.preventDefault();
    dragging = true;
    setValue(posToValue(e));
  }
  function onTouchMove(e) {
    if (!dragging) return;
    setValue(posToValue(e));
  }
  function onTouchEnd() {
    dragging = false;
  }

  track.addEventListener("mousedown", onMouseDown);
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
  track.addEventListener("touchstart", onTouchStart, { passive: false });
  window.addEventListener("touchmove", onTouchMove);
  window.addEventListener("touchend", onTouchEnd);

  setValue(value, false);

  return {
    setValue,
    getValue: () => currentVal,
    setColor: (c) => {
      fill.style.background = c;
      thumb.style.background = c;
    },
    destroy: () => {
      track.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      track.removeEventListener("touchstart", onTouchStart);
      window.removeEventListener("touchmove", onTouchMove);
      window.removeEventListener("touchend", onTouchEnd);
    },
  };
}

/**
 * Create a horizontal slider (economy knob)
 */
export function createHorizontalSlider(container, opts = {}) {
  const {
    value = 0.5,
    min = 0,
    max = 1,
    step = 0.01,
    color = "#f59e0b",
    labels = [],
    onChange = () => {},
  } = opts;

  const track = document.createElement("div");
  track.className = "economy-track";

  const fill = document.createElement("div");
  fill.className = "economy-fill";
  fill.style.background = color;

  const thumb = document.createElement("div");
  thumb.className = "economy-thumb";

  track.appendChild(fill);
  track.appendChild(thumb);
  container.appendChild(track);

  let currentVal = value;
  let dragging = false;

  function clamp(v) {
    return Math.min(max, Math.max(min, v));
  }

  function setValue(v, fireChange = true) {
    currentVal = clamp(v);
    if (step > 0) {
      currentVal = Math.round(currentVal / step) * step;
    }
    const pct = ((currentVal - min) / (max - min)) * 100;
    fill.style.width = `${pct}%`;
    thumb.style.left = `${pct}%`;
    if (fireChange) onChange(currentVal);
  }

  function posToValue(e) {
    const rect = track.getBoundingClientRect();
    const clientX = e.clientX ?? e.touches?.[0]?.clientX ?? 0;
    const x = clientX - rect.left;
    const pct = Math.max(0, Math.min(1, x / rect.width));
    return min + pct * (max - min);
  }

  function onMouseDown(e) {
    dragging = true;
    setValue(posToValue(e));
  }
  function onMouseMove(e) {
    if (!dragging) return;
    setValue(posToValue(e));
  }
  function onMouseUp() {
    dragging = false;
  }
  function onTouchStart(e) {
    e.preventDefault();
    dragging = true;
    setValue(posToValue(e));
  }
  function onTouchMove(e) {
    if (!dragging) return;
    setValue(posToValue(e));
  }
  function onTouchEnd() {
    dragging = false;
  }

  track.addEventListener("mousedown", onMouseDown);
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
  track.addEventListener("touchstart", onTouchStart, { passive: false });
  window.addEventListener("touchmove", onTouchMove);
  window.addEventListener("touchend", onTouchEnd);

  setValue(value, false);

  return {
    setValue,
    getValue: () => currentVal,
    setColor: (c) => {
      fill.style.background = c;
    },
    destroy: () => {
      track.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      track.removeEventListener("touchstart", onTouchStart);
      window.removeEventListener("touchmove", onTouchMove);
      window.removeEventListener("touchend", onTouchEnd);
    },
  };
}

/**
 * Format numbers compactly (e.g. 1.2M, 3.4K)
 */
export function formatNum(n) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toLocaleString();
}

/**
 * Format USD cost compactly
 */
export function formatUSD(n) {
  if (n >= 1) return `$${n.toFixed(2)}`;
  if (n >= 0.01) return `¢${(n * 100).toFixed(1)}`;
  return `$${Number(n).toExponential(1)}`;
}

/**
 * Debounce helper
 */
export function debounce(fn, ms = 300) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}
