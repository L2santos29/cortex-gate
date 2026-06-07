// Cortex Gate — Dimension System.
//
// Define las dimensiones de clasificación que el ecualizador combina
// para evaluar qué modelo es el más adecuado para cada prompt.
//
// Cada dimensión representa una capacidad cognitiva que un LLM puede
// tener en mayor o menor grado. El ecualizador permite al usuario
// ajustar pesos y nivel de economía para balancear calidad vs. costo.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// DimensionType
// ---------------------------------------------------------------------------

/// Tipos de dimensión cognitiva.
///
/// Cada variante representa un eje independiente de capacidad del modelo.
/// Un prompt puede requerir alta precisión matemática pero poca creatividad,
/// o mucho razonamiento pero poca velocidad, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DimensionType {
    /// Razonamiento complejo, cadenas de pensamiento largas, lógica formal.
    Reasoning,
    /// Generación de código, debugging, refactorización.
    Code,
    /// Creatividad, escritura literaria, brainstorming, ideas novedosas.
    Creativity,
    /// Cálculos matemáticos, álgebra, estadística, resolución numérica.
    Math,
    /// Tareas que requieren alta precisión factual y exactitud.
    Precision,
    /// Baja latencia, alto throughput, respuestas rápidas.
    Speed,
    /// Contextos muy largos, ventanas de atención extendidas (>32K).
    Context,
    /// Cumplimiento, seguridad, filtrado de contenido dañino.
    Safety,
}

impl DimensionType {
    /// Retorna una lista con todas las dimensiones disponibles.
    pub fn all() -> Vec<DimensionType> {
        vec![
            DimensionType::Reasoning,
            DimensionType::Code,
            DimensionType::Creativity,
            DimensionType::Math,
            DimensionType::Precision,
            DimensionType::Speed,
            DimensionType::Context,
            DimensionType::Safety,
        ]
    }

    /// Etiqueta corta legible para UI/display.
    pub fn label(&self) -> &'static str {
        match self {
            DimensionType::Reasoning  => "Reasoning",
            DimensionType::Code       => "Code",
            DimensionType::Creativity => "Creativity",
            DimensionType::Math       => "Math",
            DimensionType::Precision  => "Precision",
            DimensionType::Speed      => "Speed",
            DimensionType::Context    => "Context",
            DimensionType::Safety     => "Safety",
        }
    }

    /// Descripción breve de la dimensión.
    pub fn description(&self) -> &'static str {
        match self {
            DimensionType::Reasoning  => "Complex reasoning, chain-of-thought, formal logic",
            DimensionType::Code       => "Code generation, debugging, refactoring",
            DimensionType::Creativity => "Creative writing, brainstorming, novel ideas",
            DimensionType::Math       => "Math, algebra, statistics, numerical solving",
            DimensionType::Precision  => "Factual accuracy, exactness, deterministic output",
            DimensionType::Speed      => "Low latency, high throughput, fast responses",
            DimensionType::Context    => "Long context windows, extended attention (>32K)",
            DimensionType::Safety     => "Compliance, safety filtering, harm prevention",
        }
    }
}

impl fmt::Display for DimensionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ---------------------------------------------------------------------------
// Dimension
// ---------------------------------------------------------------------------

/// Una dimensión con su peso e intensidad actuales.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    /// Tipo de dimensión.
    pub dim_type: DimensionType,
    /// Peso asignado por el usuario (0.0 – 1.0).
    /// Controla cuánto influye esta dimensión en la decisión de ruteo.
    pub weight: f32,
    /// Intensidad detectada en el prompt (0.0 – 1.0).
    /// Es el score que el clasificador de embeddings asigna a esta dimensión.
    pub intensity: f32,
    /// Puntuación de benchmark del modelo en esta dimensión (0.0 – 1.0).
    /// Proviene de evaluaciones reales (MMLU, HumanEval, GSM8K, etc.).
    pub benchmark_score: f32,
}

impl Dimension {
    /// Crea una nueva dimensión con peso e intensidad por defecto.
    pub fn new(dim_type: DimensionType) -> Self {
        Self {
            dim_type,
            weight: 0.125,     // peso equilibrado entre 8 dimensiones
            intensity: 0.0,
            benchmark_score: 0.5,
        }
    }

    /// Valor efectivo = weight * intensity (para routing).
    pub fn effective_value(&self) -> f32 {
        self.weight * self.intensity
    }

    /// Score compuesto = weight * intensity * benchmark_score (para ecuación completa).
    pub fn composite_score(&self) -> f32 {
        self.weight * self.intensity * self.benchmark_score
    }
}

// ---------------------------------------------------------------------------
// Ecualizador
// ---------------------------------------------------------------------------

/// El ecualizador — panel de control de routing.
///
/// Combina los pesos de cada dimensión con el nivel de economía para
/// determinar el modelo óptimo para cada prompt.
///
/// ## Analogía
/// Como un ecualizador gráfico de audio: cada slider (dimensión) ajusta
/// cuánto peso darle a esa capacidad. El dial de economía controla el
/// balance general entre calidad (modelos caros) y costo (modelos baratos).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ecualizador {
    /// Dimensiones configuradas con sus pesos.
    pub dimensions: Vec<Dimension>,
    /// Nivel de economía global (0.0 = solo calidad, 1.0 = solo costo).
    pub economy: f32,
}

impl Ecualizador {
    /// Crea un ecualizador con valores por defecto.
    ///
    /// - `economy`: 0.5 (balanceado)
    /// - Pesos: distribuidos equitativamente entre las 8 dimensiones (0.125 cada una).
    pub fn default() -> Self {
        let dimensions = DimensionType::all()
            .into_iter()
            .map(Dimension::new)
            .collect();

        Self {
            dimensions,
            economy: 0.5,
        }
    }

    /// Ajusta el peso de una dimensión específica.
    ///
    /// El peso se clamp entre 0.0 y 1.0.
    pub fn adjust_weight(&mut self, dim: DimensionType, weight: f32) {
        let weight = weight.clamp(0.0, 1.0);
        if let Some(d) = self.dimensions.iter_mut().find(|d| d.dim_type == dim) {
            d.weight = weight;
        }
    }

    /// Ajusta el nivel de economía global.
    ///
    /// `level`: 0.0 = máxima calidad, 1.0 = máximo ahorro.
    /// Se clamp entre 0.0 y 1.0 automáticamente.
    pub fn adjust_economy(&mut self, level: f32) {
        self.economy = level.clamp(0.0, 1.0);
    }

    /// Obtiene una referencia a una dimensión por tipo.
    pub fn get(&self, dim: DimensionType) -> Option<&Dimension> {
        self.dimensions.iter().find(|d| d.dim_type == dim)
    }

    /// Obtiene una referencia mutable a una dimensión por tipo.
    pub fn get_mut(&mut self, dim: DimensionType) -> Option<&mut Dimension> {
        self.dimensions.iter_mut().find(|d| d.dim_type == dim)
    }

    /// Actualiza la intensidad de una dimensión (lo que el clasificador detectó).
    pub fn set_intensity(&mut self, dim: DimensionType, intensity: f32) {
        let intensity = intensity.clamp(0.0, 1.0);
        if let Some(d) = self.dimensions.iter_mut().find(|d| d.dim_type == dim) {
            d.intensity = intensity;
        }
    }

    /// Actualiza el benchmark_score de un modelo/dimensión.
    pub fn set_benchmark(&mut self, dim: DimensionType, score: f32) {
        let score = score.clamp(0.0, 1.0);
        if let Some(d) = self.dimensions.iter_mut().find(|d| d.dim_type == dim) {
            d.benchmark_score = score;
        }
    }

    /// Suma ponderada total de todas las dimensiones (weight * intensity).
    pub fn weighted_sum(&self) -> f32 {
        self.dimensions.iter().map(|d| d.effective_value()).sum()
    }

    /// Producto ponderado con benchmark scores.
    pub fn composite_sum(&self) -> f32 {
        self.dimensions.iter().map(|d| d.composite_score()).sum()
    }

    /// Normaliza los pesos para que sumen 1.0.
    pub fn normalize_weights(&mut self) {
        let total: f32 = self.dimensions.iter().map(|d| d.weight).sum();
        if total > 0.0 {
            for d in &mut self.dimensions {
                d.weight /= total;
            }
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
    fn test_default_ecualizador() {
        let eq = Ecualizador::default();
        assert_eq!(eq.dimensions.len(), 8);
        assert_eq!(eq.economy, 0.5);
        for d in &eq.dimensions {
            assert!((d.weight - 0.125).abs() < f32::EPSILON);
            assert_eq!(d.intensity, 0.0);
            assert_eq!(d.benchmark_score, 0.5);
        }
    }

    #[test]
    fn test_adjust_weight() {
        let mut eq = Ecualizador::default();
        eq.adjust_weight(DimensionType::Reasoning, 0.8);
        assert!((eq.get(DimensionType::Reasoning).unwrap().weight - 0.8).abs() < f32::EPSILON);

        // Clamp
        eq.adjust_weight(DimensionType::Code, 1.5);
        assert!((eq.get(DimensionType::Code).unwrap().weight - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_adjust_economy() {
        let mut eq = Ecualizador::default();
        eq.adjust_economy(0.2);
        assert!((eq.economy - 0.2).abs() < f32::EPSILON);

        // Clamp
        eq.adjust_economy(2.0);
        assert!((eq.economy - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_intensity() {
        let mut eq = Ecualizador::default();
        eq.set_intensity(DimensionType::Reasoning, 0.9);
        assert!((eq.get(DimensionType::Reasoning).unwrap().intensity - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_weighted_sum() {
        let mut eq = Ecualizador::default();
        eq.set_intensity(DimensionType::Reasoning, 1.0);
        eq.set_intensity(DimensionType::Code, 0.5);
        // reasoning: 0.125 * 1.0 = 0.125
        // code:      0.125 * 0.5 = 0.0625
        // others:    6 * 0.125 * 0.0 = 0.0
        // total: 0.1875
        assert!((eq.weighted_sum() - 0.1875).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalize_weights() {
        let mut eq = Ecualizador::default();
        eq.adjust_weight(DimensionType::Reasoning, 1.0);
        eq.adjust_weight(DimensionType::Code, 1.0);
        // Others at 0.125
        eq.normalize_weights();
        let total: f32 = eq.dimensions.iter().map(|d| d.weight).sum();
        assert!((total - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_dimension_labels() {
        assert_eq!(DimensionType::Reasoning.label(), "Reasoning");
        assert_eq!(DimensionType::Code.label(), "Code");
        assert_eq!(DimensionType::Math.label(), "Math");
        assert!(DimensionType::all().len() == 8);
    }
}
