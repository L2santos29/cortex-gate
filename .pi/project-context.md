# Project Context: Cortex Gate

## Overview
Intelligent AI Gateway with autonomous LLM benchmarking, ONNX embedding-based
semantic routing, visual ecualizador with adjustable dimension weights,
economy dial, and enterprise cost governance (multi-user, multi-API, token budgets).

Resuelve el Trilema Corporativo de la IA: costos incontrolables, fuga de datos
confidenciales y vendor lock-in.

## Tech Stack
- **Language:** Rust (backend + ONNX inference), Tauri (desktop UI)
- **Database:** SQLite (via rusqlite)
- **ML:** ONNX Runtime for Rust (ort crate) — int8 quantized embedding models
- **Frontend:** Tauri v2 with Tailwind CSS (ecualizador + dashboard)
- **HTTP:** Axum (gateway server, OpenAI-compatible API)
- **License:** PolyForm Noncommercial 1.0.0

## Architecture
- **Type:** Backend API + Desktop App (Gateway + ML Engine + UI)
- **License:** PolyForm Noncommercial
- **Nature:** Professional
- **Audience:** Commercial / Enterprise

## Components
| Component | Description | Agent |
|-----------|-------------|-------|
| benchmark-engine | Autonomous LLM testing and capability profiling | backend-architect |
| classifier | ONNX int8 embedding classifier + routing equation | data-engineer |
| gateway | OpenAI-compatible API server with multi-provider proxy | backend-architect |
| governance | Cost tracking, user quotas, token budgets, alerts | data-engineer |
| tauri-ui | Desktop app with ecualizador, dashboard, and config panels | frontend-developer |

## Project Structure
```
src/
  gateway/     → HTTP server, routing engine, API endpoints
  benchmark/   → Benchmark engine, LLM profiles, test library
  classifier/  → ONNX embedding, dimensions, routing equation
  governance/  → Cost governance, user management, quotas
  models/      → Shared data types and configurations
  tools/       → Utilities (providers, errors, logging)
tests/          → Integration and unit tests
docs/           → Architecture documentation
frontend/       → Tauri web frontend (Tailwind CSS)
.pi/            → Plans and agent context
.herdv-output/  → Atomic task outputs
```

## Agent Assignments
| Agent Role | Component | Tools Flag |
|-----------|-----------|------------|
| backend-architect | benchmark-engine | --exclude-tools read,edit,grep,find |
| data-engineer | classifier | --exclude-tools read,edit,grep,find |
| backend-architect | gateway | --exclude-tools read,edit,grep,find |
| data-engineer | governance | --exclude-tools read,edit,grep,find |
| frontend-developer | tauri-ui | --exclude-tools read,edit,grep,find |

## Key Design Decisions
- Rust for everything (including ONNX inference via `ort` crate) — no Python dependency
- ONNX int8 model for embeddings — <5ms inference, zero API cost
- SQLite for governance data — portable, no server, easy backups
- Tauri for desktop UI — native performance, web-based frontend
- Gateway on :18801 — OpenAI-compatible, drop-in replacement

## Next Steps
Run `/start-workflow <first-task>` to plan and execute using multi-agent orchestration.
