# 🧠 Cortex Gate

> **Intelligent AI Gateway** — Autonomous Benchmarking · Embedding Routing · Cost Governance

Cortex Gate es un AI Gateway de próxima generación que resuelve el **Trilema
Corporativo de la IA**: costos incontrolables, fuga de datos confidenciales y
vendor lock-in.

A diferencia de los routers tradicionales (que usan heurísticas fijas o un LLM
como juez), Cortex Gate **benchmarkea modelos reales**, construye perfiles de
capacidad por dimensión, y usa **embeddings ONNX int8** para clasificar prompts
y enrutar al modelo óptimo en tiempo real.

Todo controlable desde un **ecualizador visual** con perillas ajustables por
dimensión y una perilla de economía que pondera todo por costo.

---

## ✨ Features

| Feature | Descripción |
|---------|-------------|
| **Benchmark Engine** | Ejecuta tests reales contra cada LLM para conocer sus capacidades |
| **Embedding Classifier** | Clasifica prompts usando ONNX int8 (<5ms, sin llamada LLM) |
| **Ecualizador** | Ajusta peso de cada dimensión (razonamiento, código, creatividad, etc.) |
| **Perilla de Economía** | Pondera todo por precio — de barato a calidad máxima |
| **Cost Governance** | Topes de tokens por hora/día/semana/mes, multi-usuario |
| **Multi-API** | OpenRouter, Anthropic, OpenAI, proveedores locales, simultáneamente |
| **Dashboard Tauri** | UI de escritorio nativa con Tailwind CSS |
| **OpenAI-Compatible** | Drop-in replacement para cualquier cliente `/v1/chat/completions` |

---

## 🏗️ Arquitectura

```
┌──────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Client     │───▶│  Gateway (Rust)  │───▶│  OpenRouter /   │
│  (any tool)  │    │  :18801/v1       │    │  Anthropic /    │
└──────────────┘    │                  │    │  OpenAI / ...   │
                    │  ┌────────────┐  │    └─────────────────┘
                    │  │ Classifier │  │
                    │  │ (ONNX int8)│  │
                    │  └────────────┘  │
                    │  ┌────────────┐  │
                    │  │ Benchmark  │  │
                    │  │ Engine     │  │
                    │  └────────────┘  │
                    │  ┌────────────┐  │
                    │  │ Governance │  │
                    │  │ (SQLite)   │  │
                    │  └────────────┘  │
                    └──────────────────┘
                           │
                    ┌──────▼──────┐
                    │   Tauri UI  │
                    │ (Ecualizador│
                    │  + Dashboard)│
                    └─────────────┘
```

---

## 🚀 Quick Start

```bash
# 1. Build
cargo build --release

# 2. Run the gateway (headless mode)
cargo run --release

# 3. Or run with desktop UI
cargo run --release --features desktop
```

---

## 📖 Documentación

- [Origen del proyecto perdido](docs/origen-proyecto-perdido.md) — Memoria del proyecto original
- [Arquitectura](docs/architecture/) — Documentación técnica detallada

---

## ⚖️ Licencia

PolyForm Noncommercial License 1.0.0. Ver [LICENSE](LICENSE).

Para uso comercial, contactar al autor: **Luis Daniel Dos Santos**
