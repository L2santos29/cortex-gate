// ====================================================================
// Cortex Gate — Extension Manager Page
// Lists all installed extensions with enable/disable toggles.
// ====================================================================

import { registry } from "../extensions/registry.js";
import { showToast } from "../main.js";

export const html = `
<div class="flex items-center justify-between mb-6">
  <div>
    <h2 class="text-2xl font-bold text-slate-800 tracking-tight">Extensions</h2>
    <p class="text-sm text-slate-500 mt-1">
      Manage installed extensions — enable, disable, and configure
    </p>
  </div>
  <button id="refresh-ext-btn" class="btn btn-secondary">
    <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182"/>
    </svg>
    Refresh
  </button>
</div>
<div id="ext-list" class="space-y-3">
  <div class="empty-state">
    <svg fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" d="M13.5 16.5h-3m0 0a1.5 1.5 0 0 1-1.5-1.5v-3a1.5 1.5 0 0 1 1.5-1.5m0 0V6.75a.75.75 0 0 1 .75-.75h6a.75.75 0 0 1 .75.75v3.75m-7.5 0a1.5 1.5 0 0 1 1.5 1.5v3a1.5 1.5 0 0 1-1.5 1.5m0 0v3.75a.75.75 0 0 1-.75.75h-6a.75.75 0 0 1-.75-.75v-6a.75.75 0 0 1 .75-.75h.75m7.5 0h.75a.75.75 0 0 1 .75.75v.75"/>
    </svg>
    <p>No extensions installed</p>
    <p class="sub">Extensions appear here when discovered</p>
  </div>
</div>
`;

export const css = ``;

export function init() {
  renderExtensionList();
  document.getElementById("refresh-ext-btn")?.addEventListener("click", () => {
    renderExtensionList();
    showToast("Extension list refreshed", "info");
  });

  // Re-render when extensions change
  registry.onReload(() => {
    renderExtensionList();
  });
}

function renderExtensionList() {
  const container = document.getElementById("ext-list");
  if (!container) return;

  const extensions = registry.getAllExtensions();

  if (extensions.length === 0) {
    container.innerHTML = `
      <div class="empty-state">
        <svg fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="M13.5 16.5h-3m0 0a1.5 1.5 0 0 1-1.5-1.5v-3a1.5 1.5 0 0 1 1.5-1.5m0 0V6.75a.75.75 0 0 1 .75-.75h6a.75.75 0 0 1 .75.75v3.75m-7.5 0a1.5 1.5 0 0 1 1.5 1.5v3a1.5 1.5 0 0 1-1.5 1.5m0 0v3.75a.75.75 0 0 1-.75.75h-6a.75.75 0 0 1-.75-.75v-6a.75.75 0 0 1 .75-.75h.75m7.5 0h.75a.75.75 0 0 1 .75.75v.75"/>
        </svg>
        <p>No extensions installed</p>
        <p class="sub">Extensions appear here when discovered</p>
      </div>`;
    return;
  }

  container.innerHTML = extensions.map(ext => {
    const statusClass = ext.enabled ? "bg-emerald-500" : "bg-slate-300";
    const statusText = ext.enabled ? "Enabled" : "Disabled";
    const pageCount = (ext.pages || []).length;
    const pagesInfo = pageCount > 0 ? `${pageCount} page(s)` : "No UI pages";

    return `
      <div class="ext-card">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3 min-w-0">
            <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-cyan-100 to-cyan-200 flex items-center justify-center text-cyan-700 font-bold text-sm shrink-0">
              ${ext.name.charAt(0).toUpperCase()}
            </div>
            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <h3 class="font-semibold text-slate-800 text-sm">${escapeHtml(ext.name)}</h3>
                <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-slate-100 text-slate-500 font-mono">v${escapeHtml(ext.version || "?")}</span>
              </div>
              <p class="text-xs text-slate-500 mt-0.5 truncate">${escapeHtml(ext.description || "")}</p>
              <div class="flex items-center gap-3 mt-1 text-[10px] text-slate-400">
                <span>${pagesInfo}</span>
                <span class="flex items-center gap-1">
                  <span class="inline-block w-1.5 h-1.5 rounded-full ${statusClass}"></span>
                  ${statusText}
                </span>
              </div>
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0 ml-3">
            <label class="toggle">
              <input type="checkbox" class="ext-toggle" data-id="${escapeHtml(ext.id)}" ${ext.enabled ? "checked" : ""}>
              <span class="toggle-slider"></span>
            </label>
          </div>
        </div>
      </div>`;
  }).join("");

  // Bind toggle events
  container.querySelectorAll(".ext-toggle").forEach(toggle => {
    toggle.addEventListener("change", () => {
      const id = toggle.dataset.id;
      const enabled = toggle.checked;
      registry.toggleExtension(id, enabled);
      showToast(`"${id}" ${enabled ? "enabled" : "disabled"}`, "success");
    });
  });
}

function escapeHtml(str) {
  if (!str) return "";
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
