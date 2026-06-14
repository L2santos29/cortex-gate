// ====================================================================
// Extension Discovery — Auto-scan via Vite glob import
// ====================================================================

// Vite import.meta.glob discovers all manifest files automatically.
// Adding a new extension = creating a folder under extensions/.
// No code changes needed.

const manifestModules = import.meta.glob("./extensions/*/manifest.js", {
  eager: false,
});

/**
 * Discover and load all extension manifests from the extensions/ folder.
 * Returns an array of manifest objects.
 */
export async function discoverExtensions() {
  const manifests = [];
  for (const [path, loader] of Object.entries(manifestModules)) {
    try {
      const mod = await loader();
      if (mod && mod.manifest) {
        manifests.push(mod.manifest);
        console.log(`[discovery] Found extension: ${mod.manifest.id || path}`);
      }
    } catch (e) {
      console.warn(`[discovery] Failed to load extension manifest at ${path}:`, e);
    }
  }
  return manifests;
}

/**
 * Get the list of all discovered extension paths (for debugging).
 */
export function getExtensionPaths() {
  return Object.keys(manifestModules);
}
