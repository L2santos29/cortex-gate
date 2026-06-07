// ==========================================================================
// Cortex Gate — Página: Ecualizador
// 8 sliders verticales para dimensiones + perilla de economía
// ==========================================================================

import { tauriCmd, showToast, createVerticalSlider, createHorizontalSlider } from "../main.js";

export const html = `
<!-- Header -->
<div class="flex items-center justify-between mb-8">
  <div>
    <h2 class="text-2xl font-bold text-slate-100 tracking-tight">Ecualizador</h2>
    <p class="text-sm text-slate-500 mt-1">
      Ajusta los pesos de cada dimensión cognitiva para afinar el ruteo de modelos
    </p>
  </div>
  <button id="save-eq-btn" class="btn-primary">
    <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/>
    </svg>
    Guardar Configuración
  </button>
</div>

<!-- Equalizer grid -->
<div class="bg-slate-900/60 backdrop-blur-sm border border-slate-800 rounded-2xl p-6 lg:p-8">
  <div class="flex items-end gap-3 sm:gap-4 md:gap-5 lg:gap-6 justify-center"
       id="sliders-container"
       style="height: 320px;">

    <!-- 8 sliders will be injected by JS -->

  </div>

  <!-- Dimension labels & values row (rendered by JS) -->
  <div id="slider-labels" class="flex justify-center gap-3 sm:gap-4 md:gap-5 lg:gap-6 mt-4"></div>
  <div id="slider-values" class="flex justify-center gap-3 sm:gap-4 md:gap-5 lg:gap-6 mt-2"></div>
</div>

<!-- Economy knob -->
<div class="mt-6 bg-slate-900/60 backdrop-blur-sm border border-slate-800 rounded-2xl p-6">
  <div class="flex items-center justify-between mb-4">
    <div>
      <h3 class="text-sm font-semibold text-slate-200">Nivel de Economía</h3>
      <p class="text-xs text-slate-500 mt-0.5">Balance entre calidad y costo de los modelos</p>
    </div>
    <span id="economy-value" class="text-lg font-bold text-amber-400 font-mono tabular-nums">0.50</span>
  </div>

  <div id="economy-container" class="max-w-2xl mx-auto py-3"></div>

  <div class="flex justify-between text-xs text-slate-600 max-w-2xl mx-auto mt-1">
    <span class="font-medium text-emerald-500">Calidad ←</span>
    <span class="text-slate-600">Balanceado</span>
    <span class="font-medium text-amber-500">→ Economía</span>
  </div>
</div>

<!-- Info card -->
<div class="mt-6 bg-slate-900/40 border border-slate-800/60 rounded-xl p-4">
  <div class="flex items-start gap-3">
    <svg class="w-5 h-5 text-cyan-500 shrink-0 mt-0.5" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" d="m11.25 11.25.041-.02a.75.75 0 0 1 1.063.852l-.708 2.836a.75.75 0 0 0 1.063.853l.041-.021M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9-3.75h.008v.008H12V8.25Z"/>
    </svg>
    <div>
      <p class="text-xs text-slate-500 leading-relaxed">
        Los pesos determinan qué dimensiones tienen más influencia en la selección de modelo.
        El nivel de economía inclina la balanza hacia modelos más baratos (economía alta) o
        más capaces (economía baja). Los cambios se aplican en tiempo real al gateway.
      </p>
    </div>
  </div>
</div>
`;

// ---- CSS ----
export const css = `
/* Extra slider styling on top of Tailwind base */
.vertical-slider-track {
  width: 36px;
  min-width: 28px;
  flex: 0 0 36px;
}
@media (max-width: 640px) {
  .vertical-slider-track {
    width: 28px;
    flex: 0 0 28px;
  }
}
.slider-col {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0;
  flex: 1;
  max-width: 56px;
}
`;

// ---- Config ----
const DIMENSIONS = [
  { key: "reasoning",  label: "RZN",  full: "Reasoning",  color: "#a78bfa" },
  { key: "code",       label: "COD",  full: "Code",       color: "#34d399" },
  { key: "creativity", label: "CRE",  full: "Creativity", color: "#f472b6" },
  { key: "math",       label: "MTH",  full: "Math",       color: "#60a5fa" },
  { key: "precision",  label: "PRC",  full: "Precision",  color: "#fbbf24" },
  { key: "speed",      label: "SPD",  full: "Speed",      color: "#f97316" },
  { key: "context",    label: "CTX",  full: "Context",    color: "#2dd4bf" },
  { key: "safety",     label: "SAF",  full: "Safety",     color: "#fb7185" },
];

// ---- Init ----
export function init() {
  const slidersContainer = document.getElementById("sliders-container");
  const labelsContainer = document.getElementById("slider-labels");
  const valuesContainer = document.getElementById("slider-values");
  const economyContainer = document.getElementById("economy-container");
  const economyValueEl = document.getElementById("economy-value");
  const saveBtn = document.getElementById("save-eq-btn");

  if (!slidersContainer) return;

  // Track slider instances
  const sliders = [];
  const valueEls = [];

  // Create dimension sliders
  DIMENSIONS.forEach((dim, i) => {
    // Column wrapper
    const col = document.createElement("div");
    col.className = "slider-col";

    // Slider goes inside col
    const sliderHolder = document.createElement("div");
    sliderHolder.className = "flex-1 w-full flex items-end";
    col.appendChild(sliderHolder);
    slidersContainer.appendChild(col);

    const slider = createVerticalSlider(sliderHolder, {
      value: 0.125,
      min: 0,
      max: 1,
      step: 0.01,
      color: dim.color,
      onChange: (val) => {
        if (valueEls[i]) {
          valueEls[i].textContent = val.toFixed(2);
        }
      },
    });
    sliders.push(slider);

    // Label
    const label = document.createElement("div");
    label.className = "text-[11px] font-semibold text-slate-500 tracking-wider text-center";
    label.textContent = dim.label;
    labelsContainer.appendChild(label);

    // Value
    const valEl = document.createElement("div");
    valEl.className = "text-xs font-mono tabular-nums text-slate-400 text-center";
    valEl.textContent = "0.13";
    valuesContainer.appendChild(valEl);
    valueEls.push(valEl);
  });

  // Economy slider
  const economySlider = createHorizontalSlider(economyContainer, {
    value: 0.5,
    min: 0,
    max: 1,
    step: 0.01,
    color: "#f59e0b",
    onChange: (val) => {
      if (economyValueEl) {
        economyValueEl.textContent = val.toFixed(2);
      }
    },
  });

  // ---- Load current config from gateway ----
  async function loadConfig() {
    try {
      const config = await tauriCmd("get_ecualizador");
      if (config.dimensions && Array.isArray(config.dimensions)) {
        config.dimensions.forEach((dim, i) => {
          if (sliders[i]) {
            sliders[i].setValue(dim.weight, false);
            if (valueEls[i]) {
              valueEls[i].textContent = dim.weight.toFixed(2);
            }
          }
        });
      }
      if (typeof config.economy === "number") {
        economySlider.setValue(config.economy, false);
        if (economyValueEl) {
          economyValueEl.textContent = config.economy.toFixed(2);
        }
      }
    } catch (err) {
      console.warn("Could not load config from gateway:", err.message);
      // Keep defaults
    }
  }

  // ---- Save config ----
  async function saveConfig() {
    saveBtn.disabled = true;
    saveBtn.innerHTML = `
      <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
      </svg>
      Guardando...
    `;

    try {
      // Save each dimension weight
      for (let i = 0; i < DIMENSIONS.length; i++) {
        await tauriCmd("set_dimension_weight", {
          dim: DIMENSIONS[i].key,
          weight: sliders[i].getValue(),
        });
      }

      // Save economy
      await tauriCmd("set_economy", {
        level: economySlider.getValue(),
      });

      showToast("Configuración guardada correctamente", "success");
    } catch (err) {
      showToast(`Error al guardar: ${err.message}`, "error");
    } finally {
      saveBtn.disabled = false;
      saveBtn.innerHTML = `
        <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/>
        </svg>
        Guardar Configuración
      `;
    }
  }

  // ---- Events ----
  saveBtn.addEventListener("click", saveConfig);

  // ---- Load on init ----
  loadConfig();
}
