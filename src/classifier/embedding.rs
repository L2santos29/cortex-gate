//! Embedding utilities: cosine similarity, L2 normalization, mean pooling,
//! and a basic WordPiece tokenizer for BGE (bge-small-en-v1.5).
//!
//! The tokenizer uses the BERT uncased vocabulary included via `include_str!`
//! at compile time. It performs lowercasing, basic punctuation splitting, and
//! WordPiece subword segmentation with a max length of 512 tokens.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// BERT special token IDs.
pub const TOKEN_PAD: i64 = 0;
pub const TOKEN_UNK: i64 = 100;
pub const TOKEN_CLS: i64 = 101;
pub const TOKEN_SEP: i64 = 102;
pub const TOKEN_MASK: i64 = 103;

/// Maximum sequence length for BGE models.
pub const MAX_SEQ_LEN: usize = 512;

/// BGE-small embedding dimensionality.
pub const EMBEDDING_DIM: usize = 384;

// ---------------------------------------------------------------------------
// Cosine similarity
// ---------------------------------------------------------------------------

/// Compute the cosine similarity between two slices.
///
/// Both slices must be non-empty and of equal length; otherwise returns 0.0.
/// The function does **not** assume the inputs are already normalized — it
/// computes the norm of each vector on the fly.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

// ---------------------------------------------------------------------------
// L2 normalization (in-place)
// ---------------------------------------------------------------------------

/// Normalize a vector in-place so its L2 norm equals 1.0.
///
/// If the vector is all zeros it is left unchanged.
pub fn normalize(vec: &mut [f32]) {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }
}

// ---------------------------------------------------------------------------
// Mean pooling
// ---------------------------------------------------------------------------

/// Apply mean pooling over the sequence dimension of a `[seq_len, dim]` matrix.
///
/// `embeddings` is a slice of vectors, one per token position. Returns a single
/// pooled vector of length `dim`. If the input is empty, returns an empty vector.
///
/// This is the pooling strategy recommended for BGE models (instead of CLS).
pub fn mean_pooling(embeddings: &[Vec<f32>]) -> Vec<f32> {
    if embeddings.is_empty() {
        return Vec::new();
    }

    let dim = embeddings[0].len();
    let seq_len = embeddings.len();
    let mut pooled = vec![0.0_f32; dim];

    for token_emb in embeddings.iter() {
        for (j, val) in token_emb.iter().enumerate().take(dim) {
            pooled[j] += val;
        }
    }

    let scale = 1.0 / seq_len as f32;
    for val in pooled.iter_mut() {
        *val *= scale;
    }

    pooled
}

// ---------------------------------------------------------------------------
// Basic WordPiece tokenizer for BGE / BERT-uncased
// ---------------------------------------------------------------------------

/// A thread-safe singleton holding the parsed vocabulary.
///
/// Initialized once on first access.
struct Vocab {
    /// Maps token string → ID (e.g. "the" → 1996).
    token_to_id: HashMap<String, i64>,
}

impl Vocab {
    fn new() -> Self {
        let vocab_str = include_str!("vocab.txt");
        let mut token_to_id = HashMap::with_capacity(30_600);

        for (idx, line) in vocab_str.lines().enumerate() {
            let token = line.trim();
            if !token.is_empty() {
                token_to_id.insert(token.to_string(), idx as i64);
            }
        }

        Self { token_to_id }
    }
}

/// Lazy-initialized global vocabulary.
fn vocab() -> &'static Vocab {
    use std::sync::LazyLock;
    static VOCAB: LazyLock<Vocab> = LazyLock::new(Vocab::new);
    &VOCAB
}

/// Tokenize a text string into BERT-compatible token IDs.
///
/// Steps:
/// 1. Lowercase the input.
/// 2. Split on whitespace and punctuation (basic pre-tokenization).
/// 3. For each word, apply WordPiece subword segmentation.
/// 4. Wrap with `[CLS]` at position 0 and `[SEP]` at the end.
/// 5. Truncate to `MAX_SEQ_LEN` tokens (512).
///
/// Returns a `Vec<i64>` of token IDs ready for ONNX inference.
pub fn tokenize(text: &str) -> Vec<i64> {
    let v = vocab();

    // --- 1. Lowercase & normalise whitespace ---
    let text_lower = text.to_lowercase();

    // --- 2. Pre-tokenize: split on whitespace & punctuation ---
    let words = basic_tokenize(&text_lower);

    // --- 3 & 4. WordPiece + [CLS] / [SEP] wrapping ---
    let mut tokens: Vec<i64> = Vec::with_capacity(MAX_SEQ_LEN);
    tokens.push(TOKEN_CLS);

    for word in &words {
        if tokens.len() >= MAX_SEQ_LEN - 1 {
            break; // reserve room for [SEP]
        }
        let ids = wordpiece(v, word);
        for id in ids {
            if tokens.len() >= MAX_SEQ_LEN - 1 {
                break;
            }
            tokens.push(id);
        }
    }

    tokens.push(TOKEN_SEP);

    debug_assert!(
        tokens.len() <= MAX_SEQ_LEN,
        "tokenizer exceeded max sequence length"
    );

    tokens
}

/// Very basic pre-tokenizer: splits on whitespace and punctuation boundaries.
///
/// BERT's BasicTokenizer lowercases, strips accents, and splits on punctuation.
/// This is a simplified approximation.
fn basic_tokenize(text: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
        } else if is_punctuation(ch) {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
            words.push(ch.to_string());
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }

    words
}

/// Returns `true` for ASCII and common Unicode punctuation characters.
fn is_punctuation(c: char) -> bool {
    // ASCII punctuation
    if (c as u32) >= 33 && (c as u32) <= 47 {
        return true;
    }
    if (c as u32) >= 58 && (c as u32) <= 64 {
        return true;
    }
    if (c as u32) >= 91 && (c as u32) <= 96 {
        return true;
    }
    if (c as u32) >= 123 && (c as u32) <= 126 {
        return true;
    }
    // Treat CJK characters as individual tokens
    if is_cjk(c) {
        return true;
    }
    false
}

/// Check if a character is in the CJK (Chinese-Japanese-Korean) range.
fn is_cjk(c: char) -> bool {
    let cp = c as u32;
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0x2E80..=0x2EFF).contains(&cp)
}

/// WordPiece subword segmentation for a single word.
///
/// Tries to find the longest prefix of `word` in the vocabulary. If found,
/// appends its ID and recurses on the remainder (prepended with `##`).
/// Falls back to `[UNK]` if no subword can be matched.
fn wordpiece(vocab: &Vocab, word: &str) -> Vec<i64> {
    // Direct lookup first (fast path for whole-word tokens).
    if let Some(&id) = vocab.token_to_id.get(word) {
        return vec![id];
    }

    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();

    // WordPiece algorithm: greedily match the longest prefix
    let mut tokens: Vec<i64> = Vec::new();
    let mut start = 0;

    while start < len {
        let mut matched = false;
        // Try longest prefix first
        for end in (start + 1..=len).rev() {
            let candidate: String = if start == 0 {
                chars[start..end].iter().collect()
            } else {
                let mut s = String::from("##");
                s.extend(chars[start..end].iter());
                s
            };

            if let Some(&id) = vocab.token_to_id.get(&candidate) {
                tokens.push(id);
                start = end;
                matched = true;
                break;
            }
        }

        if !matched {
            // No subword found → fall back to [UNK] for this character
            tokens.push(TOKEN_UNK);
            start += 1;
        }
    }

    tokens
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6, "expected 1.0, got {sim}");
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_mismatched_length() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_normalize() {
        let mut v = vec![3.0, 4.0];
        normalize(&mut v);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_zero() {
        let mut v = vec![0.0, 0.0];
        normalize(&mut v);
        assert_eq!(v, vec![0.0, 0.0]);
    }

    #[test]
    fn test_mean_pooling() {
        let batch = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
            vec![5.0, 6.0],
        ];
        let pooled = mean_pooling(&batch);
        assert!((pooled[0] - 3.0).abs() < 1e-6);
        assert!((pooled[1] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_mean_pooling_empty() {
        assert!(mean_pooling(&[]).is_empty());
    }

    #[test]
    fn test_tokenize_adds_cls_and_sep() {
        let ids = tokenize("hello");
        assert_eq!(ids[0], TOKEN_CLS, "first token must be [CLS]");
        assert_eq!(ids[ids.len() - 1], TOKEN_SEP, "last token must be [SEP]");
    }

    #[test]
    fn test_tokenize_max_length() {
        let long_text = "word ".repeat(600);
        let ids = tokenize(&long_text);
        assert!(ids.len() <= MAX_SEQ_LEN);
    }

    #[test]
    fn test_tokenize_known_word() {
        // "the" should be in the BERT vocab
        let ids = tokenize("the");
        // We expect [CLS] the [SEP]
        assert_eq!(ids.len(), 3);
        // ID for "the" in BERT uncased vocab is 1996
        assert_eq!(ids[1], 1996, "expected 'the' → 1996");
    }

    #[test]
    fn test_cjk_tokenization() {
        let ids = tokenize("你好");
        assert!(ids.len() >= 3, "CJK chars should produce separate tokens");
        assert_eq!(ids[0], TOKEN_CLS);
        assert_eq!(ids[ids.len() - 1], TOKEN_SEP);
    }
}
