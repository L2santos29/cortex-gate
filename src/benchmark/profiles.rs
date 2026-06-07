// Cortex Gate — Model Profiles.
//
// Almacena el perfil de capacidades de un modelo basado en los resultados
// de benchmarks ejecutados internamente. Cada modelo tiene un score por
// dimensión, metadatos de cuándo se benchmarkeó por última vez, y costo
// total acumulado.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::classifier::dimensions::DimensionType;

// ---------------------------------------------------------------------------
// ModelProfile
// ---------------------------------------------------------------------------

/// Perfil completo de un modelo basado en benchmarks ejecutados.
///
/// Almacena los puntajes normalizados (0.0 – 1.0) para cada dimensión
/// cognitiva, junto con metadatos de ejecución.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    /// Nombre del modelo (ej: "gpt-4o", "claude-3.5-sonnet").
    pub name: String,
    /// Nombre del proveedor (ej: "openai", "anthropic", "openrouter").
    pub provider: String,
    /// Puntajes por dimensión (0.0 – 1.0).
    pub scores: HashMap<DimensionType, f32>,
    /// Timestamp del último benchmark completado.
    pub last_benchmarked: Option<DateTime<Utc>>,
    /// Costo total acumulado en USD de todas las ejecuciones de benchmark.
    pub total_cost: f64,
    /// Número total de tests ejecutados en este perfil.
    pub total_tests_run: u32,
    /// Tests individuales con sus resultados (historial).
    #[serde(default)]
    pub test_results: Vec<ScorerResultSummary>,
}

impl ModelProfile {
    /// Crea un nuevo perfil de modelo vacío (scores en 0.0).
    pub fn new(name: impl Into<String>, provider: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            provider: provider.into(),
            scores: DimensionType::all().into_iter().map(|d| (d, 0.0)).collect(),
            last_benchmarked: None,
            total_cost: 0.0,
            total_tests_run: 0,
            test_results: Vec::new(),
        }
    }

    /// Calcula el puntaje promedio entre todas las dimensiones.
    ///
    /// Solo considera dimensiones que tienen score > 0.0.
    /// Si ningún test se ha ejecutado, retorna 0.0.
    pub fn average_score(&self) -> f32 {
        let scored: Vec<f32> = self.scores.values().copied().filter(|s| *s > 0.0).collect();

        if scored.is_empty() {
            return 0.0;
        }

        scored.iter().sum::<f32>() / scored.len() as f32
    }

    /// Retorna el score para una dimensión específica.
    pub fn get_score(&self, dim: DimensionType) -> f32 {
        self.scores.get(&dim).copied().unwrap_or(0.0)
    }

    /// Actualiza el score de una dimensión.
    pub fn set_score(&mut self, dim: DimensionType, score: f32) {
        self.scores.insert(dim, score.clamp(0.0, 1.0));
    }

    /// Registra que se completó un benchmark.
    pub fn mark_benchmarked(&mut self) {
        self.last_benchmarked = Some(Utc::now());
    }

    /// Agrega un resumen de resultado de test al historial.
    pub fn add_test_result(&mut self, result: ScorerResultSummary) {
        self.test_results.push(result);
        self.total_tests_run += 1;
    }

    /// Agrega costo al acumulado.
    pub fn add_cost(&mut self, cost: f64) {
        self.total_cost += cost;
    }

    /// Retorna los nombres de dimensión ordenados con sus scores.
    pub fn score_summary(&self) -> Vec<(DimensionType, f32)> {
        let mut pairs: Vec<_> = self.scores.iter().map(|(d, s)| (*d, *s)).collect();
        pairs.sort_by(|a, b| a.0.label().cmp(b.0.label()));
        pairs
    }
}

// ---------------------------------------------------------------------------
// ScorerResultSummary
// ---------------------------------------------------------------------------

/// Resumen almacenable de un resultado de test individual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScorerResultSummary {
    /// ID del test ejecutado.
    pub test_id: String,
    /// Dimensión evaluada.
    pub dimension: DimensionType,
    /// Score obtenido (0.0 – 1.0).
    pub score: f32,
    /// Detalle del scoring.
    pub details: String,
    /// Latencia en milisegundos.
    pub latency_ms: Option<u64>,
    /// Timestamp de la ejecución.
    pub timestamp: DateTime<Utc>,
}

impl ScorerResultSummary {
    /// Crea un resumen desde un ScorerResult.
    pub fn from_scorer_result(result: &crate::benchmark::scorer::ScorerResult) -> Self {
        Self {
            test_id: result.test_id.clone(),
            dimension: result.dimension,
            score: result.score,
            details: result.details.clone(),
            latency_ms: result.latency_ms,
            timestamp: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_profile() {
        let profile = ModelProfile::new("gpt-4o", "openai");
        assert_eq!(profile.name, "gpt-4o");
        assert_eq!(profile.provider, "openai");
        assert_eq!(profile.scores.len(), 8);
        assert!(profile.scores.values().all(|s| *s == 0.0));
        assert!(profile.last_benchmarked.is_none());
        assert_eq!(profile.total_cost, 0.0);
        assert_eq!(profile.total_tests_run, 0);
    }

    #[test]
    fn test_average_score_empty() {
        let profile = ModelProfile::new("test", "test");
        assert!((profile.average_score() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_average_score_partial() {
        let mut profile = ModelProfile::new("test", "test");
        profile.set_score(DimensionType::Reasoning, 0.8);
        profile.set_score(DimensionType::Code, 0.6);
        assert!((profile.average_score() - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_average_score_all() {
        let mut profile = ModelProfile::new("test", "test");
        for dim in DimensionType::all() {
            profile.set_score(dim, 0.5);
        }
        assert!((profile.average_score() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_get_set_score() {
        let mut profile = ModelProfile::new("test", "test");
        profile.set_score(DimensionType::Math, 0.9);
        assert!((profile.get_score(DimensionType::Math) - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_set_score_clamp() {
        let mut profile = ModelProfile::new("test", "test");
        profile.set_score(DimensionType::Math, 1.5);
        assert!((profile.get_score(DimensionType::Math) - 1.0).abs() < 0.001);

        profile.set_score(DimensionType::Code, -0.5);
        assert!((profile.get_score(DimensionType::Code) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_score_summary_ordered() {
        let mut profile = ModelProfile::new("test", "test");
        profile.set_score(DimensionType::Code, 0.5);
        profile.set_score(DimensionType::Reasoning, 1.0);

        let summary = profile.score_summary();
        // Should be sorted by label alphabetically
        assert_eq!(summary.first().unwrap().0, DimensionType::Code);
        assert_eq!(summary.last().unwrap().0, DimensionType::Speed);
    }

    #[test]
    fn test_mark_benchmarked() {
        let mut profile = ModelProfile::new("test", "test");
        assert!(profile.last_benchmarked.is_none());
        profile.mark_benchmarked();
        assert!(profile.last_benchmarked.is_some());
    }

    #[test]
    fn test_add_test_result() {
        let mut profile = ModelProfile::new("test", "test");
        let summary = ScorerResultSummary {
            test_id: "math-01".into(),
            dimension: DimensionType::Math,
            score: 1.0,
            details: "Perfect".into(),
            latency_ms: Some(150),
            timestamp: Utc::now(),
        };
        profile.add_test_result(summary);
        assert_eq!(profile.total_tests_run, 1);
        assert_eq!(profile.test_results.len(), 1);
    }
}
