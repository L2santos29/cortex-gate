// ====================================================================
// Prompt Router — Extension Manifest
// Routes prompts to optimal AI models based on embedding classification
// ====================================================================

export const manifest = {
  id: "prompt-router",
  name: "Prompt Router",
  description:
    "Routes prompts to optimal AI models based on embedding classification and cost preferences.",
  version: "0.1.0",
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
};
