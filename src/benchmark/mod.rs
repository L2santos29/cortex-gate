// Benchmark Engine — Autonomous LLM capability profiling.
//
// Ejecuta tests REALES contra cada modelo configurado para determinar
// sus capacidades en cada dimensión. No son benchmarks de internet —
// la app ejecuta los tests internamente.
//
// ## Dimensiones benchmarkeadas
// - Razonamiento (encadenamiento lógico, proofs, step-by-step)
// - Código (generación, debugging, refactors)
// - Creatividad (originalidad, tono, narrativa)
// - Matemáticas (precisión numérica, álgebra, estadística)
// - Precisión (exactitud factual, seguimiento de instrucciones)
// - Velocidad (latencia, TTFT, tokens/s)
// - Contexto (ventana larga, >32K tokens)
// - Seguridad (resistencia a jailbreaks)
//
// ## Flujo
// 1. El engine toma un test de la biblioteca interna
// 2. Envía el prompt al LLM
// 3. Evalúa la respuesta contra el criterio del test
// 4. Asigna un puntaje (0.0 - 1.0) en la dimensión correspondiente
// 5. Almacena el resultado en el perfil del modelo

pub mod engine;
pub mod profiles;
pub mod tests;
pub mod scorer;
