// ====================================================================
// Prompt Router — Extension Page
// ====================================================================
// Equalizer UI: 8 dimension sliders + economy slider + profile view
// Uses window.__cg API from main.js
// ====================================================================

export const html = `
<div class="space-y-8">

  <!-- Dimension Sliders (8 vertical) -->
  <div class="config-section">
    <div class="flex items-center justify-between mb-4">
      <div>
        <h3 class="text-sm font-bold text-slate-700">Routing Profile</h3>
        <p class="text-xs text-slate-400 mt-0.5">Adjust each dimension to define the ideal model profile for this route.</p>
      </div>
      <button id="reset-profile" class="btn btn-ghost text-xs">Reset to Default</button>
    </div>

    <div id="sliders-row" class="flex items-end justify-between gap-2 sm:gap-4" style="height: 220px;">
      <div class="flex flex-col items-center flex-1 h-full min-w-0">
        <span class="slider-value text-[10px] font-bold text-slate-600 mb-1.5">50</span>
        <div class="flex-1 w-full" data-dim="speed" data-color="#3b82f6" data-value="0.5"></div>
        <span class="text-[10px] font-medium text-slate-400 mt-2 text-center leading-tight">Speed</span>
      </div>
      <div class="flex flex-col items-center flex-1 h-full min-w-0">
        <span class="slider-value text-[10px] font-bold text-slate-600 mb-1.5">50</span>
        <div class="flex-1 w-full" data-dim="math" data-color="#8b5cf6" data-value="0.5"></div>
        <span class="text-[10px] font-medium text-slate-400 mt-2 text-center leading-tight">Math<br/>Logic</span>
      </div>
      <div class="flex flex-col items-center flex-1 h-full min-w-0">
        <span class="slider-value text-[10px] font-bold text-slate-600 mb-1.5">50</span>
        <div class="flex-1 w-full" data-dim="creativity" data-color="#ec4899" data-value="0.5"></div>
        <span class="text-[10px] font-medium text-slate-400 mt-2 text-center leading-tight">Creativity</span>
      </div>
      <div class="flex flex-col items-center flex-1 h-full min-w-0">
        <span class="slider-value text-[10px] font-bold text-slate-600 mb-1.5">50</span>
        <div class="flex-1 w-full" data-dim="precision" data-color="#f59e0b" data-value="0.5"></div>
        <span class="text-[10px] font-medium text-slate-400 mt-2 text-center leading-tight">Precision</span>
      </div>
      <div class="flex flex-col items-center flex-1 h-full min-w-0">
        <span class="slider-value text-[10px] font-bold text-slate-600 mb-1.5">50</span>
        <div class="flex-1 w-full" data-dim="context" data-color="#10b981" data-value="0.5"></div>
        <span class="text-[10px] font-medium text-slate-400 mt-2 text-center leading-tight">Context<br/>Length</span>
      </div>
      <div class="flex flex-col items-center flex-1 h-full min-w-0">
        <span class="slider-value text-[10px] font-bold text-slate-600 mb-1.5">50</span>
        <div class="flex-1 w-full" data-dim="code" data-color="#6366f1" data-value="0.5"></div>
        <span class="text-[10px] font-medium text-slate-400 mt-2 text-center leading-tight">Code Gen</span>
      </div>
      <div class="flex flex-col items-center flex-1 h-full min-w-0">
        <span class="slider-value text-[10px] font-bold text-slate-600 mb-1.5">50</span>
        <div class="flex-1 w-full" data-dim="reasoning" data-color="#14b8a6" data-value="0.5"></div>
        <span class="text-[10px] font-medium text-slate-400 mt-2 text-center leading-tight">Reasoning</span>
      </div>
      <div class="flex flex-col items-center flex-1 h-full min-w-0">
        <span class="slider-value text-[10px] font-bold text-slate-600 mb-1.5">50</span>
        <div class="flex-1 w-full" data-dim="vision" data-color="#f97316" data-value="0.5"></div>
        <span class="text-[10px] font-medium text-slate-400 mt-2 text-center leading-tight">Vision</span>
      </div>
    </div>
  </div>

  <!-- Economy Slider -->
  <div class="config-section">
    <div class="flex items-center justify-between mb-4">
      <div>
        <h3 class="text-sm font-bold text-slate-700">Economy</h3>
        <p class="text-xs text-slate-400 mt-0.5">Balance between cost savings and response quality.</p>
      </div>
      <span id="economy-label" class="text-xs font-bold text-slate-600">Balanced</span>
    </div>
    <div id="economy-slider-container" class="py-3"></div>
    <div class="flex justify-between text-[10px] text-slate-400 font-medium mt-1">
      <span>Max Savings</span>
      <span>Best Quality</span>
    </div>
  </div>

  <!-- Profile Preview -->
  <div class="config-section">
    <div class="flex items-center justify-between mb-4">
      <div>
        <h3 class="text-sm font-bold text-slate-700">Assigned Model</h3>
        <p class="text-xs text-slate-400 mt-0.5">The model that best matches this routing profile.</p>
      </div>
    </div>

    <div id="profile-result" class="flex items-center justify-center min-h-[120px]">
      <div class="text-center">
        <div id="profile-model" class="text-lg font-bold text-slate-700">—</div>
        <div id="profile-confidence" class="text-xs text-slate-400 mt-1"></div>
      </div>
    </div>

    <div class="flex justify-end mt-4 pt-3 border-t border-slate-100">
      <button id="save-profile" class="btn-save flex items-center gap-2">
        <svg class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5"/>
        </svg>
        Save Configuration
      </button>
    </div>
  </div>
</div>
`;

// -------------------------------------------------------------------
// Init
// -------------------------------------------------------------------
export function init() {
  const cg = window.__cg;
  if (!cg) return;

  const dimensions = [
    "speed", "math", "creativity", "precision",
    "context", "code", "reasoning", "vision",
  ];

  const values = {
    speed: 0.5, math: 0.5, creativity: 0.5, precision: 0.5,
    context: 0.5, code: 0.5, reasoning: 0.5, vision: 0.5,
  };
  let economy = 0.5;
  const sliderControllers = [];

  // Create 8 vertical sliders
  for (const dim of dimensions) {
    const container = document.querySelector(`[data-dim="${dim}"]`);
    if (!container) continue;
    const color = container.dataset.color || "#06b6d4";
    const initialVal = parseFloat(container.dataset.value) || 0.5;

    const valueSpan = container.parentElement.querySelector(".slider-value");

    const ctrl = cg.createVerticalSlider(container, {
      min: 0, max: 1, value: initialVal, color,
      onChange: (val) => {
        values[dim] = val;
        valueSpan.textContent = Math.round(val * 100);
        updateProfile(values, economy);
      },
    });
    sliderControllers.push(ctrl);
  }

  // Economy horizontal slider
  const ecoContainer = document.getElementById("economy-slider-container");
  let ecoCtrl = null;
  if (ecoContainer) {
    ecoCtrl = cg.createHorizontalSlider(ecoContainer, {
      min: 0, max: 1, value: 0.5, color: "#06b6d4",
      onChange: (val) => {
        economy = val;
        updateEconomyLabel(val);
        updateProfile(values, economy);
      },
    });
  }

  // Reset button
  document.getElementById("reset-profile")?.addEventListener("click", () => {
    for (const dim of dimensions) {
      values[dim] = 0.5;
      const container = document.querySelector(`[data-dim="${dim}"]`);
      if (container) {
        const idx = dimensions.indexOf(dim);
        if (sliderControllers[idx]) sliderControllers[idx].setValue(0.5);
      }
      const vs = container?.parentElement.querySelector(".slider-value");
      if (vs) vs.textContent = "50";
    }
    economy = 0.5;
    if (ecoContainer) {
      ecoContainer.innerHTML = "";
      ecoCtrl = cg.createHorizontalSlider(ecoContainer, {
        min: 0, max: 1, value: 0.5, color: "#06b6d4",
        onChange: (val) => {
          economy = val;
          updateEconomyLabel(val);
          updateProfile(values, economy);
        },
      });
    }
    document.getElementById("economy-label").textContent = "Balanced";
    updateProfile(values, economy);
  });

  // Save button
  document.getElementById("save-profile")?.addEventListener("click", async () => {
    const profile = { dimensions: toPct(values), economy: Math.round(economy * 100) };
    try {
      await cg.tauriCmd("save_routing_profile", { profile: JSON.stringify(profile) });
      cg.showToast("Routing profile saved successfully", "success");
    } catch {
      localStorage.setItem("cg:prompt-router:profile", JSON.stringify(profile));
      cg.showToast("Profile saved locally (backend not connected)", "success");
    }
  });

  // Initial update
  updateProfile(values, economy);
}

function updateEconomyLabel(val) {
  const label = document.getElementById("economy-label");
  if (!label) return;
  const pct = val * 100;
  if (pct < 30) label.textContent = "Maximum Savings";
  else if (pct < 50) label.textContent = "Savings Focused";
  else if (pct < 60) label.textContent = "Balanced";
  else if (pct < 80) label.textContent = "Quality Focused";
  else label.textContent = "Best Quality";
}

function toPct(obj) {
  const r = {};
  for (const [k, v] of Object.entries(obj)) r[k] = Math.round(v * 100);
  return r;
}

// -------------------------------------------------------------------
// Profile computation
// -------------------------------------------------------------------
function updateProfile(values, economy) {
  const modelEl = document.getElementById("profile-model");
  const confEl = document.getElementById("profile-confidence");
  if (!modelEl) return;

  const models = [
    { name: "GPT-4o", profile: { speed: 0.4, math: 0.9, creativity: 0.6, precision: 0.95, context: 0.7, code: 0.85, reasoning: 0.9, vision: 0.95 }, econTilt: 0.3 },
    { name: "GPT-4o-mini", profile: { speed: 0.8, math: 0.7, creativity: 0.55, precision: 0.75, context: 0.6, code: 0.7, reasoning: 0.65, vision: 0.8 }, econTilt: 0.1 },
    { name: "Claude 3.5 Sonnet", profile: { speed: 0.45, math: 0.85, creativity: 0.85, precision: 0.9, context: 0.95, code: 0.8, reasoning: 0.9, vision: 0.8 }, econTilt: 0.4 },
    { name: "Claude 3 Haiku", profile: { speed: 0.9, math: 0.55, creativity: 0.6, precision: 0.65, context: 0.5, code: 0.6, reasoning: 0.55, vision: 0.5 }, econTilt: 0.15 },
    { name: "Gemini 1.5 Pro", profile: { speed: 0.5, math: 0.8, creativity: 0.7, precision: 0.8, context: 1.0, code: 0.75, reasoning: 0.8, vision: 0.9 }, econTilt: 0.35 },
    { name: "Gemini 1.5 Flash", profile: { speed: 0.95, math: 0.55, creativity: 0.5, precision: 0.6, context: 0.7, code: 0.55, reasoning: 0.5, vision: 0.6 }, econTilt: 0.1 },
  ];

  let best = null;
  let bestScore = -Infinity;

  for (const m of models) {
    let score = 0;
    for (const [dim, val] of Object.entries(values)) {
      const target = m.profile[dim] ?? 0.5;
      const diff = Math.abs(val - target);
      score += Math.max(0, 1 - diff) ** 2;
    }
    const econFactor = m.econTilt <= economy ? 1.0 : Math.max(0.5, 1 - (m.econTilt - economy));
    score *= econFactor;

    if (score > bestScore) {
      bestScore = score;
      best = m;
    }
  }

  if (best) {
    const maxPossible = Object.keys(values).length;
    const confidence = Math.round((bestScore / maxPossible) * 100);
    modelEl.textContent = best.name;
    confEl.textContent = `Confidence: ${Math.min(confidence, 98)}% — Economy tilt: ${Math.round(economy * 100)}%`;
  }
}
