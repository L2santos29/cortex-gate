# Cortex Gate — Full Extension System: Architecture & Implementation Plan

> **Goal:** True extension system where extensions are fully independent, installable/uninstallable at runtime.
> **Architecture:** Hybrid — Frontend (JS dynamic imports + EventBus) + Backend (Rust trait-based + Tower middleware)
> **Isolation Tiers:** Tier 1 (trusted built-in) → Tier 2 (semi-trusted, Shadow DOM) → Tier 3 (untrusted, iframe sandbox)

---

## Design Principles

1. **Manifest-driven** — every extension declares capabilities in `manifest.json`
2. **Lazy loading** — extensions activate only when needed, not at startup
3. **Unique IDs** — all contributions prefixed with extension ID (no conflicts)
4. **Lifecycle hooks** — `onLoad → onEnable → Running → onDisable → onUnload`
5. **Contribution points** — named extension slots in host (sidebar, settings, header, status bar)
6. **Storage isolation** — scoped localStorage + DB schema per extension
7. **Graceful degradation** — host never crashes from extension error (try/catch boundaries)
8. **Capability-based security** — extensions only access what's declared in permissions

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    CORTEX GATE HOST                              │
│                                                                 │
│  ┌─────────────────────────┐   ┌────────────────────────────┐  │
│  │  Frontend Extension     │   │  Backend Extension          │  │
│  │  Engine (JS)            │   │  Engine (Rust)              │  │
│  │                         │   │                             │  │
│  │  ExtensionRegistry      │   │  ExtensionManager           │  │
│  │  EventBus               │   │  ExtensionTrait             │  │
│  │  ContributionPoints     │   │  ProviderPlugin             │  │
│  │  Sandbox (iframe/SDOM)  │   │  Dynamic Routes             │  │
│  │  Discovery (glob)       │   │  PermissionChecker          │  │
│  └──────────┬──────────────┘   └──────────┬──────────────────┘  │
│             │                              │                     │
│             └──────────┬───────────────────┘                     │
│                        ▼                                        │
│             ┌──────────────────────┐                             │
│             │   Extension          │                             │
│             │   Manifest (JSON)    │                             │
│             │   id, name, version  │                             │
│             │   permissions, contrib│                            │
│             │   entry, sandbox     │                             │
│             └──────────────────────┘                             │
└─────────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Extension Manifest

Extensions declare themselves via `manifest.json`. This is read BEFORE any code loads.

```json
{
  "id": "com.cortex.prompt-router",
  "name": "Prompt Router",
  "version": "0.2.0",
  "description": "Routes prompts to optimal AI models based on embedding classification",
  "author": { "name": "Cortex Gate", "url": "https://cortex-gate.local" },
  "license": "MIT",
  "minAppVersion": "0.2.0",

  "entry": {
    "frontend": "./page.js",
    "backend": null
  },

  "permissions": [
    "providers:list",
    "config:read",
    "storage:ext"
  ],

  "contrib": {
    "pages": [
      { "name": "prompt-router", "label": "Prompt Router", "icon": "equalizer" }
    ],
    "providers": [],
    "commands": [],
    "settings": [
      { "key": "default_model", "type": "string", "default": "gpt-4o-mini", "label": "Default Model" },
      { "key": "auto_route", "type": "boolean", "default": true, "label": "Auto-route prompts" }
    ],
    "hooks": {
      "onBeforeCommand": false,
      "onAfterPageLoad": false
    }
  },

  "dependencies": [],
  "sandbox": { "type": "shadow-dom" }
}
```

**Validation:** JSON Schema validation on register — reject unknown fields, validate semver, check permissions against host's allowlist.

---

## Layer 2: Frontend Extension Engine (JS)

### A. Discovery — Auto-scan via Vite Glob Import

Current problem: `extensions/prompt-router/manifest.js` is hardcoded in `main.js`.

Solution: Vite's `import.meta.glob` auto-discovers all extensions:

```js
// discovery.js
const manifestModules = import.meta.glob('./extensions/*/manifest.json', { eager: false });

export async function discoverExtensions() {
  const manifests = [];
  for (const [path, loader] of Object.entries(manifestModules)) {
    try {
      const mod = await loader();
      manifests.push(mod);
    } catch (e) {
      console.warn(`Failed to load extension manifest: ${path}`, e);
    }
  }
  return manifests;
}
```

This discovers any folder under `extensions/` that has a `manifest.json`. No code changes needed to add new extensions.

### B. ExtensionRegistry — Full Lifecycle

```js
class ExtensionRegistry {
  constructor() {
    this.extensions = new Map();   // id → ExtensionRecord
    this.contributions = new Map(); // type → Map<name, Contribution>
    this.eventBus = new EventBus();
  }

  async install(manifestPath) { /* load manifest, copy to extensions/ */ }
  async uninstall(id) { /* call onUnload, remove files, clean storage */ }
  async enable(id) { /* call onEnable, register contributions */ }
  async disable(id) { /* call onDisable, unregister contributions */ }

  register(manifest) { /* existing logic + contrib registration */ }
  unregister(id) { /* full cleanup */ }

  getContributions(type) { /* get all contributions of a type */ }
  getExtension(id) { /* get extension record */ }
  getAllExtensions() { /* list all for manager UI */ }
}

class EventBus {
  constructor() { this.listeners = {}; }

  on(event, handler) {
    (this.listeners[event] ??= []).push(handler);
    return () => this.off(event, handler); // returns disposable
  }

  emit(event, data) {
    (this.listeners[event] ?? []).forEach(fn => fn(data));
  }

  off(event, handler) {
    this.listeners[event] = this.listeners[event]?.filter(l => l !== handler);
  }
}
```

### C. Page Module — Updated Interface

Current: `{ html, css, init }`

New: `{ manifest, html, css, init(ctx), destroy(), onResume(), onPause() }`

```js
// extensions/my-ext/page.js
export const manifest = { /* extension metadata */ };

export const html = `<div class="my-ext-root">...</div>`;

export const css = `/* scoped via Shadow DOM automatically */`;

export function init(ctx) {
  // ctx.platform — window.__cg API
  // ctx.extension — extension id, settings, permissions
  // ctx.eventBus — for cross-extension communication
  // ctx.settings — settings values from manifest defaults
  const { platform, settings } = ctx;
  platform.showToast("Extension loaded", "success");
}

export function destroy() {
  // Cleanup: remove event listeners, clear intervals, destroy sliders
}
```

### D. Sandboxing Tiers

| Tier | Technique | JS Isolation | CSS Isolation | Trust Level |
|------|-----------|-------------|--------------|-------------|
| 1 | Direct import + Shadow DOM | None | Complete (Shadow DOM) | Built-in, verified |
| 2 | Proxy API + Shadow DOM | Medium (Proxy traps) | Complete | Community extensions |
| 3 | iframe sandbox + postMessage | Complete | Complete | Untrusted third-party |

---

## Layer 3: Backend Extension Engine (Rust)

### A. CortexExtension Trait

```rust
use async_trait::async_trait;
use axum::routing::MethodRouter;

#[async_trait]
pub trait CortexExtension: Send + Sync + 'static {
    fn manifest(&self) -> &ExtensionManifest;
    fn id(&self) -> &str { self.manifest().id }

    /// Called once when extension is loaded (not yet active)
    async fn init(&mut self, ctx: &ExtensionContext) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Called when extension is enabled
    async fn on_enable(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    /// Called when extension is disabled
    async fn on_disable(&mut self) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    /// Custom HTTP routes this extension registers
    fn routes(&self) -> Vec<(String, MethodRouter)> { vec![] }

    /// Custom LLM providers this extension adds
    fn providers(&self) -> Vec<Box<dyn ProviderPlugin>> { vec![] }

    /// Custom Tower middleware layers
    fn middleware(&self) -> Vec<Box<dyn tower::Layer<axum::body::Body>>> { vec![] }

    /// Custom Tauri commands
    fn commands(&self) -> Vec<ExtensionCommand> { vec![] }
}
```

### B. ExtensionContext — What Extensions Can Access

```rust
pub struct ExtensionContext {
    pub id: String,
    pub config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    pub db: Arc<Database>,
    pub http_client: reqwest::Client,
    pub event_bus: Arc<EventBus>,
}

pub struct EventBus {
    listeners: Arc<RwLock<HashMap<String, Vec<Box<dyn Fn(&Value) + Send + Sync>>>>>,
}

impl EventBus {
    pub fn emit(&self, event: &str, data: &Value);
    pub fn on(&self, event: &str, handler: Box<dyn Fn(&Value) + Send + Sync>) -> EventHandle;
}
```

### C. ProviderPlugin — Open Provider Registration

```rust
#[async_trait]
pub trait ProviderPlugin: Send + Sync {
    /// Unique provider type identifier (e.g., "ollama", "gemini")
    fn provider_type(&self) -> &str;

    /// Build a reqwest client with provider-specific headers
    async fn build_client(&self, config: &ProviderConfig) -> Result<reqwest::Client, String>;

    /// Build request URL + body for chat completion
    fn build_request(&self, model: &str, messages: Value, stream: bool) -> (String, Value);

    /// Normalize provider-specific response to OpenAI-compatible format
    fn normalize_response(&self, raw: Value) -> Value;
}
```

### D. ExtensionManager — Lifecycle Orchestrator

```rust
pub struct ExtensionManager {
    extensions: HashMap<String, Box<dyn CortexExtension>>,
}

impl ExtensionManager {
    pub fn new() -> Self;

    /// Register a compiled-in extension
    pub fn register(&mut self, ext: Box<dyn CortexExtension>);

    /// Initialize all registered extensions
    pub async fn init_all(&mut self, ctx: &ExtensionContext) -> Result<(), Vec<(String, BoxError)>>;

    /// Enable/disable at runtime
    pub async fn enable(&mut self, id: &str) -> Result<()>;
    pub async fn disable(&mut self, id: &str) -> Result<()>;

    /// Collect all routes from extensions for the router
    pub fn collect_routes(&self) -> Vec<(String, MethodRouter)>;

    /// Collect all providers from extensions
    pub fn collect_providers(&self) -> Vec<Box<dyn ProviderPlugin>>;

    /// Get extension by ID
    pub fn get(&self, id: &str) -> Option<&Box<dyn CortexExtension>>;
}
```

### E. Wiring into Axum Router

```rust
// In server.rs build_router:
pub fn build_router(state: Arc<AppState>, ext_manager: &ExtensionManager) -> Router {
    let mut router = Router::new()
        .route("/health", get(routes::health))
        .route("/v1/models", get(routes::models))
        .route("/v1/chat/completions", post(routes::chat_completions))
        .route("/admin/config", get(routes::admin_config_get).post(routes::admin_config_post))
        .route("/extensions", get(routes::extensions_list))
        .route("/extensions/:id/enable", post(routes::extension_enable))
        .route("/extensions/:id/disable", post(routes::extension_disable));

    // Add extension routes
    for (path, method_router) in ext_manager.collect_routes() {
        router = router.route(&path, method_router);
    }

    // Apply extension middleware
    for layer in ext_manager.collect_middleware() {
        router = router.layer(layer);
    }

    router
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(cors)
        .with_state(state)
}
```

---

## Layer 4: Extension Management API

### Backend HTTP Routes

```
GET    /extensions               → List installed extensions with status
GET    /extensions/:id           → Extension details (manifest, status, settings)
POST   /extensions/install       → Install from path or uploaded package
POST   /extensions/:id/enable    → Enable extension
POST   /extensions/:id/disable   → Disable extension
POST   /extensions/:id/uninstall → Remove extension completely
GET    /extensions/:id/settings  → Get extension settings
POST   /extensions/:id/settings  → Update extension settings
```

### Frontend Extension Manager UI (Upgraded)

Current extensions.js is minimal. Needs:

- **Extension cards** with: icon, name, version badge, status badge (enabled/disabled/error), description, author
- **Toggle switch** — enable/disable with animation
- **Detail panel** (expandable): pages list, permissions, settings form, dependencies
- **Install button** — file picker (local folder/zip) or URL input
- **Uninstall button** — confirmation modal, removes files and data
- **Refresh button** — rescan extensions directory
- **Error badges** — extensions that failed to load show error detail

---

## Implementation Phases

### Phase 1 — Foundation (v0.3)
**Goal:** Working extension system for built-in extensions only.

**Frontend (4-5 days):**
- [ ] Rewrite `ExtensionRegistry` with full lifecycle (register/unregister/enable/disable)
- [ ] Add `destroy()`, `onResume()`, `onPause()` to page module interface
- [ ] Implement Vite `import.meta.glob` for auto-discovery
- [ ] Add `EventBus` for cross-extension communication
- [ ] Create `ExtensionContext` API object passed to `init(ctx)`
- [ ] Add contribution points: sidebar, settings panels
- [ ] Upgrade extension manager UI (detail panel, status badges, install button)
- [ ] Update Prompt Router extension to new manifest + lifecycle

**Backend (4-5 days):**
- [ ] Define `CortexExtension` trait in new `src/extensions/mod.rs`
- [ ] Create `ExtensionManager` with `register()`, `init_all()`, `collect_routes()`
- [ ] Add extension route collection to `build_router()`
- [ ] Create `ExtensionContext` with DB access, config, EventBus
- [ ] Make `ProviderType` extensible (trait-based)
- [ ] Add extension management API routes to `routes.rs`
- [ ] Wire `ExtensionManager` into `create_app()`

### Phase 2 — Dynamic Extensions (v0.4)
**Goal:** Install/uninstall without code changes.

- [ ] Folder-based extension discovery (scan `extensions/` directory)
- [ ] Install from local folder (file picker in UI → copy to extensions/)
- [ ] Uninstall with full cleanup (remove files, deactivate, delete scoped data)
- [ ] Extension settings UI (auto-generated from manifest `settings` schema)
- [ ] `ProviderPlugin` trait and registration in ExtensionManager
- [ ] Dynamic route registration (add/remove routes without server restart)
- [ ] Permission checking middleware (runtime enforcement)
- [ ] Version compatibility check (`minAppVersion` field)

### Phase 3 — Sandbox & Security (v0.5)
**Goal:** Third-party extensions with isolation guarantees.

- [ ] Shadow DOM CSS isolation for all extension pages (automatic)
- [ ] iframe sandbox tier for untrusted extensions
- [ ] Permission system (declarative in manifest + runtime enforcement)
- [ ] Storage isolation (prefixed localStorage keys, scoped DB tables)
- [ ] Error boundaries per extension (crash → graceful recovery, toast notification)
- [ ] Rate limiting per-extension (calls/second)
- [ ] Audit logging of extension actions (install, enable, api calls)

### Phase 4 — Ecosystem (v0.6+)
**Goal:** Extension registry, SDK, community contributions.

- [ ] Extension template / CLI scaffolding tool (`cortex new-extension`)
- [ ] Extension documentation site
- [ ] WASM plugin support for hot-reloadable routing logic
- [ ] Rhai scripting for policy/rule extensions (custom content filters)
- [ ] Remote extension registry (download from URL, verify signature)
- [ ] Extension update mechanism (check for updates, auto-download)
- [ ] Extension marketplace UI (browse, ratings, install from list)

---

## File Map (Phase 1)

### New Files
```
frontend/src/extensions/registry.js      — Full ExtensionRegistry + EventBus
frontend/src/extensions/discovery.js     — import.meta.glob auto-discovery
frontend/src/extensions/context.js       — ExtensionContext API object
frontend/src/extensions/contribution.js  — Contribution point manager
frontend/src/extensions/sandbox.js       — Shadow DOM helpers
src/extensions/mod.rs                    — CortexExtension trait + ExtensionManifest
src/extensions/manager.rs                — ExtensionManager
src/extensions/context.rs                — ExtensionContext
src/extensions/provider_plugin.rs        — ProviderPlugin trait
src/extensions/event_bus.rs              — EventBus
```

### Modified Files
```
frontend/src/main.js                     — Wire discovery, upgrade registry usage
frontend/vite.config.js                  — Add glob config for extensions
frontend/src/pages/extensions.js         — UPGRADED extension manager UI
frontend/src/extensions/prompt-router/manifest.json — New manifest format
frontend/src/extensions/prompt-router/page.js     — Add destroy(), onResume()
src/gateway/server.rs                    — Extension-aware build_router
src/gateway/routes.rs                    — Add extension management endpoints
src/lib.rs                               — Extension init loop
src/tools/provider.rs                    — Open ProviderType for extensions
Cargo.toml                               — Extension features
```

---

## Migration Path

### From old manifest format:
```diff
- export const manifest = { id, name, description, version, author, pages, hooks }
+ // manifest.json file (separate from code)
+ { id, name, version, description, entry: { frontend: "./page.js" }, permissions, contrib }
```

### From old page module:
```diff
- export function init() { /* no args */ }
+ export function init(ctx) { /* ExtensionContext with platform API */ }
+ export function destroy() { /* cleanup */ }
```

Backward compatibility: old format pages still load with deprecation warning logged to console.

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Extension crash takes down host | Low | High | try/catch around every extension hook, isolated spawn for backend tasks |
| Permission bypass (extension accesses things it shouldn't) | Low | High | Runtime permission check at every access point, deny by default |
| Vite glob not finding extension manifests | Low | Medium | Fallback to explicit import list, log warning during dev |
| Rust trait object safety (CortexExtension with generic methods) | Medium | High | Use `Box<dyn>`, avoid generic parameters in trait, use `async_trait` macro |
| Two extensions register same page name | Low | Medium | Prefix all contribution IDs with extension ID (enforced by registry) |
| Extension memory leak (forgets to clean up) | Medium | Low | Platform tracks all registrations, auto-cleans on disable |
| Dynamic route removal races with in-flight requests | Low | Medium | Drain active connections before removing routes, use graceful shutdown |

---

## Quick Start: Creating an Extension (Phase 1)

1. Create folder: `frontend/src/extensions/my-ext/`
2. Add `manifest.json`:
   ```json
   { "id": "com.cortex.my-ext", "name": "My Ext", "version": "0.1.0", "entry": { "frontend": "./page.js" }, "permissions": [], "contrib": { "pages": [{ "name": "my-ext", "label": "My Ext", "icon": "puzzle" }] }, "sandbox": { "type": "shadow-dom" } }
   ```
3. Add `page.js`:
   ```js
   export const html = `<div><h2>My Extension</h2><p>Hello!</p></div>`;
   export function init(ctx) { ctx.platform.showToast("My Ext loaded!", "success"); }
   export function destroy() { /* cleanup */ }
   ```
4. Done — Vite glob discovers it automatically. No code changes needed.
