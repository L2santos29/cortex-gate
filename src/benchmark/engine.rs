// Cortex Gate — Benchmark Engine.
//
// Ejecuta la suite completa de benchmarks contra un modelo LLM,
// evalúa las respuestas usando el sistema de scorers, y genera
// un perfil de capacidades (ModelProfile).
//
// ## Flujo
// 1. El engine toma un modelo y un proveedor configurado
// 2. Para cada test en la suite, envía el prompt al LLM vía HTTP
// 3. Evalúa la respuesta contra el criterio del test
// 4. Acumula los resultados en un ModelProfile
// 5. Retorna el perfil completo con scores por dimensión

use std::collections::HashMap;
use std::time::Instant;

use reqwest::Client;
use serde_json::Value;

use crate::benchmark::profiles::{ModelProfile, ScorerResultSummary};
use crate::benchmark::scorer::{self, ScorerResult};
use crate::benchmark::tests::BenchmarkTest;
use crate::classifier::dimensions::DimensionType;
use crate::models::config::ProviderEntry;

// ---------------------------------------------------------------------------
// BenchmarkEngine
// ---------------------------------------------------------------------------

/// Motor de benchmarking que ejecuta tests contra modelos LLM.
///
/// # Ejemplo
/// ```ignore
/// let client = reqwest::Client::new();
/// let engine = BenchmarkEngine::new(client);
///
/// let provider = ProviderEntry {
///     name: "openai".into(),
///     base_url: "https://api.openai.com/v1".into(),
///     api_key: Some("sk-...".into()),
///     provider_type: "openai".into(),
///     models: vec!["gpt-4o".into()],
/// };
///
/// match engine.run_benchmark("gpt-4o", &provider).await {
///     Ok(profile) => println!("Score: {:.2}", profile.average_score()),
///     Err(e) => eprintln!("Benchmark failed: {}", e),
/// }
/// ```
pub struct BenchmarkEngine {
    /// Tests a ejecutar en cada benchmark.
    pub tests: Vec<BenchmarkTest>,
    /// Cliente HTTP reutilizable para llamadas a APIs.
    http_client: Client,
}

impl BenchmarkEngine {
    /// Crea un nuevo engine con el catálogo default de tests.
    ///
    /// `http_client`: Cliente reqwest configurado (debe tener timeout
    /// adecuado para llamadas LLM, ej: 30s por request).
    pub fn new(http_client: Client) -> Self {
        Self {
            tests: super::tests::default_tests(),
            http_client,
        }
    }

    /// Crea un engine con una lista personalizada de tests.
    pub fn with_tests(http_client: Client, tests: Vec<BenchmarkTest>) -> Self {
        Self { tests, http_client }
    }

    // ------------------------------------------------------------------
    // run_benchmark
    // ------------------------------------------------------------------

    /// Ejecuta la suite completa de benchmarks contra un modelo.
    ///
    /// # Argumentos
    /// - `model_name`: nombre del modelo (ej: "gpt-4o", "claude-3.5-sonnet")
    /// - `provider`: configuración del proveedor (base_url, api_key, tipo)
    ///
    /// # Retorna
    /// Un `ModelProfile` con scores poblados para todas las dimensiones.
    ///
    /// # Errores
    /// Retorna `Err` si alguna llamada HTTP falla o si la respuesta
    /// no se puede parsear. Incluye el contexto del test que falló.
    pub async fn run_benchmark(
        &self,
        model_name: &str,
        provider: &ProviderEntry,
    ) -> Result<ModelProfile, String> {
        let mut profile = ModelProfile::new(model_name, &provider.name);
        let mut all_results: Vec<ScorerResult> = Vec::new();

        for test in &self.tests {
            tracing::debug!(
                "Benchmark [{}/{}] {} — {}",
                all_results.len() + 1,
                self.tests.len(),
                test.id,
                test.dimension.label(),
            );

            match self.run_single_test(test, model_name, provider).await {
                Ok(result) => {
                    tracing::info!(
                        "  → {}: {:.2} ({}ms)",
                        test.id,
                        result.score,
                        result.latency_ms.unwrap_or(0),
                    );
                    if let Some(ref cost) = estimate_cost(model_name, result.latency_ms) {
                        profile.add_cost(*cost);
                    }
                    all_results.push(result);
                }
                Err(e) => {
                    tracing::error!("  → {} FAILED: {}", test.id, e);
                    // Push a failed result so we don't skip the dimension
                    all_results.push(ScorerResult {
                        score: 0.0,
                        details: format!("Test failed: {}", e),
                        latency_ms: None,
                        dimension: test.dimension,
                        test_id: test.id.clone(),
                        response_snippet: String::new(),
                    });
                }
            }
        }

        Self::update_profile(&mut profile, &all_results);
        profile.mark_benchmarked();

        Ok(profile)
    }

    // ------------------------------------------------------------------
    // run_single_test
    // ------------------------------------------------------------------

    /// Ejecuta un test individual contra el modelo.
    ///
    /// Envía el prompt al LLM, cronometra la respuesta, y evalúa
    /// el resultado contra el expected_output del test.
    pub async fn run_single_test(
        &self,
        test: &BenchmarkTest,
        model_name: &str,
        provider: &ProviderEntry,
    ) -> Result<ScorerResult, String> {
        let start = Instant::now();

        let response_text = match provider.provider_type.as_str() {
            "anthropic" => {
                self.call_anthropic(model_name, &test.prompt, provider)
                    .await?
            }
            _ => {
                // Default: OpenAI-compatible (OpenAI, OpenRouter, Ollama, vLLM, etc.)
                self.call_openai_compatible(model_name, &test.prompt, provider)
                    .await?
            }
        };

        let latency_ms = start.elapsed().as_millis() as u64;

        let mut result = scorer::score_response(test, &response_text);
        result.latency_ms = Some(latency_ms);

        Ok(result)
    }

    // ------------------------------------------------------------------
    // update_profile
    // ------------------------------------------------------------------

    /// Actualiza un ModelProfile con los resultados de una tanda de tests.
    ///
    /// Agrupa los resultados por dimensión, promedia los scores,
    /// y almacena el resumen de cada test en el historial del perfil.
    pub fn update_profile(profile: &mut ModelProfile, results: &[ScorerResult]) {
        // Agrupar scores por dimensión
        let mut dim_scores: HashMap<DimensionType, Vec<f32>> = HashMap::new();
        for result in results {
            dim_scores
                .entry(result.dimension)
                .or_default()
                .push(result.score);
        }

        // Promediar scores por dimensión
        for (dim, scores) in &dim_scores {
            let avg = scores.iter().sum::<f32>() / scores.len() as f32;
            profile.set_score(*dim, avg);
        }

        // Almacenar resúmenes individuales
        for result in results {
            let summary = ScorerResultSummary::from_scorer_result(result);
            profile.add_test_result(summary);
        }
    }

    // ------------------------------------------------------------------
    // Llamadas HTTP a proveedores
    // ------------------------------------------------------------------

    /// Llama a un API compatible con OpenAI (chat completions).
    async fn call_openai_compatible(
        &self,
        model_name: &str,
        prompt: &str,
        provider: &ProviderEntry,
    ) -> Result<String, String> {
        let base = provider.base_url.trim_end_matches('/');
        let url = format!("{}/chat/completions", base);

        let body = serde_json::json!({
            "model": model_name,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "max_tokens": 512,
            "temperature": 0.0,
        });

        let mut req = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json");

        if let Some(ref key) = provider.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP request to {} failed: {}", url, e))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        if !status.is_success() {
            return Err(format!(
                "API error ({}): {} — {}",
                status,
                provider.name,
                text.chars().take(300).collect::<String>()
            ));
        }

        let json: Value = serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse response JSON: {} — body: {}",
                e,
                text.chars().take(200).collect::<String>()
            )
        })?;

        json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| {
                format!(
                    "Unexpected OpenAI response format (no choices[0].message.content): {}",
                    text.chars().take(200).collect::<String>()
                )
            })
    }

    /// Llama a la API de Anthropic.
    async fn call_anthropic(
        &self,
        model_name: &str,
        prompt: &str,
        provider: &ProviderEntry,
    ) -> Result<String, String> {
        let base = provider.base_url.trim_end_matches('/');
        let url = format!("{}/messages", base);

        let body = serde_json::json!({
            "model": model_name,
            "max_tokens": 512,
            "messages": [
                {"role": "user", "content": prompt}
            ],
        });

        let mut req = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01");

        if let Some(ref key) = provider.api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP request to {} failed: {}", url, e))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        if !status.is_success() {
            return Err(format!(
                "Anthropic API error ({}): {} — {}",
                status,
                provider.name,
                text.chars().take(300).collect::<String>()
            ));
        }

        let json: Value = serde_json::from_str(&text).map_err(|e| {
            format!(
                "Failed to parse Anthropic response JSON: {} — body: {}",
                e,
                text.chars().take(200).collect::<String>()
            )
        })?;

        // Anthropic response format: content[{ type: "text", text: "..." }]
        json["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| {
                format!(
                    "Unexpected Anthropic response format (no content[0].text): {}",
                    text.chars().take(200).collect::<String>()
                )
            })
    }
}

// ---------------------------------------------------------------------------
// Helper: estimación de costo
// ---------------------------------------------------------------------------

/// Estimación de costo en USD para una llamada.
///
/// Basada en valores aproximados por modelo. Retorna None si el modelo
/// no está en la tabla de costos conocidos.
fn estimate_cost(model_name: &str, latency_ms: Option<u64>) -> Option<f64> {
    // Costos aproximados por 1K tokens de output (USD)
    let cost_per_1k_output = match model_name {
        m if m.contains("gpt-4o") && m.contains("mini") => 0.000_150, // gpt-4o-mini
        m if m.contains("gpt-4o") => 0.010_000,                       // gpt-4o
        m if m.contains("gpt-4-turbo") => 0.010_000,
        m if m.contains("gpt-3.5") => 0.000_500,
        m if m.contains("claude-3.5-sonnet") => 0.015_000,
        m if m.contains("claude-3-haiku") => 0.000_250,
        m if m.contains("claude-3-opus") => 0.075_000,
        m if m.contains("llama") || m.contains("mistral") => 0.000_100,
        m if m.contains("gemini") => 0.000_050,
        _ => return None, // Unknown model, can't estimate
    };

    // Asumimos ~50 tokens de output por segundo de latencia
    let estimated_tokens = latency_ms
        .map(|ms| (ms as f64 / 1000.0 * 50.0).max(10.0))
        .unwrap_or(50.0);

    Some(cost_per_1k_output * estimated_tokens / 1000.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::scorer::ScorerType;
    use crate::benchmark::tests::default_tests;

    #[test]
    fn test_engine_new() {
        let client = Client::new();
        let engine = BenchmarkEngine::new(client);
        assert!(engine.tests.len() >= 24);
    }

    #[test]
    fn test_update_profile_basic() {
        let mut profile = ModelProfile::new("test-model", "test-provider");

        let results = vec![
            ScorerResult {
                score: 0.8,
                details: "Good".into(),
                latency_ms: Some(100),
                dimension: DimensionType::Reasoning,
                test_id: "reasoning-01".into(),
                response_snippet: "ok".into(),
            },
            ScorerResult {
                score: 0.6,
                details: "Okay".into(),
                latency_ms: Some(200),
                dimension: DimensionType::Reasoning,
                test_id: "reasoning-02".into(),
                response_snippet: "ok".into(),
            },
            ScorerResult {
                score: 1.0,
                details: "Perfect".into(),
                latency_ms: Some(50),
                dimension: DimensionType::Code,
                test_id: "code-01".into(),
                response_snippet: "ok".into(),
            },
        ];

        BenchmarkEngine::update_profile(&mut profile, &results);

        // Reasoning: (0.8 + 0.6) / 2 = 0.7
        assert!(
            (profile.get_score(DimensionType::Reasoning) - 0.7).abs() < 0.001,
            "Expected 0.7, got {}",
            profile.get_score(DimensionType::Reasoning)
        );
        // Code: 1.0
        assert!((profile.get_score(DimensionType::Code) - 1.0).abs() < 0.001);
        // Other dimensions should remain 0.0
        assert!((profile.get_score(DimensionType::Math) - 0.0).abs() < 0.001);

        assert_eq!(profile.total_tests_run, 3);
        assert_eq!(profile.test_results.len(), 3);
    }

    #[test]
    fn test_update_profile_empty() {
        let mut profile = ModelProfile::new("test", "test");
        BenchmarkEngine::update_profile(&mut profile, &[]);
        assert!(profile.scores.values().all(|s| *s == 0.0));
        assert_eq!(profile.total_tests_run, 0);
    }

    #[test]
    fn test_estimate_cost_known() {
        let cost = estimate_cost("gpt-4o", Some(1000));
        assert!(cost.is_some());
        assert!(cost.unwrap() > 0.0);
    }

    #[test]
    fn test_estimate_cost_unknown() {
        let cost = estimate_cost("some-unknown-model", Some(100));
        assert!(cost.is_none());
    }
}
