// ====================================================================
// Extensions — built-in extension manager page
// ====================================================================

export const html = `
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <div>
        <h3 class="text-sm font-semibold text-slate-600">Installed Extensions</h3>
        <p class="text-xs text-slate-400 mt-0.5">Enable or disable extensions. Disabling an extension removes its pages from the sidebar.</p>
      </div>
    </div>
    <div id="extensions-list" class="space-y-3"></div>
  </div>
`;

export function init() {
  const cg = window.__cg;
  if (!cg) return;

  const list = document.getElementById("extensions-list");
  if (!list) return;

  const exts = cg.registry.getAllExtensions();

  if (exts.length === 0) {
    list.innerHTML = `
      <div class="empty-state">
        <svg fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" d="M11.48 3.499a.562.562 0 0 1 1.04 0l2.125 5.111a.563.563 0 0 0 .475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 0 0-.182.557l1.285 5.385a.562.562 0 0 1-.84.61l-4.725-2.885a.562.562 0 0 0-.586 0L6.982 20.54a.562.562 0 0 1-.84-.61l1.285-5.386a.562.562 0 0 0-.182-.557l-4.204-3.602a.562.562 0 0 1 .321-.988l5.518-.442a.563.563 0 0 0 .475-.345L11.48 3.5Z"/>
        </svg>
        <p>No extensions installed</p>
        <span class="sub">Extensions will appear here once installed in <code>frontend/src/extensions/</code>.</span>
      </div>
    `;
    return;
  }

  list.innerHTML = exts
    .map(
      (ext) => `
    <div class="ext-card flex items-start gap-4">
      <div class="flex-1 min-w-0">
        <div class="flex items-center gap-2.5">
          <h4 class="text-sm font-semibold text-slate-800">${esc(ext.name)}</h4>
          <span class="text-[10px] font-medium text-slate-400 bg-slate-100 px-1.5 py-0.5 rounded-full">v${esc(ext.version)}</span>
        </div>
        <p class="text-xs text-slate-500 mt-1">${esc(ext.description)}</p>
        <p class="text-[10px] text-slate-400 mt-1.5">by ${esc(ext.author)}</p>
      </div>
      <label class="toggle mt-0.5">
        <input type="checkbox" ${ext.enabled ? "checked" : ""} data-ext-id="${esc(ext.id)}" />
        <span class="toggle-slider"></span>
      </label>
    </div>
  `
    )
    .join("");

  // Bind toggle events
  list.querySelectorAll(".toggle input[type=checkbox]").forEach((cb) => {
    cb.addEventListener("change", () => {
      const extId = cb.dataset.extId;
      const enabled = cb.checked;
      cg.registry.toggleExtension(extId, enabled);
      if (enabled) {
        cg.showToast(`${extId} enabled`, "success");
      } else {
        cg.showToast(`${extId} disabled`, "info");
      }
    });
  });
}

function esc(str) {
  if (!str) return "";
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}
