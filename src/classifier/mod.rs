//! Embedding-based prompt classifier for Cortex Gate (STUB).
//!
//! ## WARNING
//! This module requires `ort` (ONNX Runtime) and `ndarray` crates which
//! are temporarily removed from Cargo.toml due to OpenSSL build dependency
//! issues in the current development environment.
//!
//! All types and functions are provided as stubs that compile without
//! the ONNX dependencies. Replace with the real implementation once
//! `ort` and `ndarray` are re-added to Cargo.toml and OpenSSL development
//! headers are available (install `libssl-dev` on Debian/Ubuntu).
//!
//! ## Re-enabling
//! ```bash
//! cargo add ort --features ndarray,download-binaries
//! cargo add ndarray
//! ```
//! Then restore the files under `src/classifier/`.

pub mod dimensions;
pub mod embedding;
pub mod equation;

use std::collections::HashMap;

use thiserror::Error;

use crate::classifier::dimensions::DimensionType;

// ---------------------------------------------------------------------------
// Stub error type (mirrors the real classifier errors)
// ---------------------------------------------------------------------------

/// Stub error that matches the real classifier's error surface.
#[derive(Error, Debug)]
pub enum ClassifierError {
    #[error("Classifier module is a stub — ONNX Runtime not linked")]
    OnnxNotLinked,

    #[error("No dimensions configured for classification")]
    NoDimensions,
}

// ---------------------------------------------------------------------------
// ClassificationResult
// ---------------------------------------------------------------------------

/// The result of classifying a prompt against a set of dimensions.
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// Per-dimension similarity scores (label → 0.0 … 1.0).
    pub scores: HashMap<String, f32>,

    /// Overall confidence of the classification (0.0 … 1.0).
    pub confidence: f32,
}

// ---------------------------------------------------------------------------
// ClassifierEngine (stub)
// ---------------------------------------------------------------------------

/// High-level classifier that turns a prompt into per-dimension scores.
///
/// ## Stub
/// The real implementation loads an ONNX int8 model and computes
/// embeddings. This stub returns placeholder scores based on keyword
/// heuristics so the rest of the system can be developed in parallel.
pub struct ClassifierEngine {
    /// Dimension definitions (from the project's dimension system).
    dimensions: Vec<DimensionType>,
}

impl ClassifierEngine {
    /// Create a new classifier.
    ///
    /// ## Stub
    /// Accepts a `model_path` parameter for API compatibility but
    /// does not load any model.
    pub fn new(_model_path: &str) -> Result<Self, ClassifierError> {
        let dimensions = DimensionType::all().to_vec();
        Ok(Self { dimensions })
    }

    /// Return the list of dimension types this classifier knows about.
    pub fn dimensions(&self) -> &[DimensionType] {
        &self.dimensions
    }

    /// Classify `prompt` and return per-dimension similarity scores.
    ///
    /// ## Stub
    /// Uses keyword-based heuristic scoring. Replace with ONNX
    /// embedding + centroid comparison when the real engine is linked.
    pub async fn classify(&self, prompt: &str) -> Result<ClassificationResult, ClassifierError> {
        if self.dimensions.is_empty() {
            return Err(ClassifierError::NoDimensions);
        }

        let mut scores = HashMap::with_capacity(self.dimensions.len());
        let mut total: f32 = 0.0;

        for dim in &self.dimensions {
            let score = keyword_score(prompt, dim);
            scores.insert(dim.label().to_string(), score);
            total += score;
        }

        let confidence = if self.dimensions.is_empty() {
            0.0
        } else {
            total / self.dimensions.len() as f32
        };

        Ok(ClassificationResult { scores, confidence })
    }
}

// ---------------------------------------------------------------------------
// Keyword heuristic (placeholder until ONNX engine lands)
// ---------------------------------------------------------------------------

/// Keyword-based scoring for each dimension.
fn keyword_score(prompt: &str, dim: &DimensionType) -> f32 {
    let lower = prompt.to_lowercase();

    let keywords: &[&str] = match dim {
        DimensionType::Reasoning => &[
            "reason", "think", "logic", "argument", "why", "because",
            "explain", "infer", "conclude", "deduce", "analyze",
        ],
        DimensionType::Code => &[
            "code", "function", "program", "rust", "python", "javascript",
            "debug", "compile", "api", "algorithm", "implement",
        ],
        DimensionType::Creativity => &[
            "write", "story", "poem", "creative", "imagine", "design",
            "art", "brainstorm", "novel", "invent",
        ],
        DimensionType::Math => &[
            "math", "calculate", "equation", "number", "sum", "solve",
            "integral", "derivative", "statistics", "probability",
        ],
        DimensionType::Precision => &[
            "exact", "precise", "accurate", "specific", "factual",
            "verify", "correct", "define", "confirm",
        ],
        DimensionType::Speed => &[
            "fast", "quick", "speed", "urgent", "immediate", "instant",
            "short", "brief", "rapid",
        ],
        DimensionType::Context => &[
            "long", "context", "document", "history", "conversation",
            "previously", "remember", "chapter",
        ],
        DimensionType::Safety => &[
            "safe", "security", "harm", "danger", "toxic", "filter",
            "abuse", "policy", "guideline", "ethical",
        ],
    };

    let mut count = 0;
    for kw in keywords {
        if lower.contains(kw) {
            count += 1;
        }
    }

    (count as f32 / 5.0).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_keyword_scoring_reasoning() {
        let prompt = "Explain why this logic leads to a conclusion";
        let score = keyword_score(prompt, &DimensionType::Reasoning);
        assert!(score > 0.0, "reasoning prompt should score > 0");
    }

    #[tokio::test]
    async fn test_keyword_scoring_code() {
        let prompt = "Write a rust function to sort an array";
        let score = keyword_score(prompt, &DimensionType::Code);
        assert!(score > 0.0, "code prompt should score > 0");
    }

    #[tokio::test]
    async fn test_classifier_stub_works() {
        let engine = ClassifierEngine::new("/dev/null").expect("stub should not fail");
        let result = engine
            .classify("Write a poem about AI")
            .await
            .expect("classification should succeed");

        assert!(!result.scores.is_empty());
        assert!(result.confidence >= 0.0);
        assert!(result.confidence <= 1.0);
    }
}
