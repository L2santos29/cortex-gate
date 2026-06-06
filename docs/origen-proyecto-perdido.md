# Cortex Gate — Memoria del Proyecto Perdido

> **Documento fundacional.** Reconstrucción de la arquitectura, visión y
> componentes del proyecto original que precedió a Cortex Gate.
> El proyecto original se perdió por accidente antes de ser commitado.
> Este documento preserva todo lo que recordamos.

---

## 📜 Origen: El Manifiesto del Enrutamiento Dinámico

El proyecto nació del **[Manifiesto del Enrutamiento Dinámico](../../agency%20documents/research/Manifiesto%20Estrat%C3%A9gico_%20El%20AI%20Gateway%20Definitivo.pdf)**,
un documento estratégico de 5 páginas que analiza el **Trilema Corporativo de la IA**:

1. **Crisis Económica** — La trampa del token. Usar modelos masivos para tareas
   mecánicas quema presupuesto. Procesar 10,000 documentos puede costar cientos
   de dólares en APIs premium en lugar de centavos con SLMs.
2. **Crisis de Cumplimiento** — Shadow AI. Envío irrestricto de datos a nubes
   públicas anula privilegio legal y confidencialidad (Ej: *United States v. Heppner 2026*).
3. **Crisis Arquitectónica** — Vendor Lock-In. Rigidez estática, latencia TTFT,
   manejo irresponsable de Golden Keys, incompatibilidad de Tool Calling.

---

## 🧠 El Proyecto Perdido

### Nombre
**Cortex Router** (tentativo) — El nombre exacto se perdió con el código.
"Cortex" resonaba como candidato.

### Stack original (recordado)
- **Lenguaje:** Python (sentence-transformers, PyTorch)
- **Modelo de embeddings:** IBM Granite Embedding English R2 (~288 MB)
- **Modelo alternativo (implementado):** ONNX int8, 2× más rápido, mitad de tamaño
  (probablemente `bge-small-en-v1.5` o `all-MiniLM-L6-v2` en ONNX int8)
- **Inferencia:** ONNX Runtime
- **Clasificador:** KNN con scikit-learn (cosine similarity sobre embeddings)
- **UI:** Web con Tailwind CSS (similar al Admin UI de Prompt Switch)
- **Estado:** Funcional — llegó a ejecutarse y funcionaba correctamente
- **Motivo de pérdida:** Accidente — se borró el directorio del proyecto
  (`rm -rf`), sin backup ni commits. No pasó por la papelera.

### Evidencia física que sobrevive
- Modelos IBM Granite en HuggingFace cache:
  - `granite-embedding-english-r2` (288 MB)
  - `granite-embedding-small-english-r2` (95 MB)
- `sentence-transformers` v5.5.1 instalado (con torch 2.12, transformers 5.9)
- `scikit-learn` v1.8.0 instalado

### Predecesores conceptuales
| Proyecto | Aportó |
|----------|--------|
| **Prompt Switch** (Rust) | Two-Pass Semantic Routing, Admin UI Tailwind, streaming SSE, auth dual-token |
| **FreeRouter** (TypeScript) | 14-dimension weighted scoring, tier system (SIMPLE/MEDIUM/COMPLEX/REASONING), sigmoid confidence calibration |
| **ClawRouter** (original) | KNN fallback design doc, LLM classifier con Gemini Flash |

---

## 🏗️ Arquitectura Reconstruida

```
┌──────────────────────────────────────────────────────────────────┐
│                        CORTEX GATE                                │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │  FASE 1: BENCHMARK ENGINE                                    │ │
│  │                                                              │ │
│  │  Ejecuta tests REALES contra cada LLM configurado.           │ │
│  │  No son benchmarks de internet — la app los ejecuta          │ │
│  │  internamente contra cada modelo.                            │ │
│  │                                                              │ │
│  │  Cada LLM recibe un PERFIL DE CAPACIDAD en N dimensiones:    │ │
│  │  LLM A → { razonamiento: 0.92, código: 0.88, creatividad: 0.45, ... } │
│  │  LLM B → { razonamiento: 0.55, código: 0.90, creatividad: 0.82, ... } │
│  └──────────────────────────┬───────────────────────────────────┘ │
│                             │                                     │
│  ┌──────────────────────────▼───────────────────────────────────┐ │
│  │  FASE 2: EMBEDDING CLASSIFIER                                │ │
│  │                                                              │ │
│  │  Prompt entrante → embedding (ONNX int8, <5ms)               │ │
│  │       ↓                                                      │ │
│  │  Vector de 384-768 dimensiones                               │ │
│  │       ↓                                                      │ │
│  │  Cosine similarity con centroides de cada dimensión          │ │
│  │       ↓                                                      │ │
│  │  Puntaje de similitud por dimensión                          │ │
│  └──────────────────────────┬───────────────────────────────────┘ │
│                             │                                     │
│  ┌──────────────────────────▼───────────────────────────────────┐ │
│  │  FASE 3: ROUTING ECUATION                                    │ │
│  │                                                              │ │
│  │  Score = Σ(dimensión_i × peso_i) × economía                  │ │
│  │                                                              │ │
│  │  ╔═══════════════════════════════════════════════╗           │ │
│  │  ║         E C U A L I Z A D O R                ║           │ │
│  │  ║                                               ║           │ │
│  │  ║  [Razonamiento]    ──●───────────────        ║           │ │
│  │  ║  [Código]          ────────●───────────      ║           │ │
│  │  ║  [Creatividad]     ──────────────●─────      ║           │ │
│  │  ║  [Matemáticas]     ───●─────────────────      ║           │ │
│  │  ║  [Precisión]       ────────●───────────      ║           │ │
│  │  ║  [Velocidad]       ──●─────────────────      ║           │ │
│  │  ║  ...                                          ║           │ │
│  │  ║                                               ║           │ │
│  │  ║  💰 Economía:     ◉─────○─────○              ║           │ │
│  │  ║                   bajo  medio  alto           ║           │ │
│  │  ╚═══════════════════════════════════════════════╝           │ │
│  └──────────────────────────┬───────────────────────────────────┘ │
│                             │                                     │
│  ┌──────────────────────────▼───────────────────────────────────┐ │
│  │  FASE 4: COST GOVERNANCE                                     │ │
│  │                                                              │ │
│  │  • Topes de tokens: por hora / día / semana / mes            │ │
│  │  • Multi-usuario: cada developer con su propia cuota          │ │
│  │  • Multi-API: OpenRouter, Anthropic, OpenAI, proveedores locales │ │
│  │  • Alertas automáticas y cortes por superación de límites    │ │
│  │  • Logs de uso por usuario, modelo, proyecto                  │ │
│  └──────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

---

## 🎛️ Las Dimensiones del Ecualizador

El sistema clasifica prompts y perfila LLMs usando dimensiones de capacidad.
Cada dimensión tiene:

- **Peso** (0.0 - 1.0) — qué tanto influye en la decisión final
- **Intensidad** (perilla del ecualizador) — el usuario puede subir/bajar
- **Puntaje del LLM** — resultado de los tests benchmark en esa dimensión
- **Similitud del prompt** — qué tanto se parece el prompt a esa dimensión

| Dimensión | Descripción | Influencia típica |
|-----------|-------------|-------------------|
| Razonamiento | Capacidad de encadenamiento lógico, proofs, step-by-step | Alta para tareas complejas |
| Código | Calidad de generación de código, debugging, refactors | Alta para developers |
| Creatividad | Originalidad, tono, narrativa, brainstorming | Alta para marketing/contenido |
| Matemáticas | Precisión numérica, álgebra, estadística | Alta para análisis |
| Precisión | Exactitud factual, seguimiento de instrucciones | Alta para tareas legales |
| Velocidad | Latencia de respuesta, TTFT, tokens/s | Alta para chat en tiempo real |
| Contexto | Capacidad de manejar ventanas largas (>32K) | Alta para análisis de documentos |
| Seguridad | Resistencia a jailbreaks, outputs seguros | Alta para producción |

La **perilla de Economía** pondera todo por precio del modelo:
- **Bajo:** prioriza modelos baratos (Gemini Flash, Haiku, DeepSeek)
- **Medio:** balance costo-calidad (default)
- **Alto:** prioriza calidad sin importar costo (Opus, GPT-5, Sonnet)

---

## 📋 Tests de Benchmark

El Benchmark Engine ejecuta tests reales contra cada LLM para construir
su perfil de capacidades. Los tests son:

- **Propios del sistema** — no descargados de internet
- **Ejecutados periódicamente** — para actualizar perfiles
- **Puntuados automáticamente** — comparación con respuestas conocidas

Cada test consiste en:
1. Un prompt específico para una dimensión
2. Una respuesta esperada o criterio de evaluación
3. Un scorer que evalúa la respuesta del LLM

---

## 💡 Innovación Clave

Lo que hacía único a este sistema —y lo que Cortex Gate debe preservar—:

1. **No usa heurísticas fijas** (diferencia de FreeRouter)
2. **No usa un LLM como judge** (diferencia de Prompt Switch)
3. **Usa evidencia real** (benchmarks ejecutados localmente)
4. **Es transparente y ajustable** (el ecualizador, no caja negra)
5. **Es autónomo** — se calibra solo corriendo tests
6. **Es consciente de costos** — la economía es una dimensión nativa

---

## 🔄 Relación con los Proyectos Existentes

```
Manifiesto Estratégico (documento fundacional)
        │
        ├──→ Prompt Switch (Rust, Two-Pass, Judge LLM)
        │       │
        │       ├── → RoutingMetadata headers
        │       ├── → Fallback client API key
        │       ├── → Model ~ aliases
        │       └── → Admin UI Tailwind
        │
        ├──→ FreeRouter (TypeScript, 14-dim heurístico)
        │       │
        │       ├── → Weighted scoring engine
        │       ├── → Tier system (SIMPLE/MEDIUM/COMPLEX/REASONING)
        │       ├── → Sigmoid confidence calibration
        │       └── → Mode overrides (/max, [simple], deep mode:)
        │
        └──→ ❌ Proyecto Perdido (Cortex Router)
                │
                ├── → Benchmark engine + perfiles reales
                ├── → Embedding ONNX int8 classifier
                ├── → Ecualizador con perillas
                ├── → Perilla de economía
                └── → Cost governance (borrado con el proyecto)
                         │
                         ▼
                  CORTEX GATE (reconstrucción)
```

---

## 🎯 Visión de Cortex Gate

Cortex Gate es la evolución final de todo este linaje:

| Dimensión | Prompt Switch | FreeRouter | Cortex Gate |
|-----------|:---:|:---:|:---:|
| Rendimiento | ⚡ Rust | 🟡 Node | ⚡ Rust + Tauri |
| Clasificación | 🧠 LLM Judge | 📊 Heurísticas | 🧬 Embeddings ONNX |
| Precisión | Muy alta | Media | Alta + Mejorable |
| Latencia extra | ~2s (2 LLM calls) | <1ms | <5ms |
| Costo extra | Alto | Cero | Casi cero |
| Benchmark real | ❌ No | ❌ No | ✅ Sí |
| Ecualizador UX | ❌ No | ❌ No | ✅ Tauri Desktop |
| Economía ajustable | ❌ No | ❌ No | ✅ Perilla |
| Cost governance | ❌ No | ❌ No | ✅ Multi-usuario |
| Multi-API | ❌ Solo OpenRouter | ✅ Multi-provider | ✅ Multi-provider |

---

*Documento generado: 2026-06-05*
*Base para la reconstrucción de Cortex Gate*
