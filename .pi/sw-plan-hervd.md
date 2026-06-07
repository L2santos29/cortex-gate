# Plan: Cortex Gate v0.1 — Boceto Inicial

> Generado: 2026-06-05 | Versión: 0.1.0 MVP
> Archivo maestro de planificación para start-workflow multi-agente.

---

## Árbol de Descomposición

```
                    Cortex Gate v0.1
                   /                \
          Backend Runtime          Desktop UI
         /        |        \          |
    Gateway    ML Engine    Gov       Tauri App
    /     \    /     \      |        /       \
  Server  Proxy Class  Bench DB     Frontend  Bridge
  + Auth  + Prov +ONNX +Tests +Quotas+Ecuali  +API
```

---

## Fase A: Tareas Atómicas (PARALELO TOTAL — 8 paneles)

| ID | Descripción | Personalidad | Tools | Contexto | Dependencias |
|----|-------------|-------------|-------|----------|-------------|
| G1 | **Gateway Server**: axum server con /v1/chat/completions, auth middleware (admin + client), admin API, CORS, logging, estructura de routing engine | backend-architect | --exclude-tools read,edit,grep,find | src/gateway/{mod,server,auth,routes}.rs, src/main.rs | - |
| G2 | **Multi-Provider Proxy**: clientes HTTP para OpenRouter, Anthropic, OpenAI; forwarder unificado; SSE streaming proxy; traducción de tool calling entre formatos | backend-architect | --exclude-tools read,edit,grep,find | src/gateway/{providers,streaming}.rs, src/tools/provider.rs | - |
| C1 | **Embedding Classifier ONNX**: carga de modelo ONNX int8, inference engine, generación de embeddings, cosine similarity, integración con `ort` crate | data-engineer | --exclude-tools read,edit,grep,find | src/classifier/{mod,embedding,onnx}.rs | - |
| C2 | **Routing Equation + Dimension System**: 8 dimensiones (razonamiento, código, creatividad, mates, precisión, velocidad, contexto, seguridad), sistema de pesos, factor economía (bajo/medio/alto), selector de modelo, threshold de confianza | backend-architect | --exclude-tools read,edit,grep,find | src/classifier/{dimensions,equation}.rs, src/models/config.rs | - |
| B1 | **Benchmark Engine**: test library interna (prompts + expected outputs), LLM caller para ejecutar tests, scorer automático, almacenamiento de perfiles por modelo | data-engineer | --exclude-tools read,edit,grep,find | src/benchmark/{mod,engine,tests,scorer,profiles}.rs | - |
| GV1 | **Governance DB + Usuarios**: schema SQLite (usuarios, API keys, cuotas, budgets), CRUD de usuarios, manejo de API keys, modelos de datos | data-engineer | --exclude-tools read,edit,grep,find | src/governance/{mod,database,users}.rs | - |
| GV2 | **Token Tracking + Budgets**: contador de tokens por request, enforcement de budgets por hora/día/semana/mes, alertas automáticas, corte por superación | backend-architect | --exclude-tools read,edit,grep,find | src/governance/{tracking,quotas,alerts}.rs | - |
| F1 | **Tauri Frontend**: scaffold Tauri v2, Tailwind CSS, página ecualizador (8 sliders + perilla economía), dashboard (stats, costos por usuario), panel config, bridge Rust↔Frontend | frontend-developer | --exclude-tools read,edit,grep,find | frontend/ (Tauri + Tailwind) | - |

## Fase B: Ensamblaje

| ID | Descripción | Personalidad | Tools | Contexto | Dependencias |
|----|-------------|-------------|-------|----------|-------------|
| EA | **Integración Final**: wireo de todos los módulos en lib.rs, Cargo.toml final con dependencias correctas, build verification, tests de integración, binario funcional | software-architect | --exclude-tools read,edit,grep,find | raíz del proyecto | G1, G2, C1, C2, B1, GV1, GV2, F1 |

## Tool Briefings Inyectados por Agente

| Agente | Tool Briefing |
|--------|--------------|
| backend-architect (G1,G2,GV2,C2) | `[TOOLS] pi-memory para guardar decisiones técnicas clave. pi-github-tools disponible para CI. cc-safety-net protege comandos destructivos.` |
| data-engineer (C1,B1,GV1) | `[TOOLS] pi-memory para esquemas y decisiones de datos. pi-docparser disponible si hay docs con specs.` |
| frontend-developer (F1) | `[TOOLS] pi-zentui activo para preview visual. pi-memory para preferencias guardadas.` |
| software-architect (EA) | `[TOOLS] pi-memory para ADRs. pi-goal para sesiones autónomas de ensamblaje. pi-github-tools para CI.` |

## Research Queries por Tarea (Fase 2.5)

| ID | Queries |
|----|---------|
| G1 | "axum 0.8 routing best practices 2026", "axum middleware auth example", "axum OpenAI compatible API proxy" |
| G2 | "reqwest rust SSE streaming proxy", "OpenRouter API format 2026", "Anthropic API OpenAI compatibility" |
| C1 | "ort crate ONNX runtime rust example", "ONNX int8 embedding model rust", "rust ONNX inference embedding generation" |
| C2 | "rust ML model selection algorithm", "weighted scoring system rust" |
| B1 | "LLM benchmark testing framework", "automated LLM evaluation scoring" |
| GV1 | "rusqlite schema design rust", "SQLite rust async example" |
| GV2 | "token counting algorithm rust", "rate limiting token budget rust" |
| F1 | "Tauri v2 setup 2026", "Tauri v2 with Tailwind CSS", "Tauri command system rust" |

## Orden de Ejecución

```
Fase A (8 paralelo):
  G1 + G2 + C1 + C2 + B1 + GV1 + GV2 + F1
        ↓
Fase B (1 ensamblaje):
  EA (depende de todos los outputs de Fase A)
```

## Criterio de Éxito

- `cargo build --release` compila sin errores
- `cargo test` pasa todas las pruebas
- El gateway responde en `:18801/v1/chat/completions`
- El embedding classifier genera vectores correctamente
- La UI de Tauri muestra el ecualizador funcional
- Los budgets de governance se aplican correctamente
