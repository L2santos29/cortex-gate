// Cortex Gate — Routing Equation.
//
// Motor de decisión que combina las dimensiones del ecualizador con los
// perfiles de cada modelo para seleccionar el LLM más adecuado para cada
// prompt entrante.
//
// ## Ecuación de Routing
//
// ```text
// score(m) = quality(m) × economy_factor(cost_factor(m), economy)
// quality(m) = Σ(dim_i × weight_i × intensity_i)
//
// Donde:
//   dim_i         = benchmark_score del modelo m en la dimensión i
//   weight_i      = peso configurado por el usuario para la dimensión i
//   intensity_i   = intensidad detectada por el embedding en la dimensión i
//   cost_factor   = costo normalizado del modelo (0.0 = más barato, 1.0 = más caro)
//   economy       = nivel global de economía (0.0 = calidad, 1.0 = costo)
// ```
//
// ## Thresholds de Confianza
//
// | Rango         | Estado      | Acción                                          |
// |---------------|-------------|-------------------------------------------------|
// | > 0.70        | Confident   | Usar el modelo seleccionado (state = "normal")  |
// | 0.40 – 0.70   | Ambiguous   | Default a MEDIUM (gpt-4o-mini, state = "ambiguous") |
// | < 0.40        | Safe mode   | Default a modelo barato (gemini-2.0-flash-lite, state = "safe_mode") |
//
// La confianza se calcula sobre la calidad pura (sin economía) para que
// refleje exclusivamente la certeza semántica del clasificador, no el
// costo. La economía solo influye en el ranking final de modelos y en
// la decisión de qué default usar.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::dimensions::{DimensionType, Ecualizador};

// ---------------------------------------------------------------------------
// ModelProfile
// ---------------------------------------------------------------------------

/// Perfil de capacidades de un modelo LLM.
///
/// Almacena los scores de benchmark del modelo en cada dimensión,
/// su proveedor, y su costo por millón de tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    /// Nombre del modelo (ej: "gpt-4o", "claude-3.5-sonnet").
    pub name: String,
    /// Proveedor al que pertenece (ej: "openai", "anthropic").
    pub provider: String,
    /// Puntuaciones del modelo en cada dimensión (0.0 – 1.0).
    /// Son scores de benchmark reales o estimados.
    pub scores: HashMap<DimensionType, f32>,
    /// Costo en dólares por millón de tokens (input + output promedio).
    pub cost_per_mtok: f64,
}

impl ModelProfile {
    /// Crea un nuevo perfil de modelo.
    pub fn new(
        name: impl Into<String>,
        provider: impl Into<String>,
        scores: HashMap<DimensionType, f32>,
        cost_per_mtok: f64,
    ) -> Self {
        Self {
            name: name.into(),
            provider: provider.into(),
            scores,
            cost_per_mtok,
        }
    }

    /// Obtiene el score del modelo en una dimensión, o 0.0 si no está registrado.
    pub fn score_for(&self, dim: DimensionType) -> f32 {
        self.scores.get(&dim).copied().unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// ConfidenceLevel
// ---------------------------------------------------------------------------

/// Nivel de confianza del clasificador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    /// Confianza alta (> 0.70): usar modelo seleccionado.
    Confident,
    /// Confianza media (0.40 – 0.70): ambiguous, default a MEDIUM.
    Ambiguous,
    /// Confianza baja (< 0.40): safe mode, modelo barato.
    SafeMode,
}

impl ConfidenceLevel {
    /// Determina el nivel de confianza a partir de un valor.
    pub fn from_score(confidence: f64) -> Self {
        if confidence > 0.70 {
            ConfidenceLevel::Confident
        } else if confidence >= 0.40 {
            ConfidenceLevel::Ambiguous
        } else {
            ConfidenceLevel::SafeMode
        }
    }

    /// Retorna el modelo por defecto y el modo según el nivel de confianza.
    pub fn default_model_and_mode(&self) -> (&'static str, &'static str) {
        match self {
            ConfidenceLevel::Confident => ("", "normal"),
            ConfidenceLevel::Ambiguous => ("gpt-4o-mini", "ambiguous"),
            ConfidenceLevel::SafeMode => ("gemini-2.0-flash-lite", "safe_mode"),
        }
    }
}

// ---------------------------------------------------------------------------
// RoutingEquation
// ---------------------------------------------------------------------------

/// Motor de la ecuación de routing.
///
/// Combina el ecualizador (pesos del usuario + intensidades del embedding)
/// con los perfiles de cada modelo para calcular el score y seleccionar
/// el modelo óptimo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingEquation {
    /// Ecualizador con pesos e intensidades actuales.
    pub ecualizador: Ecualizador,
    /// Perfiles de todos los modelos disponibles.
    pub model_profiles: HashMap<String, ModelProfile>,
}

impl RoutingEquation {
    /// Crea una nueva ecuación de routing con un ecualizador por defecto.
    pub fn new(ecualizador: Ecualizador) -> Self {
        Self {
            ecualizador,
            model_profiles: HashMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Gestión de modelos
    // -----------------------------------------------------------------------

    /// Registra un perfil de modelo.
    pub fn register_model(&mut self, profile: ModelProfile) {
        self.model_profiles.insert(profile.name.clone(), profile);
    }

    /// Elimina un modelo por su nombre.
    pub fn unregister_model(&mut self, name: &str) {
        self.model_profiles.remove(name);
    }

    /// Retorna el modelo más barato (menor costo por MTok).
    pub fn cheapest_model(&self) -> Option<&ModelProfile> {
        self.model_profiles
            .values()
            .min_by(|a, b| a.cost_per_mtok.partial_cmp(&b.cost_per_mtok).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Retorna el modelo más caro (mayor costo por MTok).
    pub fn most_expensive_model(&self) -> Option<&ModelProfile> {
        self.model_profiles
            .values()
            .max_by(|a, b| a.cost_per_mtok.partial_cmp(&b.cost_per_mtok).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Retorna el rango de costos (min, max) entre todos los modelos.
    pub fn cost_range(&self) -> (f64, f64) {
        let min = self
            .model_profiles
            .values()
            .map(|m| m.cost_per_mtok)
            .fold(f64::INFINITY, f64::min);

        let max = self
            .model_profiles
            .values()
            .map(|m| m.cost_per_mtok)
            .fold(f64::NEG_INFINITY, f64::max);

        if min.is_infinite() || max.is_infinite() {
            (0.0, 0.0)
        } else {
            (min, max)
        }
    }

    // -----------------------------------------------------------------------
    // Ecuación de Routing — métodos estáticos
    // -----------------------------------------------------------------------

    /// Normaliza un costo dentro de un rango [min, max] → [0.0, 1.0].
    ///
    /// - Si min == max (rango plano), retorna 0.0.
    /// - Si cost == min, retorna 0.0 (más barato).
    /// - Si cost == max, retorna 1.0 (más caro).
    pub fn normalize_cost(cost: f64, min: f64, max: f64) -> f64 {
        if max <= min {
            return 0.0;
        }
        ((cost - min) / (max - min)).clamp(0.0, 1.0)
    }

    /// Calcula el factor de economía.
    ///
    /// `economy_factor = 1 - economy × cost_factor`
    ///
    /// - Si economy = 0.0 → factor = 1.0 (el costo no influye, calidad total)
    /// - Si economy = 1.0, cost_factor = 0.0 (barato) → factor = 1.0
    /// - Si economy = 1.0, cost_factor = 1.0 (caro) → factor = 0.0
    /// - Si economy = 0.5, cost_factor = 0.5 → factor = 0.75
    ///
    /// `cost_factor` es el costo normalizado del modelo (0.0 = más barato, 1.0 = más caro).
    pub fn economy_factor(cost_factor: f64, economy: f64) -> f64 {
        (1.0 - economy * cost_factor).clamp(0.0, 1.0)
    }

    // -----------------------------------------------------------------------
    // Calidad pura (sin economía)
    // -----------------------------------------------------------------------

    /// Calcula la calidad pura de un modelo sin factor económico.
    ///
    /// `quality = Σ(benchmark_i × weight_i × intensity_i)`
    ///
    /// Este valor se usa para la confianza del clasificador (refleja certeza
    /// semántica, no influencia del costo).
    pub fn raw_quality(
        &self,
        prompt_scores: &HashMap<DimensionType, f32>,
        model: &ModelProfile,
    ) -> f64 {
        let mut quality = 0.0_f64;

        for dim in DimensionType::all() {
            let weight = self
                .ecualizador
                .get(dim)
                .map(|d| d.weight as f64)
                .unwrap_or(0.125);

            let intensity = prompt_scores
                .get(&dim)
                .copied()
                .unwrap_or(0.0) as f64;

            let benchmark = model.score_for(dim) as f64;

            quality += benchmark * weight * intensity;
        }

        quality
    }

    // -----------------------------------------------------------------------
    // Ecuación de Routing — score completo
    // -----------------------------------------------------------------------

    /// Calcula el score completo para un modelo: calidad × factor económico.
    ///
    /// ## Ecuación
    /// ```text
    /// score = raw_quality(prompt_scores, model) × economy_factor(cost_factor, economy)
    /// ```
    pub fn score_model(
        &self,
        prompt_scores: &HashMap<DimensionType, f32>,
        model: &ModelProfile,
    ) -> f64 {
        let quality = self.raw_quality(prompt_scores, model);

        let (min_cost, max_cost) = self.cost_range();
        let cost_factor = RoutingEquation::normalize_cost(model.cost_per_mtok, min_cost, max_cost);

        let economy = self.ecualizador.economy as f64;
        let e_factor = RoutingEquation::economy_factor(cost_factor, economy);

        quality * e_factor
    }

    // -----------------------------------------------------------------------
    // Selección de modelo
    // -----------------------------------------------------------------------

    /// Selecciona el mejor modelo para un vector de intensidades dado.
    ///
    /// La confianza se calcula sobre la **calidad pura** (sin economía).
    /// La selección final usa el **score completo** (con economía).
    ///
    /// # Returns
    /// `(model_name, confidence, mode)`
    ///
    /// - `model_name`: nombre del modelo seleccionado (o default según confianza)
    /// - `confidence`: valor de confianza (0.0 – 1.0)
    /// - `mode`: "normal", "ambiguous", o "safe_mode"
    pub fn select_best_model(
        &self,
        prompt_scores: &HashMap<DimensionType, f32>,
    ) -> (String, f64, String) {
        if self.model_profiles.is_empty() {
            return (
                "gemini-2.0-flash-lite".to_string(),
                0.0,
                "safe_mode".to_string(),
            );
        }

        // --- Ranking por score completo (calidad × economía) ---
        let mut ranked: Vec<(f64, f64, &ModelProfile)> = self
            .model_profiles
            .values()
            .map(|model| {
                let total = self.score_model(prompt_scores, model);
                let quality = self.raw_quality(prompt_scores, model);
                (total, quality, model)
            })
            .collect();

        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let (_best_total, best_quality, best_model) = match ranked.first() {
            Some((t, q, m)) => (*t, *q, *m),
            None => {
                return (
                    "gemini-2.0-flash-lite".to_string(),
                    0.0,
                    "safe_mode".to_string(),
                );
            }
        };

        let second_quality = ranked
            .get(1)
            .map(|(_, q, _)| *q)
            .unwrap_or(best_quality);

        // Confianza sobre calidad pura (sin economía)
        let confidence = if best_quality > 0.0 {
            (best_quality - second_quality) / best_quality
        } else {
            0.0
        };

        // Nivel de confianza
        let level = ConfidenceLevel::from_score(confidence);
        let (default_model, mode) = level.default_model_and_mode();

        // Modo normal: usar el modelo con mejor SCORE TOTAL
        if level == ConfidenceLevel::Confident {
            return (best_model.name.clone(), confidence, mode.to_string());
        }

        // Ambigüedad o safe mode: usar default
        (default_model.to_string(), confidence, mode.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::dimensions::Ecualizador;

    fn make_profile(
        name: &str,
        provider: &str,
        cost: f64,
        scores: &[(DimensionType, f32)],
    ) -> ModelProfile {
        let scores_map: HashMap<DimensionType, f32> = scores.iter().copied().collect();
        ModelProfile::new(name, provider, scores_map, cost)
    }

    fn make_prompt_scores(scores: &[(DimensionType, f32)]) -> HashMap<DimensionType, f32> {
        scores.iter().copied().collect()
    }

    // -----------------------------------------------------------------------
    // ModelProfile
    // -----------------------------------------------------------------------

    #[test]
    fn test_model_profile_score_for() {
        let mut scores = HashMap::new();
        scores.insert(DimensionType::Reasoning, 0.95);
        let profile = ModelProfile::new("test-model", "test-provider", scores, 1.0);

        assert!((profile.score_for(DimensionType::Reasoning) - 0.95).abs() < f32::EPSILON);
        assert_eq!(profile.score_for(DimensionType::Code), 0.0);
    }

    // -----------------------------------------------------------------------
    // ConfidenceLevel
    // -----------------------------------------------------------------------

    #[test]
    fn test_confidence_levels() {
        assert_eq!(ConfidenceLevel::from_score(0.80), ConfidenceLevel::Confident);
        assert_eq!(ConfidenceLevel::from_score(0.71), ConfidenceLevel::Confident);

        assert_eq!(ConfidenceLevel::from_score(0.70), ConfidenceLevel::Ambiguous);
        assert_eq!(ConfidenceLevel::from_score(0.55), ConfidenceLevel::Ambiguous);
        assert_eq!(ConfidenceLevel::from_score(0.40), ConfidenceLevel::Ambiguous);

        assert_eq!(ConfidenceLevel::from_score(0.39), ConfidenceLevel::SafeMode);
        assert_eq!(ConfidenceLevel::from_score(0.0), ConfidenceLevel::SafeMode);
    }

    // -----------------------------------------------------------------------
    // economy_factor
    // -----------------------------------------------------------------------

    #[test]
    fn test_economy_factor_basics() {
        // economy=0 → factor=1.0 (no penalty)
        assert!((RoutingEquation::economy_factor(0.5, 0.0) - 1.0).abs() < f64::EPSILON);

        // economy=1, cost_factor=1 → factor=0.0
        assert!((RoutingEquation::economy_factor(1.0, 1.0) - 0.0).abs() < f64::EPSILON);

        // economy=1, cost_factor=0 → factor=1.0
        assert!((RoutingEquation::economy_factor(0.0, 1.0) - 1.0).abs() < f64::EPSILON);

        // economy=0.5, cost_factor=0.5 → factor=0.75
        assert!((RoutingEquation::economy_factor(0.5, 0.5) - 0.75).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // normalize_cost
    // -----------------------------------------------------------------------

    #[test]
    fn test_normalize_cost() {
        assert!((RoutingEquation::normalize_cost(1.0, 1.0, 100.0) - 0.0).abs() < f64::EPSILON);
        assert!((RoutingEquation::normalize_cost(100.0, 1.0, 100.0) - 1.0).abs() < f64::EPSILON);
        assert!((RoutingEquation::normalize_cost(50.5, 1.0, 100.0) - 0.5).abs() < 0.01);
        // Rango plano
        assert!((RoutingEquation::normalize_cost(10.0, 5.0, 5.0) - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // cost_range
    // -----------------------------------------------------------------------

    #[test]
    fn test_cost_range() {
        let mut equation = RoutingEquation::new(Ecualizador::default());
        equation.register_model(make_profile("cheap", "t1", 1.0, &[]));
        equation.register_model(make_profile("mid", "t1", 10.0, &[]));
        equation.register_model(make_profile("expensive", "t1", 100.0, &[]));

        let (min, max) = equation.cost_range();
        assert!((min - 1.0).abs() < f64::EPSILON);
        assert!((max - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cost_range_empty() {
        let equation = RoutingEquation::new(Ecualizador::default());
        let (min, max) = equation.cost_range();
        assert!((min - 0.0).abs() < f64::EPSILON);
        assert!((max - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // register / unregister
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_and_unregister() {
        let mut equation = RoutingEquation::new(Ecualizador::default());
        assert!(equation.model_profiles.is_empty());

        equation.register_model(make_profile("test-model", "test", 5.0, &[]));
        assert_eq!(equation.model_profiles.len(), 1);

        equation.unregister_model("test-model");
        assert!(equation.model_profiles.is_empty());
    }

    // -----------------------------------------------------------------------
    // cheapest / most expensive
    // -----------------------------------------------------------------------

    #[test]
    fn test_cheapest_and_most_expensive() {
        let mut equation = RoutingEquation::new(Ecualizador::default());
        equation.register_model(make_profile("cheap", "t1", 1.0, &[]));
        equation.register_model(make_profile("mid", "t1", 10.0, &[]));
        equation.register_model(make_profile("expensive", "t1", 100.0, &[]));

        assert_eq!(equation.cheapest_model().unwrap().name, "cheap");
        assert_eq!(equation.most_expensive_model().unwrap().name, "expensive");
    }

    #[test]
    fn test_cheapest_most_expensive_empty() {
        let equation = RoutingEquation::new(Ecualizador::default());
        assert!(equation.cheapest_model().is_none());
        assert!(equation.most_expensive_model().is_none());
    }

    // -----------------------------------------------------------------------
    // score_model / raw_quality
    // -----------------------------------------------------------------------

    #[test]
    fn test_score_model_basic() {
        let eq = Ecualizador::default();
        let equation = RoutingEquation::new(eq);

        let profile = make_profile(
            "gpt-4o",
            "openai",
            10.0,
            &[
                (DimensionType::Reasoning, 0.9),
                (DimensionType::Code, 0.85),
                (DimensionType::Math, 0.8),
                (DimensionType::Creativity, 0.6),
                (DimensionType::Precision, 0.85),
                (DimensionType::Speed, 0.5),
                (DimensionType::Context, 0.7),
                (DimensionType::Safety, 0.6),
            ],
        );

        let prompt = make_prompt_scores(&[
            (DimensionType::Reasoning, 1.0),
            (DimensionType::Code, 0.8),
        ]);

        let score = equation.score_model(&prompt, &profile);
        assert!(score >= 0.0 && score <= 1.0, "Score should be in [0,1], got {score}");
    }

    #[test]
    fn test_score_model_with_intensities() {
        let mut eq = Ecualizador::default();
        eq.normalize_weights();
        let equation = RoutingEquation::new(eq);

        let profile = make_profile(
            "reasoning-model",
            "test",
            5.0,
            &[
                (DimensionType::Reasoning, 0.95),
                (DimensionType::Code, 0.3),
                (DimensionType::Creativity, 0.1),
                (DimensionType::Math, 0.4),
                (DimensionType::Precision, 0.5),
                (DimensionType::Speed, 0.2),
                (DimensionType::Context, 0.3),
                (DimensionType::Safety, 0.3),
            ],
        );

        let prompt = make_prompt_scores(&[(DimensionType::Reasoning, 1.0)]);

        let score = equation.score_model(&prompt, &profile);
        assert!(score >= 0.0 && score <= 1.0);
        assert!(score > 0.0);
    }

    #[test]
    fn test_scoring_different_prompts() {
        let eq = Ecualizador::default();
        let equation = RoutingEquation::new(eq);

        let profile = make_profile(
            "general-model",
            "test",
            10.0,
            &[
                (DimensionType::Reasoning, 0.9),
                (DimensionType::Code, 0.3),
                (DimensionType::Creativity, 0.1),
                (DimensionType::Math, 0.9),
                (DimensionType::Precision, 0.8),
                (DimensionType::Speed, 0.2),
                (DimensionType::Context, 0.5),
                (DimensionType::Safety, 0.4),
            ],
        );

        let math_prompt = make_prompt_scores(&[
            (DimensionType::Math, 1.0),
            (DimensionType::Reasoning, 0.9),
        ]);
        let math_score = equation.score_model(&math_prompt, &profile);

        let creative_prompt = make_prompt_scores(&[(DimensionType::Creativity, 1.0)]);
        let creative_score = equation.score_model(&creative_prompt, &profile);

        assert!(
            math_score > creative_score,
            "Math score ({math_score:.4}) should be > creative score ({creative_score:.4})"
        );
    }

    #[test]
    fn test_raw_quality_ignores_economy() {
        let mut eq = Ecualizador::default();
        eq.adjust_economy(1.0);
        let equation = RoutingEquation::new(eq);

        let profile = make_profile("m", "p", 10.0, &[(DimensionType::Reasoning, 0.5)]);
        let prompt = make_prompt_scores(&[(DimensionType::Reasoning, 1.0)]);

        let raw = equation.raw_quality(&prompt, &profile);
        let scored = equation.score_model(&prompt, &profile);

        // raw_quality no debe incluir economía
        // quality = 0.5 * 0.125 * 1.0 = 0.0625
        // score = 0.0625 * economy_factor
        // con cost_range solo este modelo: min=max=10 → cost_factor=0 → e_factor=1
        // Ambos iguales porque hay un solo modelo (cost_range plano)
        assert!(raw > 0.0);
        assert!(scored > 0.0);
    }

    // -----------------------------------------------------------------------
    // economy_penalty
    // -----------------------------------------------------------------------

    #[test]
    fn test_economy_penalty() {
        let mut eq = Ecualizador::default();
        eq.adjust_economy(1.0); // máxima economía
        let mut equation = RoutingEquation::new(eq);

        let cheap = make_profile(
            "cheap",
            "test",
            1.0,
            &[
                (DimensionType::Reasoning, 0.5),
                (DimensionType::Code, 0.5),
                (DimensionType::Creativity, 0.5),
                (DimensionType::Math, 0.5),
                (DimensionType::Precision, 0.5),
                (DimensionType::Speed, 0.5),
                (DimensionType::Context, 0.5),
                (DimensionType::Safety, 0.5),
            ],
        );
        let expensive = make_profile(
            "expensive",
            "test",
            100.0,
            &[
                (DimensionType::Reasoning, 0.9),
                (DimensionType::Code, 0.9),
                (DimensionType::Creativity, 0.9),
                (DimensionType::Math, 0.9),
                (DimensionType::Precision, 0.9),
                (DimensionType::Speed, 0.9),
                (DimensionType::Context, 0.9),
                (DimensionType::Safety, 0.9),
            ],
        );

        equation.register_model(cheap.clone());
        equation.register_model(expensive.clone());

        let prompt = make_prompt_scores(&[
            (DimensionType::Reasoning, 0.5),
            (DimensionType::Code, 0.5),
            (DimensionType::Creativity, 0.5),
            (DimensionType::Math, 0.5),
            (DimensionType::Precision, 0.5),
            (DimensionType::Speed, 0.5),
            (DimensionType::Context, 0.5),
            (DimensionType::Safety, 0.5),
        ]);

        let cheap_score = equation.score_model(&prompt, &cheap);
        let expensive_score = equation.score_model(&prompt, &expensive);

        // Con economy=1.0, el barato gana
        assert!(
            cheap_score > expensive_score,
            "Cheap {cheap_score:.4} should beat expensive {expensive_score:.4} at economy=1.0"
        );
    }

    // -----------------------------------------------------------------------
    // select_best_model
    // -----------------------------------------------------------------------

    #[test]
    fn test_select_best_model_normal() {
        let mut equation = RoutingEquation::new(Ecualizador::default());

        equation.register_model(make_profile(
            "gpt-4o",
            "openai",
            10.0,
            &[
                (DimensionType::Reasoning, 0.9),
                (DimensionType::Code, 0.9),
                (DimensionType::Creativity, 0.9),
                (DimensionType::Math, 0.9),
                (DimensionType::Precision, 0.9),
                (DimensionType::Speed, 0.9),
                (DimensionType::Context, 0.9),
                (DimensionType::Safety, 0.9),
            ],
        ));
        equation.register_model(make_profile(
            "gpt-4o-mini",
            "openai",
            2.0,
            &[
                (DimensionType::Reasoning, 0.2),
                (DimensionType::Code, 0.2),
                (DimensionType::Creativity, 0.2),
                (DimensionType::Math, 0.2),
                (DimensionType::Precision, 0.2),
                (DimensionType::Speed, 0.2),
                (DimensionType::Context, 0.2),
                (DimensionType::Safety, 0.2),
            ],
        ));

        let prompt = make_prompt_scores(&[
            (DimensionType::Reasoning, 1.0),
            (DimensionType::Code, 1.0),
            (DimensionType::Creativity, 1.0),
            (DimensionType::Math, 1.0),
            (DimensionType::Precision, 1.0),
            (DimensionType::Speed, 1.0),
            (DimensionType::Context, 1.0),
            (DimensionType::Safety, 1.0),
        ]);

        let (model, confidence, mode) = equation.select_best_model(&prompt);
        assert_eq!(model, "gpt-4o");
        assert!(confidence > 0.70);
        assert_eq!(mode, "normal");
    }

    #[test]
    fn test_select_best_model_ambiguous() {
        let mut equation = RoutingEquation::new(Ecualizador::default());

        equation.register_model(make_profile(
            "model-a",
            "test",
            10.0,
            &[
                (DimensionType::Reasoning, 0.6),
                (DimensionType::Code, 0.6),
                (DimensionType::Creativity, 0.6),
                (DimensionType::Math, 0.6),
                (DimensionType::Precision, 0.6),
                (DimensionType::Speed, 0.6),
                (DimensionType::Context, 0.6),
                (DimensionType::Safety, 0.6),
            ],
        ));
        equation.register_model(make_profile(
            "model-b",
            "test",
            10.0,
            &[
                (DimensionType::Reasoning, 0.3),
                (DimensionType::Code, 0.3),
                (DimensionType::Creativity, 0.3),
                (DimensionType::Math, 0.3),
                (DimensionType::Precision, 0.3),
                (DimensionType::Speed, 0.3),
                (DimensionType::Context, 0.3),
                (DimensionType::Safety, 0.3),
            ],
        ));

        let prompt = make_prompt_scores(&[
            (DimensionType::Reasoning, 0.5),
            (DimensionType::Code, 0.5),
            (DimensionType::Creativity, 0.5),
            (DimensionType::Math, 0.5),
            (DimensionType::Precision, 0.5),
            (DimensionType::Speed, 0.5),
            (DimensionType::Context, 0.5),
            (DimensionType::Safety, 0.5),
        ]);

        let (model, confidence, mode) = equation.select_best_model(&prompt);
        assert_eq!(mode, "ambiguous");
        assert_eq!(model, "gpt-4o-mini");
        assert!(confidence >= 0.40 && confidence <= 0.70);
    }

    #[test]
    fn test_select_best_model_safe_mode() {
        let mut equation = RoutingEquation::new(Ecualizador::default());

        equation.register_model(make_profile(
            "model-a",
            "test",
            10.0,
            &[
                (DimensionType::Reasoning, 0.1),
                (DimensionType::Code, 0.1),
                (DimensionType::Creativity, 0.1),
                (DimensionType::Math, 0.1),
                (DimensionType::Precision, 0.1),
                (DimensionType::Speed, 0.1),
                (DimensionType::Context, 0.1),
                (DimensionType::Safety, 0.1),
            ],
        ));

        let prompt = make_prompt_scores(&[
            (DimensionType::Reasoning, 0.1),
            (DimensionType::Code, 0.1),
            (DimensionType::Creativity, 0.1),
            (DimensionType::Math, 0.1),
            (DimensionType::Precision, 0.1),
            (DimensionType::Speed, 0.1),
            (DimensionType::Context, 0.1),
            (DimensionType::Safety, 0.1),
        ]);

        let (model, confidence, mode) = equation.select_best_model(&prompt);
        assert_eq!(mode, "safe_mode");
        assert_eq!(model, "gemini-2.0-flash-lite");
        assert!(confidence < 0.40);
    }

    #[test]
    fn test_select_best_model_no_models() {
        let equation = RoutingEquation::new(Ecualizador::default());
        let prompt = make_prompt_scores(&[(DimensionType::Reasoning, 1.0)]);
        let (model, confidence, mode) = equation.select_best_model(&prompt);
        assert_eq!(mode, "safe_mode");
        assert_eq!(model, "gemini-2.0-flash-lite");
        assert_eq!(confidence, 0.0);
    }

    // -----------------------------------------------------------------------
    // score_model — edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_score_model_no_intensity() {
        let equation = RoutingEquation::new(Ecualizador::default());
        let profile = make_profile("m", "p", 5.0, &[(DimensionType::Reasoning, 0.9)]);

        let prompt = make_prompt_scores(&[]);
        let score = equation.score_model(&prompt, &profile);
        assert!((score - 0.0).abs() < f64::EPSILON);
    }
}
