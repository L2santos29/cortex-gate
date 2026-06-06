// Embedding Classifier — ONNX int8 semantic prompt classification.
//
// Toma un prompt entrante, lo convierte en embedding vectorial usando
// un modelo ONNX int8 (como bge-small-en-v1.5), y calcula similitud
// con los centroides de cada dimensión.
//
// ## Flujo
// 1. Cargar modelo ONNX int8 al iniciar (una vez)
// 2. Embedding del prompt → vector de 384-768 dimensiones
// 3. Cosine similarity con cada centroide de dimensión
// 4. Combinar con pesos del ecualizador y factor económico
// 5. Devolver (modelo_objetivo, nivel_razonamiento, confianza)
//
// ## Ventajas sobre Prompt Switch
// - Sin llamada LLM (Judge): 0 costo extra
// - Sin reglas heurísticas: basado en semántica real
// - Tiempo: <5ms vs ~2s del Two-Pass
// - Precisa de perfiles de benchmark reales

pub mod embedding;
pub mod dimensions;
pub mod equation;
pub mod knn;
