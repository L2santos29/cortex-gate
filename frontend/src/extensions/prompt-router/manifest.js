// ====================================================================
// Prompt Router — Extension Manifest
// Routes prompts to optimal AI models based on embedding classification and cost preferences.
// ====================================================================

export const manifest = {
  id: "prompt-router",
  name: "Prompt Router",
  description:
    "Routes prompts to optimal AI models based on embedding classification and cost preferences.",
  version: "0.2.0",
  author: "Cortex Gate",
  enabledDefault: true,

  pages: [
    {
      name: "prompt-router",
      label: "Prompt Router",
      icon: "equalizer",
      async load() {
        return import("./page.js");
      },
    },
  ],

  hooks: {
    onBeforeCommand: null,
    onAfterPageLoad: null,
  },

  onInit(ctx) {
    console.log("[Prompt Router] Initialized with storage:", ctx.storage);
  },

  onEnable() {
    console.log("[Prompt Router] Enabled");
  },

  onDisable() {
    console.log("[Prompt Router] Disabled");
  },
};
