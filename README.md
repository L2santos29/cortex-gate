# 🧠 Cortex Gate

> **Intelligent AI Gateway** — Autonomous Benchmarking · Embedding Routing · Cost Governance

Cortex Gate is a next-generation AI Gateway that solves the **Corporate AI
Trilemma**: uncontrollable costs, confidential data leakage, and vendor lock-in.

Unlike traditional routers (which use fixed heuristics or an LLM-as-judge),
Cortex Gate **benchmarks real models**, builds capability profiles per dimension,
and uses **ONNX int8 embeddings** to classify prompts and route to the optimal
model in real time.

Everything is controllable from a **visual equalizer** with adjustable knobs per
dimension and an economy knob that weighs everything by cost.

---

## ✨ Features

| Feature | Description |
|---------|-------------|
| **Benchmark Engine** | Runs real tests against each LLM to map its capabilities |
| **Embedding Classifier** | Classifies prompts using ONNX int8 (<5ms, no LLM call) |
| **Equalizer** | Adjusts weight per dimension (reasoning, code, creativity, etc.) |
| **Economy Knob** | Weighs everything by price — from budget to maximum quality |
| **Cost Governance** | Token caps per hour/day/week/month, multi-user |
| **Multi-API** | OpenRouter, Anthropic, OpenAI, local providers, simultaneously |
| **Tauri Dashboard** | Native desktop UI with Tailwind CSS |
| **OpenAI-Compatible** | Drop-in replacement for any `/v1/chat/completions` client |

---

## 🏗️ Architecture

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
                    │  (Equalizer │
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

# 3. Or run with desktop Tauri UI (launches backend + frontend)
# Double-click the app icon, or:
cd frontend/src-tauri && cargo build --release && DISPLAY=:0 ./target/release/cortex-gate-tauri
```

### From the Tauri Desktop App

1. Double-click the **Cortex Gate** icon
2. In the sidebar, click **Start Server**
3. Status dot turns green → **Server Online**
4. Click **Open Web UI** to open `http://127.0.0.1:18801` in your browser
5. Click **Stop Server** to shut down

### Direct Web Access

Once the backend is running, open `http://127.0.0.1:18801` in any browser — it serves both the API and the frontend UI.

---

## 📖 Documentation

- [Project Origins](docs/origen-proyecto-perdido.md) — Historical project memory
- [Architecture](docs/architecture/) — Detailed technical documentation (WIP)

---

## ⚖️ License

PolyForm Noncommercial License 1.0.0. See [LICENSE](LICENSE).

For commercial use, contact the author: **Luis Daniel Dos Santos**
