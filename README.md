# 🧠 Cortex Gate

> **Extensible AI Platform** — Modular gateway for controlling AI token flows via plugins.

Cortex Gate is an extensible platform for routing, governing, and optimizing AI
LLM requests. It provides a pluggable architecture where functionality is
delivered through **extensions** — self-contained modules that register UI pages
and backend routes.

**Current status:** Early v0.2. The platform shell and extension system are
functional. Individual extensions (routing, governance, benchmarking) are under
active development.

---

## ✨ Current Features

| Feature | Status | Description |
|---------|--------|-------------|
| **Extension System** | ✅ Done | Registry with lazy-loaded pages, toggle enable/disable, localStorage persistence |
| **Prompt Router UI** | ✅ Done | Visual equalizer with 8 dimensions + economy slider |
| **Server Control** | ✅ Done | Tauri app manages backend lifecycle (start/stop/status) |
| **Backend API** | ✅ Done | OpenAI-compatible `/v1/chat/completions` endpoint |
| **SQLite Database** | ✅ Done | User, API key, and budget storage |
| **Provider Config** | ✅ Done | Multi-provider configuration (OpenAI, Anthropic, OpenRouter) |
| **Embedding Classifier** | 🟡 Stub | ONNX int8 embeddings — keyword heuristic fallback for now |
| **Benchmark Engine** | 🟡 Stub | Real model benchmarking — test framework exists, full integration pending |
| **Cost Governance** | 🟡 Stub | Token budgets and alerts — basic structure in place |
| **Streaming Proxy** | 🟡 Stub | SSE passthrough + Anthropic→OpenAI translation — wired but untested with real providers |

---

## 🗺️ Roadmap

### v0.2 (current) — Extensible Foundation
- [x] Extension registry (frontend: JS, lazy imports, toggle enable/disable)
- [x] Prompt Router extension (equalizer UI)
- [x] Server control from Tauri (start/stop/status)
- [x] Backend serves frontend static files (single port :18801)
- [ ] Wire classifier → actual ONNX embeddings
- [ ] Wire streaming → real provider calls
- [ ] Wire governance → token tracking + budget enforcement

### v0.3 — Functional Routing
- [ ] Complete embedding classifier (ONNX int8, dimension scoring)
- [ ] Real provider proxy (streaming SSE with keepalive)
- [ ] Token usage tracking + cost calculation
- [ ] Dashboard extension (real-time stats, charts)
- [ ] Rate limiting middleware (tower-governor)

### v0.4 — Production Hardening
- [ ] Circuit breaker per provider (tower-resilience)
- [ ] Retry with exponential backoff
- [ ] OpenAPI docs (utoipa-axum)
- [ ] Stress testing + performance benchmarks
- [ ] Prometheus metrics

### v0.5 — Extension Ecosystem
- [ ] Analytics extension
- [ ] Audit log extension
- [ ] A/B testing extension
- [ ] MCP / JSON-RPC protocol support

---

## 🏗️ Architecture

```
┌──────────────────────────────────────────────────┐
│                   Client                          │
│  (OpenAI-compatible SDK / HTTP)                   │
└──────────┬───────────────────────────────────────┘
           │ POST /v1/chat/completions
           ▼
┌──────────────────────────────────────────────────┐
│              Gateway (Rust / Axum)                │
│  :18801/api/*  +  :18801/  (static frontend)     │
│                                                   │
│  ┌──────────┐  ┌──────────┐  ┌────────────────┐  │
│  │ Auth     │  │ Routes   │  │ Extensions     │  │
│  │ Middleware│  │ Handlers │  │ (plugin loader)│  │
│  └──────────┘  └──────────┘  └────────────────┘  │
│                                                   │
│  ┌──────────┐  ┌──────────┐  ┌────────────────┐  │
│  │ProxyEngine│  │Classifier│  │ Governance     │  │
│  │(reqwest) │  │(ONNX)    │  │ (SQLite)       │  │
│  └──────────┘  └──────────┘  └────────────────┘  │
└──────────────────────────────────────────────────┘
           │
           ▼
┌──────────┴──────────┐     ┌──────────────────────┐
│   LLM Providers     │     │   Tauri Desktop App  │
│  OpenAI / Anthropic │     │  (Electron-like,     │
│  OpenRouter / Custom│     │   embeds frontend)   │
└─────────────────────┘     └──────────────────────┘
```

---

## 🚀 Quick Start

```bash
# 1. Build and run the backend
cargo build --release
./target/release/cortex-gate

# 2. Open in browser
# http://127.0.0.1:18801

# Or use the Tauri desktop app (Linux with display):
cd frontend/src-tauri && cargo build --release
DISPLAY=:0 ./target/release/cortex-gate-tauri
```

### Tauri Desktop Controls

1. Double-click the **Cortex Gate** icon
2. Click **Start Server** in the sidebar
3. Status dot turns green → **Server Online**
4. Click **Open Web UI** → browser opens `http://127.0.0.1:18801`
5. Click **Stop Server** to shut down

---

## 🧩 Extension System

Extensions are self-contained modules that register UI pages and optional
backend routes. Currently available:

| Extension | Status | Description |
|-----------|--------|-------------|
| **Prompt Router** | 🟡 Partial | Equalizer UI works; backend routing not yet wired to real providers |

To create a new extension:
```js
// frontend/src/extensions/my-ext/manifest.js
export const manifest = {
  id: "my-ext",
  name: "My Extension",
  description: "Does something cool",
  version: "0.1.0",
  pages: [{
    name: "my-ext",
    label: "My Ext",
    icon: "puzzle",
    load: () => import("./page.js"),
  }],
};
```
Then register it in `frontend/src/main.js`.

---

## 📖 Documentation

- [Project Origins](docs/origen-proyecto-perdido.md) — Historical context
- [Best Practices](.pi/best-practices.md) — Code standards and patterns
- [Code Suggestions](.pi/code-suggestions.md) — Pending improvements

---

## ⚖️ License

PolyForm Noncommercial License 1.0.0. See [LICENSE](LICENSE).

For commercial use, contact the author: **Luis Daniel Dos Santos**
