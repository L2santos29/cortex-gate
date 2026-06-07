// ==========================================================================
// Cortex Gate — Página: Configuración
// Modelos/Providers, API keys, Budgets por usuario
// ==========================================================================

import { tauriCmd, showToast } from "../main.js";

export const html = `
<!-- Header -->
<div class="mb-8">
  <h2 class="text-2xl font-bold text-slate-100 tracking-tight">Configuración</h2>
  <p class="text-sm text-slate-500 mt-1">
    Proveedores, API keys y límites de presupuesto
  </p>
</div>

<div class="grid grid-cols-1 lg:grid-cols-2 gap-6">

  <!-- LEFT COLUMN -->

  <!-- Providers -->
  <div class="config-section">
    <div class="flex items-center justify-between mb-4">
      <h3 class="text-sm font-semibold text-slate-200">Proveedores y Modelos</h3>
      <button id="add-provider-btn" class="btn-ghost text-xs">
        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/>
        </svg>
        Añadir
      </button>
    </div>
    <div id="providers-list" class="space-y-3">
      <!-- Injected by JS -->
      <div class="text-center text-slate-600 text-sm py-8">
        <span class="skeleton inline-block w-48 h-4"></span>
      </div>
    </div>
  </div>

  <!-- API Keys -->
  <div class="config-section">
    <div class="flex items-center justify-between mb-4">
      <h3 class="text-sm font-semibold text-slate-200">API Keys de Proveedores</h3>
      <button id="add-apikey-btn" class="btn-ghost text-xs">
        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/>
        </svg>
        Añadir
      </button>
    </div>
    <div id="apikeys-list" class="space-y-2">
      <!-- Injected by JS -->
      <div class="text-center text-slate-600 text-sm py-8">
        <span class="skeleton inline-block w-48 h-4"></span>
      </div>
    </div>
  </div>

  <!-- BOTTOM ROW (full width) -->

  <!-- Budgets -->
  <div class="lg:col-span-2 config-section">
    <div class="flex items-center justify-between mb-4">
      <h3 class="text-sm font-semibold text-slate-200">Presupuestos por Usuario</h3>
      <button id="add-budget-btn" class="btn-ghost text-xs">
        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15"/>
        </svg>
        Añadir
      </button>
    </div>
    <div id="budgets-list">
      <!-- Injected by JS -->
      <div class="text-center text-slate-600 text-sm py-8">
        <span class="skeleton inline-block w-56 h-4"></span>
      </div>
    </div>
  </div>

</div>
`;

// ---- CSS ----
export const css = ``;

// ---- Init ----
export function init() {
  loadProviders();
  loadApiKeys();
  loadBudgets();

  // Event listeners
  document.getElementById("add-provider-btn")?.addEventListener("click", addProvider);
  document.getElementById("add-apikey-btn")?.addEventListener("click", addApiKey);
  document.getElementById("add-budget-btn")?.addEventListener("click", addBudget);
}

// ============================================================
// Providers
// ============================================================

async function loadProviders() {
  const container = document.getElementById("providers-list");
  if (!container) return;

  try {
    const config = await tauriCmd("get_config");
    const providers = config.providers ?? [];

    if (providers.length === 0) {
      container.innerHTML = `
        <div class="text-center text-slate-600 text-sm py-8">
          <svg class="w-10 h-10 mx-auto mb-3 text-slate-700" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375m16.5 0v3.75m-16.5-3.75v3.75m16.5 0v3.75C20.25 16.153 16.556 18 12 18s-8.25-1.847-8.25-4.125v-3.75m16.5 0c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125"/>
          </svg>
          <p>No hay proveedores configurados</p>
          <p class="text-xs text-slate-600 mt-1">Añade un proveedor para empezar a rutear requests</p>
        </div>
      `;
      return;
    }

    container.innerHTML = providers
      .map(
        (p, i) => `
        <div class="bg-slate-800/40 border border-slate-700/50 rounded-lg p-4 provider-item" data-index="${i}">
          <div class="flex items-center justify-between mb-2">
            <div class="flex items-center gap-2">
              <span class="w-2 h-2 rounded-full ${p.enabled ? "bg-emerald-500" : "bg-slate-600"}"></span>
              <span class="font-medium text-sm text-slate-200">${p.name ?? "—"}</span>
              <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-slate-700 text-slate-400 uppercase">${p.provider_type ?? "?"}</span>
            </div>
            <button class="text-slate-600 hover:text-red-400 transition-colors remove-provider" data-index="${i}">
              <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"/>
              </svg>
            </button>
          </div>
          <div class="text-xs text-slate-500 space-y-0.5">
            <div class="flex items-center gap-2">
              <span class="text-slate-600 w-16">URL:</span>
              <span class="font-mono text-slate-400 truncate">${p.base_url ?? "—"}</span>
            </div>
            <div class="flex items-center gap-2">
              <span class="text-slate-600 w-16">Modelos:</span>
              <span class="text-slate-400">${(p.models ?? []).length} configurados</span>
            </div>
          </div>
        </div>
      `
      )
      .join("");

    // Remove handlers
    container.querySelectorAll(".remove-provider").forEach((btn) => {
      btn.addEventListener("click", async () => {
        const idx = parseInt(btn.dataset.index);
        try {
          await tauriCmd("remove_provider", { index: idx });
          showToast("Proveedor eliminado", "success");
          loadProviders();
        } catch (err) {
          showToast(`Error: ${err.message}`, "error");
        }
      });
    });
  } catch (err) {
    container.innerHTML = `
      <div class="text-center text-red-400 text-sm py-8">
        Error al cargar proveedores: ${err.message}
      </div>
    `;
  }
}

async function addProvider() {
  // Simple prompt for provider name
  const name = prompt("Nombre del proveedor (ej: openai, anthropic):");
  if (!name) return;

  const baseUrl = prompt("Base URL:", "https://api.openai.com/v1");
  if (!baseUrl) return;

  const type = prompt("Tipo (openai, anthropic, openrouter, ollama, custom):", "openai");
  if (!type) return;

  try {
    await tauriCmd("add_provider", { name, base_url: baseUrl, provider_type: type });
    showToast(`Proveedor "${name}" añadido`, "success");
    loadProviders();
  } catch (err) {
    showToast(`Error: ${err.message}`, "error");
  }
}

// ============================================================
// API Keys
// ============================================================

async function loadApiKeys() {
  const container = document.getElementById("apikeys-list");
  if (!container) return;

  try {
    const data = await tauriCmd("get_api_keys");
    const keys = data.keys ?? [];

    if (keys.length === 0) {
      container.innerHTML = `
        <div class="text-center text-slate-600 text-sm py-8">
          <svg class="w-10 h-10 mx-auto mb-3 text-slate-700" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 5.25a3 3 0 0 1 3 3m3 0a6 6 0 0 1-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1 1 21.75 8.25Z"/>
          </svg>
          <p>No hay API keys configuradas</p>
        </div>
      `;
      return;
    }

    container.innerHTML = keys
      .map(
        (k) => `
        <div class="flex items-center justify-between bg-slate-800/40 border border-slate-700/50 rounded-lg px-4 py-2.5 apikey-item">
          <div class="flex items-center gap-3 min-w-0">
            <svg class="w-4 h-4 text-slate-500 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" d="M15.75 5.25a3 3 0 0 1 3 3m3 0a6 6 0 0 1-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1 1 21.75 8.25Z"/>
            </svg>
            <div class="min-w-0">
              <div class="text-sm font-medium text-slate-200 truncate">${k.name ?? "—"}</div>
              <div class="text-xs text-slate-600 font-mono truncate">${k.provider ?? ""} · ${(k.key_preview ?? "••••").slice(0, 20)}...</div>
            </div>
          </div>
          <button class="text-slate-600 hover:text-red-400 transition-colors shrink-0 ml-2 remove-apikey" data-id="${k.id}">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12"/>
            </svg>
          </button>
        </div>
      `
      )
      .join("");

    container.querySelectorAll(".remove-apikey").forEach((btn) => {
      btn.addEventListener("click", async () => {
        const id = btn.dataset.id;
        try {
          await tauriCmd("remove_api_key", { id });
          showToast("API key eliminada", "success");
          loadApiKeys();
        } catch (err) {
          showToast(`Error: ${err.message}`, "error");
        }
      });
    });
  } catch (err) {
    container.innerHTML = `
      <div class="text-center text-red-400 text-sm py-8">
        Error: ${err.message}
      </div>
    `;
  }
}

function addApiKey() {
  const provider = prompt("Proveedor (ej: openai, anthropic):");
  if (!provider) return;
  const name = prompt("Nombre descriptivo para esta key:");
  if (!name) return;
  const key = prompt("API Key:");
  if (!key) return;

  (async () => {
    try {
      await tauriCmd("add_api_key", { provider, name, key });
      showToast("API key añadida", "success");
      loadApiKeys();
    } catch (err) {
      showToast(`Error: ${err.message}`, "error");
    }
  })();
}

// ============================================================
// Budgets
// ============================================================

async function loadBudgets() {
  const container = document.getElementById("budgets-list");
  if (!container) return;

  try {
    const data = await tauriCmd("get_budgets");
    const budgets = data.budgets ?? [];

    if (budgets.length === 0) {
      container.innerHTML = `
        <div class="text-center text-slate-600 text-sm py-8">
          <svg class="w-10 h-10 mx-auto mb-3 text-slate-700" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v12m-3-2.818.879.659c1.171.879 3.07.879 4.242 0 1.172-.879 1.172-2.303 0-3.182C13.536 12.219 12.768 12 12 12c-.725 0-1.45-.22-2.003-.659-1.106-.879-1.106-2.303 0-3.182s2.9-.879 4.006 0l.415.33M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"/>
          </svg>
          <p>No hay presupuestos configurados</p>
          <p class="text-xs text-slate-600 mt-1">Establece límites de tokens por usuario para controlar costos</p>
        </div>
      `;
      return;
    }

    container.innerHTML = `
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>Usuario</th>
              <th class="text-right">Tokens/hora</th>
              <th class="text-right">Tokens/día</th>
              <th class="text-right">Tokens/mes</th>
              <th class="text-right">Usado hoy</th>
              <th class="text-center">Estado</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            ${budgets
              .map(
                (b) => `
              <tr>
                <td class="font-medium text-slate-200">${b.user_name ?? b.user_id ?? "—"}</td>
                <td class="text-right font-mono tabular-nums">${(b.tokens_per_hour ?? 0).toLocaleString()}</td>
                <td class="text-right font-mono tabular-nums">${(b.tokens_per_day ?? 0).toLocaleString()}</td>
                <td class="text-right font-mono tabular-nums">${(b.tokens_per_month ?? 0).toLocaleString()}</td>
                <td class="text-right font-mono tabular-nums text-slate-400">${(b.used_today ?? 0).toLocaleString()}</td>
                <td class="text-center">
                  <span class="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full ${
                    (b.used_today ?? 0) >= (b.tokens_per_day ?? Infinity)
                      ? "bg-red-900/30 text-red-400"
                      : (b.used_today ?? 0) >= (b.tokens_per_day ?? Infinity) * 0.8
                      ? "bg-amber-900/30 text-amber-400"
                      : "bg-emerald-900/30 text-emerald-400"
                  }">
                    <span class="w-1.5 h-1.5 rounded-full currentColor opacity-60"></span>
                    ${(b.used_today ?? 0) >= (b.tokens_per_day ?? Infinity) ? "Excedido" : "Activo"}
                  </span>
                </td>
                <td>
                  <button class="text-slate-600 hover:text-red-400 transition-colors remove-budget" data-id="${b.id}">
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0"/>
                    </svg>
                  </button>
                </td>
              </tr>
            `
              )
              .join("")}
          </tbody>
        </table>
      </div>
    `;

    container.querySelectorAll(".remove-budget").forEach((btn) => {
      btn.addEventListener("click", async () => {
        const id = btn.dataset.id;
        try {
          await tauriCmd("remove_budget", { id });
          showToast("Presupuesto eliminado", "success");
          loadBudgets();
        } catch (err) {
          showToast(`Error: ${err.message}`, "error");
        }
      });
    });
  } catch (err) {
    container.innerHTML = `
      <div class="text-center text-red-400 text-sm py-8">
        Error: ${err.message}
      </div>
    `;
  }
}

function addBudget() {
  const userId = prompt("User ID:");
  if (!userId) return;
  const perHour = prompt("Tokens por hora (0 = ilimitado):", "100000");
  const perDay = prompt("Tokens por día:", "1000000");
  const perMonth = prompt("Tokens por mes:", "30000000");

  (async () => {
    try {
      await tauriCmd("set_budget", {
        user_id: userId,
        tokens_per_hour: parseInt(perHour) || 0,
        tokens_per_day: parseInt(perDay) || 0,
        tokens_per_month: parseInt(perMonth) || 0,
      });
      showToast("Presupuesto configurado", "success");
      loadBudgets();
    } catch (err) {
      showToast(`Error: ${err.message}`, "error");
    }
  })();
}
