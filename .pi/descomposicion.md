# Árbol Binario — Cortex Gate v0.1

```
                    Cortex Gate v0.1
                   /                \
          Backend Runtime          Desktop UI
         /        |        \          |
    Gateway    ML Engine    Gov       Tauri App
    /     \    /     \      |        /       \
  Server  Proxy Class  Bench DB     Frontend  Backend
  + Auth  + Prov +ONNX +Tests +Quotas+Ecuali  +Bridge
```

## Tareas Atómicas (8)

### Fase A — Independientes (PARALELO TOTAL)

| ID | Componente | Descripción | Archivos clave | Output |
|----|-----------|-------------|----------------|--------|
| G1 | Gateway Server | Axum server, OpenAI /v1/chat/completions, routing engine struct, auth middleware, admin API | src/gateway/{mod,server,auth,routes}.rs src/main.rs Cargo.toml | src/gateway/ listos |
| G2 | Multi-Provider Proxy | HTTP client pool, OpenRouter/Anthropic/OpenAI forwarders, SSE streaming, tool call translation | src/gateway/{providers,streaming}.rs src/tools/provider.rs | src/gateway/providers.rs |
| C1 | Embedding Classifier | ONNX model load, inference engine, vector embedding, cosine similarity | src/classifier/{mod,embedding,onnx}.rs | src/classifier/ listo |
| C2 | Routing Equation | Dimension system, ecualizador weights, economy factor, model selector, tier logic | src/classifier/{dimensions,equation}.rs src/models/config.rs | src/classifier/dimensions.rs |
| B1 | Benchmark Engine | Test library, LLM caller, scorer, profile storage | src/benchmark/{mod,engine,tests,scorer,profiles}.rs | src/benchmark/ listo |
| GV1 | Governance DB + Users | SQLite schema, user CRUD, API key management, quota models | src/governance/{mod,database,users}.rs | src/governance/ listo |
| GV2 | Token Tracking + Budgets | Token counting per request, budget enforcement, hourly/daily/monthly caps, alerts | src/governance/{tracking,quotas,alerts}.rs | src/governance/tracking.rs |
| F1 | Tauri Frontend UI | Tauri v2 scaffold, Tailwind CSS, ecualizador page, dashboard page, config page, bridge | frontend/ (src + config) | frontend/ listo |

### Fase B — Ensamblaje (depende de Fase A)

| ID | Componente | Descripción | Dependencias |
|----|-----------|-------------|-------------|
| EA | Integración | Cargo.toml final, module wiring, lib.rs, build verification, binary output | G1, G2, C1, C2, B1, GV1, GV2, F1 |

## Dependencias
- G1 ← G2 (G2 necesita los tipos de G1, pero pueden desarrollarse en paralelo con interfaces claras)
- G1 + G2 + C2 → integración de routing
- GV1 ← GV2 (GV2 necesita el schema de GV1)
- G1 + GV1 + GV2 → integración de governance en gateway
- F1 → necesita la API shape (puerto, endpoints) pero puede mockearla

## Paralelización
```
Fase A: G1 + G2 + C1 + C2 + B1 + GV1 + GV2 + F1 → 8 PANELES EN PARALELO
Fase B: EA → 1 panel (ensamblaje final)
```
