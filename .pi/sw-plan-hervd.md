# Plan: Cortex Gate

> Generado por new-project. Los componentes serán expandidos por start-workflow.

## Tareas
| ID | Descripción | Personalidad | Tools | Contexto | Dependencias |
|----|-------------|-------------|-------|----------|-------------|
| G1 | Gateway core: axum server, routes, auth, OpenAI-compatible API | backend-architect | --exclude-tools read,edit,grep,find | gateway | - |
| C1 | Embedding classifier: ONNX model loading + inference + routing equation | data-engineer | --exclude-tools read,edit,grep,find | classifier | - |
| B1 | Benchmark engine: test library, LLM profiling, scorer | backend-architect | --exclude-tools read,edit,grep,find | benchmark | - |
| GV1 | Cost governance: SQLite schema, quotas, user management, alerts | data-engineer | --exclude-tools read,edit,grep,find | governance | - |
| F1 | Tauri desktop UI: ecualizador, dashboard, config panels | frontend-developer | --exclude-tools read,edit,grep,find | frontend | G1, C1 |
| EA | Integrar componentes y ensamblar binario | software-architect | --exclude-tools read,edit,grep,find | Integración de G1, C1, B1, GV1, F1 | G1, C1, B1, GV1, F1 |

## Orden
1. G1, C1, B1, GV1 (paralelo — independientes)
2. F1 (depende de G1 + C1 para API)
3. EA (ensamblaje final)
