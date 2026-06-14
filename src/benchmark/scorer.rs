// Cortex Gate — Benchmark Scorer System.
//
// Define los tipos de scorers, el resultado de un scoring, y la función
// principal `score_response` que evalúa la respuesta de un LLM contra
// el criterio esperado de un BenchmarkTest.
//
// ## Scorers disponibles
// - ExactMatch:  comparación exacta (case-insensitive)
// - Contains:    subcadena contenida en la respuesta
// - Regex:       patrón regex contra la respuesta
// - Semantic:    coincidencia de palabras clave semánticas
// - Length:      proximidad de longitud al objetivo esperado

use serde::{Deserialize, Serialize};

use crate::benchmark::tests::BenchmarkTest;
use crate::classifier::dimensions::DimensionType;

// ---------------------------------------------------------------------------
// ScorerType
// ---------------------------------------------------------------------------

/// Estrategia de evaluación para un benchmark test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScorerType {
    /// La respuesta debe coincidir exactamente con expected_output (ignorando mayúsculas y espacios extremos).
    ExactMatch,
    /// La respuesta debe contener expected_output como subcadena.
    Contains,
    /// La respuesta debe coincidir con un patrón regex (expected_output es el patrón).
    Regex,
    /// Evaluación por presencia de palabras clave semánticas (expected_output contiene keywords separadas por coma).
    Semantic,
    /// Puntúa por proximidad de longitud de la respuesta al valor numérico en expected_output.
    Length,
}

impl ScorerType {
    /// Retorna una descripción legible del scorer.
    pub fn description(&self) -> &'static str {
        match self {
            ScorerType::ExactMatch => "Exact match (case-insensitive)",
            ScorerType::Contains => "Substring containment",
            ScorerType::Regex => "Regex pattern match",
            ScorerType::Semantic => "Keyword/semantic presence",
            ScorerType::Length => "Response length proximity",
        }
    }
}

// ---------------------------------------------------------------------------
// ScorerResult
// ---------------------------------------------------------------------------

/// Resultado completo de evaluar una respuesta contra un test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScorerResult {
    /// Puntuación normalizada 0.0 – 1.0.
    pub score: f32,
    /// Explicación legible de cómo se obtuvo el score.
    pub details: String,
    /// Latencia de la llamada al modelo en milisegundos.
    pub latency_ms: Option<u64>,
    /// Dimensión a la que pertenece el test.
    pub dimension: DimensionType,
    /// ID del test que generó este resultado.
    pub test_id: String,
    /// Fragmento de la respuesta del modelo (primeros 200 caracteres).
    pub response_snippet: String,
}

// ---------------------------------------------------------------------------
// score_response
// ---------------------------------------------------------------------------

/// Evalúa la respuesta de un LLM contra el criterio definido en el test.
///
/// # Argumentos
/// - `test`: el benchmark test con expected_output y scorer_type
/// - `response`: la respuesta textual del modelo
///
/// # Retorna
/// Un `ScorerResult` con score normalizado (0.0 – 1.0) y detalles.
pub fn score_response(test: &BenchmarkTest, response: &str) -> ScorerResult {
    let trimmed = response.trim();
    let snippet = if trimmed.len() > 200 {
        format!("{}...", &trimmed[..200])
    } else {
        trimmed.to_string()
    };

    let (score, details) = match test.scorer_type {
        ScorerType::ExactMatch => score_exact_match(&test.expected_output, trimmed),
        ScorerType::Contains => score_contains(&test.expected_output, trimmed),
        ScorerType::Regex => score_regex(&test.expected_output, trimmed),
        ScorerType::Semantic => score_semantic(&test.expected_output, trimmed),
        ScorerType::Length => score_length(&test.expected_output, trimmed),
    };

    ScorerResult {
        score,
        details,
        latency_ms: None,
        dimension: test.dimension,
        test_id: test.id.clone(),
        response_snippet: snippet,
    }
}

// ---------------------------------------------------------------------------
// Implementaciones individuales de scorers
// ---------------------------------------------------------------------------

/// ExactMatch: comparación case-insensitive, ignorando espacios extremos.
fn score_exact_match(expected: &str, response: &str) -> (f32, String) {
    let normalized_expected = expected.trim().to_lowercase();
    let normalized_response = response.trim().to_lowercase();

    if normalized_response == normalized_expected {
        (
            1.0,
            format!(
                "Exact match: respuesta coincide exactamente con '{}'",
                expected.trim()
            ),
        )
    } else {
        (
            0.0,
            format!(
                "No coincide. Esperado: '{}'. Obtenido: '{}'",
                expected.trim(),
                response.trim()
            ),
        )
    }
}

/// Contains: verifica si la respuesta contiene la subcadena esperada.
fn score_contains(expected: &str, response: &str) -> (f32, String) {
    let response_lower = response.to_lowercase();
    let expected_lower = expected.trim().to_lowercase();

    if response_lower.contains(&expected_lower) {
        (
            1.0,
            format!("Contains match: respuesta contiene '{}'", expected.trim()),
        )
    } else {
        (
            0.0,
            format!(
                "No contiene '{}'. Respuesta: '{}'",
                expected.trim(),
                response.chars().take(100).collect::<String>()
            ),
        )
    }
}

/// Regex: compila el patrón y verifica si la respuesta hace match.
fn score_regex(pattern: &str, response: &str) -> (f32, String) {
    match regex::Regex::new(pattern.trim()) {
        Ok(re) => {
            if re.is_match(response) {
                (
                    1.0,
                    format!("Regex match: patrón '{}' encontrado", pattern.trim()),
                )
            } else {
                (
                    0.0,
                    format!(
                        "Regex no match: patrón '{}' no encontrado en respuesta",
                        pattern.trim()
                    ),
                )
            }
        }
        Err(e) => (0.0, format!("Error de regex '{}': {}", pattern.trim(), e)),
    }
}

/// Semantic: evalúa presencia de palabras clave separadas por coma.
///
/// Score = (keywords_encontradas / keywords_totales).
/// Útil para tests de creatividad (verificar temas) y safety (verificar rechazo).
fn score_semantic(expected: &str, response: &str) -> (f32, String) {
    let response_lower = response.to_lowercase();
    let keywords: Vec<&str> = expected
        .split(',')
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .collect();

    if keywords.is_empty() {
        return (
            0.0,
            "No hay keywords definidas en expected_output".to_string(),
        );
    }

    let found: Vec<&str> = keywords
        .iter()
        .filter(|k| response_lower.contains(&k.to_lowercase()))
        .copied()
        .collect();

    let score = found.len() as f32 / keywords.len() as f32;

    let details = if score >= 1.0 {
        format!("Todas las keywords encontradas: {:?}", found)
    } else if found.is_empty() {
        format!("Ninguna keyword encontrada. Buscadas: {:?}", keywords)
    } else {
        format!(
            "Keywords encontradas ({}/{}): {:?}. Faltantes: {:?}",
            found.len(),
            keywords.len(),
            found,
            keywords
                .iter()
                .filter(|k| !found.contains(k))
                .collect::<Vec<_>>()
        )
    };

    (score, details)
}

/// Length: score por proximidad de longitud (en palabras) al objetivo.
///
/// Score = max(0, 1 - |len_esperada - len_respuesta| / len_esperada)
fn score_length(expected: &str, response: &str) -> (f32, String) {
    let target_words: usize = match expected.trim().parse() {
        Ok(n) if n > 0 => n,
        Ok(_) => return (0.0, "Target length debe ser > 0".to_string()),
        Err(_) => {
            return (
                0.0,
                format!(
                    "Expected_output '{}' no es un número válido para Length scorer",
                    expected.trim()
                ),
            )
        }
    };

    let response_words: usize = response.split_whitespace().count();
    let diff = if response_words > target_words {
        response_words - target_words
    } else {
        target_words - response_words
    };

    let score = (1.0 - diff as f32 / target_words as f32).max(0.0);

    let details = format!(
        "Longitud: esperada ~{} palabras, obtenida {} palabras (diff={}). Score: {:.2}",
        target_words, response_words, diff, score
    );

    (score, details)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::dimensions::DimensionType;

    fn make_test(scorer_type: ScorerType, expected: &str) -> BenchmarkTest {
        BenchmarkTest {
            id: "test-scorer".into(),
            dimension: DimensionType::Precision,
            prompt: "ignored".into(),
            expected_output: expected.into(),
            scorer_type,
        }
    }

    #[test]
    fn test_exact_match_pass() {
        let _t = make_test(ScorerType::ExactMatch, "Paris");
        let r = score_exact_match("Paris", "Paris");
        assert!((r.0 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_exact_match_case_insensitive() {
        let _t = make_test(ScorerType::ExactMatch, "Paris");
        let r = score_exact_match("Paris", "paris");
        assert!((r.0 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_exact_match_fail() {
        let _t = make_test(ScorerType::ExactMatch, "Paris");
        let r = score_exact_match("Paris", "London");
        assert!((r.0 - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_contains_pass() {
        let _t = make_test(ScorerType::Contains, "fibonacci");
        let r = score_contains("fibonacci", "def fibonacci(n):");
        assert!((r.0 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_contains_fail() {
        let _t = make_test(ScorerType::Contains, "fibonacci");
        let r = score_contains("fibonacci", "def factorial(n):");
        assert!((r.0 - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_regex_pass() {
        let r = score_regex(r"\d{3}-\d{4}", "Tel: 555-1234");
        assert!((r.0 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_regex_fail() {
        let r = score_regex(r"\d{3}-\d{4}", "Tel: 5551234");
        assert!((r.0 - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_regex_invalid() {
        let r = score_regex(r"[invalid", "anything");
        assert!((r.0 - 0.0).abs() < 0.001);
        assert!(r.1.contains("Error de regex"));
    }

    #[test]
    fn test_semantic_all_found() {
        let r = score_semantic("hello,world,test", "hello world this is a test");
        assert!((r.0 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_semantic_partial() {
        let r = score_semantic("hello,world,missing", "hello world");
        assert!((r.0 - 2.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_semantic_none() {
        let r = score_semantic("hello,world", "goodbye");
        assert!((r.0 - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_length_exact() {
        let r = score_length("50", "word ".repeat(50).trim());
        assert!((r.0 - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_length_half() {
        let r = score_length("50", "word ".repeat(25).trim());
        // diff = 25, target = 50 → score = 1 - 25/50 = 0.5
        assert!((r.0 - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_length_invalid_target() {
        let r = score_length("not-a-number", "hello");
        assert!((r.0 - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_integration_score_response() {
        let test = BenchmarkTest {
            id: "integ-test".into(),
            dimension: DimensionType::Code,
            prompt: "Write fibonacci".into(),
            expected_output: "fibonacci".into(),
            scorer_type: ScorerType::Contains,
        };
        let result = score_response(
            &test,
            "def fibonacci(n):\n    return n if n <= 1 else fibonacci(n-1) + fibonacci(n-2)",
        );
        assert!((result.score - 1.0).abs() < 0.001);
        assert_eq!(result.dimension, DimensionType::Code);
        assert_eq!(result.test_id, "integ-test");
    }
}
