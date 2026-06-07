// ==========================================================================
// Cortex Gate — Página: Dashboard
// Estadísticas, uso por usuario, gráfico de barras
// ==========================================================================

import { tauriCmd, showToast, formatNum, formatUSD } from "../main.js";

export const html = `
<!-- Header -->
<div class="flex items-center justify-between mb-8">
  <div>
    <h2 class="text-2xl font-bold text-slate-100 tracking-tight">Dashboard</h2>
    <p class="text-sm text-slate-500 mt-1">
      Estadísticas de uso, costos y actividad del gateway
    </p>
  </div>
  <button id="refresh-dash-btn" class="btn-secondary">
    <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182"/>
    </svg>
    Actualizar
  </button>
</div>

<!-- Stats cards -->
<div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
  <div class="stat-card" id="stat-tokens">
    <div class="stat-value text-cyan-400">—</div>
    <div class="stat-label">Tokens Hoy</div>
    <div class="mt-2 flex items-center gap-1.5 text-xs text-slate-600">
      <span class="inline-block w-2 h-2 rounded-full bg-cyan-500/50"></span>
      Total entrada + salida
    </div>
  </div>

  <div class="stat-card" id="stat-cost">
    <div class="stat-value text-amber-400">—</div>
    <div class="stat-label">Costo Hoy</div>
    <div class="mt-2 flex items-center gap-1.5 text-xs text-slate-600">
      <span class="inline-block w-2 h-2 rounded-full bg-amber-500/50"></span>
      USD estimado
    </div>
  </div>

  <div class="stat-card" id="stat-requests">
    <div class="stat-value text-emerald-400">—</div>
    <div class="stat-label">Requests</div>
    <div class="mt-2 flex items-center gap-1.5 text-xs text-slate-600">
      <span class="inline-block w-2 h-2 rounded-full bg-emerald-500/50"></span>
      Total hoy
    </div>
  </div>

  <div class="stat-card" id="stat-models">
    <div class="stat-value text-violet-400">—</div>
    <div class="stat-label">Modelos Activos</div>
    <div class="mt-2 flex items-center gap-1.5 text-xs text-slate-600">
      <span class="inline-block w-2 h-2 rounded-full bg-violet-500/50"></span>
      En uso actual
    </div>
  </div>
</div>

<!-- Chart + Usage table -->
<div class="grid grid-cols-1 lg:grid-cols-5 gap-6 mb-8">
  <!-- Bar chart (CSS pure) -->
  <div class="lg:col-span-2 bg-slate-900/60 backdrop-blur-sm border border-slate-800 rounded-xl p-5">
    <h3 class="text-sm font-semibold text-slate-200 mb-4">Tokens por modelo (hoy)</h3>
    <div id="bar-chart" class="bar-chart">
      <!-- Bars injected by JS -->
    </div>
  </div>

  <!-- Usage by user table -->
  <div class="lg:col-span-3 bg-slate-900/60 backdrop-blur-sm border border-slate-800 rounded-xl p-5">
    <h3 class="text-sm font-semibold text-slate-200 mb-4">Uso por Usuario</h3>
    <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th>Usuario</th>
            <th class="text-right">Tokens</th>
            <th class="text-right">Costo</th>
            <th class="text-right">Requests</th>
          </tr>
        </thead>
        <tbody id="usage-table-body">
          <tr>
            <td colspan="4" class="text-center text-slate-600 py-8">
              <span class="skeleton inline-block w-48 h-4"></span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</div>

<!-- Usage report per user -->
<div class="bg-slate-900/60 backdrop-blur-sm border border-slate-800 rounded-xl p-5 mb-6">
  <div class="flex items-center justify-between mb-4">
    <h3 class="text-sm font-semibold text-slate-200">Reporte Detallado</h3>
    <div class="flex items-center gap-2">
      <input
        id="report-user-input"
        type="text"
        placeholder="User ID..."
        class="config-input w-48 text-xs"
      />
      <button id="load-report-btn" class="btn-secondary text-xs px-3 py-1.5">
        Cargar
      </button>
    </div>
  </div>
  <div id="report-content" class="text-sm text-slate-500 text-center py-8">
    Ingresa un User ID y presiona "Cargar" para ver el reporte
  </div>
</div>
`;

// ---- CSS ----
export const css = ``;

// ---- Init ----
export function init() {
  const refreshBtn = document.getElementById("refresh-dash-btn");
  const loadReportBtn = document.getElementById("load-report-btn");
  const reportInput = document.getElementById("report-user-input");

  if (!refreshBtn) return;

  // ---- Load dashboard stats ----
  async function loadStats() {
    try {
      const data = await tauriCmd("get_dashboard_stats");

      document.querySelector("#stat-tokens .stat-value").textContent =
        formatNum(data.tokens_today ?? 0);
      document.querySelector("#stat-cost .stat-value").textContent =
        formatUSD(data.cost_today ?? 0);
      document.querySelector("#stat-requests .stat-value").textContent =
        formatNum(data.requests_today ?? 0);
      document.querySelector("#stat-models .stat-value").textContent =
        data.active_models ?? "—";

      // Render bar chart
      renderBarChart(data.models ?? []);

      // Render usage table
      renderUsageTable(data.usage_by_user ?? []);
    } catch (err) {
      showToast(`Error al cargar stats: ${err.message}`, "error");
      console.error(err);
    }
  }

  // ---- Bar chart ----
  function renderBarChart(models) {
    const container = document.getElementById("bar-chart");
    if (!container) return;

    if (!models || models.length === 0) {
      container.innerHTML = `
        <div class="w-full text-center text-slate-600 text-sm py-8">
          No hay datos de modelos aún
        </div>
      `;
      return;
    }

    const maxTokens = Math.max(...models.map((m) => m.tokens ?? 0), 1);

    container.innerHTML = models
      .map((m) => {
        const pct = ((m.tokens ?? 0) / maxTokens) * 100;
        const label = (m.model ?? "?").split("/").pop()?.slice(0, 12) ?? "?";
        return `
          <div class="bar-item">
            <span class="text-[10px] text-slate-500 font-mono">${formatNum(m.tokens ?? 0)}</span>
            <div class="bar" style="height: ${Math.max(pct, 4)}%; background: linear-gradient(180deg, #06b6d4 0%, #0891b2 100%);"></div>
            <span class="bar-label" title="${m.model ?? ""}">${label}</span>
          </div>
        `;
      })
      .join("");
  }

  // ---- Usage table ----
  function renderUsageTable(users) {
    const tbody = document.getElementById("usage-table-body");
    if (!tbody) return;

    if (!users || users.length === 0) {
      tbody.innerHTML = `
        <tr>
          <td colspan="4" class="text-center text-slate-600 py-8">Sin actividad registrada hoy</td>
        </tr>
      `;
      return;
    }

    tbody.innerHTML = users
      .map(
        (u) => `
        <tr>
          <td class="font-medium text-slate-200">${u.name ?? u.user_id ?? "—"}</td>
          <td class="text-right font-mono tabular-nums text-slate-300">${formatNum(u.tokens ?? 0)}</td>
          <td class="text-right font-mono tabular-nums text-amber-400">${formatUSD(u.cost ?? 0)}</td>
          <td class="text-right font-mono tabular-nums text-slate-300">${formatNum(u.requests ?? 0)}</td>
        </tr>
      `
      )
      .join("");
  }

  // ---- Load usage report ----
  async function loadReport() {
    const userId = reportInput?.value?.trim();
    if (!userId) {
      showToast("Ingresa un User ID", "error");
      return;
    }

    const reportContent = document.getElementById("report-content");
    reportContent.innerHTML = `<span class="skeleton inline-block w-32 h-4"></span>`;

    try {
      const report = await tauriCmd("get_usage_report", { user_id: userId });

      if (!report || !report.entries || report.entries.length === 0) {
        reportContent.innerHTML = `
          <div class="text-slate-600 py-4">
            <p class="mb-2">No se encontraron entradas para <code class="text-cyan-500 bg-slate-800 px-1.5 py-0.5 rounded text-xs">${userId}</code></p>
            <p class="text-xs text-slate-600">Período: ${report.period ?? "hoy"}</p>
          </div>
        `;
        return;
      }

      const totalTokens = report.entries.reduce((s, e) => s + (e.tokens_in ?? 0) + (e.tokens_out ?? 0), 0);
      const totalCost = report.entries.reduce((s, e) => s + (e.cost ?? 0), 0);

      let rows = report.entries
        .map(
          (e) => `
          <tr>
            <td class="font-mono text-xs text-slate-400">${new Date(e.timestamp).toLocaleString()}</td>
            <td>${e.model ?? "—"}</td>
            <td class="text-right font-mono tabular-nums">${formatNum(e.tokens_in ?? 0)}</td>
            <td class="text-right font-mono tabular-nums">${formatNum(e.tokens_out ?? 0)}</td>
            <td class="text-right font-mono tabular-nums text-amber-400">${formatUSD(e.cost ?? 0)}</td>
          </tr>
        `
        )
        .join("");

      reportContent.innerHTML = `
        <div class="mb-4 grid grid-cols-3 gap-4">
          <div class="bg-slate-800/50 rounded-lg p-3 text-center">
            <div class="text-lg font-bold text-slate-100">${formatNum(totalTokens)}</div>
            <div class="text-[10px] text-slate-600 uppercase tracking-wider">Tokens totales</div>
          </div>
          <div class="bg-slate-800/50 rounded-lg p-3 text-center">
            <div class="text-lg font-bold text-amber-400">${formatUSD(totalCost)}</div>
            <div class="text-[10px] text-slate-600 uppercase tracking-wider">Costo total</div>
          </div>
          <div class="bg-slate-800/50 rounded-lg p-3 text-center">
            <div class="text-lg font-bold text-slate-100">${report.entries.length}</div>
            <div class="text-[10px] text-slate-600 uppercase tracking-wider">Entradas</div>
          </div>
        </div>
        <div class="table-wrap max-h-64 overflow-y-auto">
          <table class="data-table text-xs">
            <thead>
              <tr>
                <th>Fecha</th>
                <th>Modelo</th>
                <th class="text-right">In</th>
                <th class="text-right">Out</th>
                <th class="text-right">Costo</th>
              </tr>
            </thead>
            <tbody>${rows}</tbody>
          </table>
        </div>
      `;
    } catch (err) {
      reportContent.innerHTML = `
        <div class="text-red-400 text-center py-4">
          Error: ${err.message}
        </div>
      `;
      showToast(`Error al cargar reporte: ${err.message}`, "error");
    }
  }

  // ---- Events ----
  refreshBtn.addEventListener("click", loadStats);
  loadReportBtn.addEventListener("click", loadReport);
  reportInput?.addEventListener("keydown", (e) => {
    if (e.key === "Enter") loadReport();
  });

  // ---- Initial load ----
  loadStats();
}
