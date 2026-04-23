//! Deterministic mock embedding provider.
//!
//! Hashes tokens into fixed-dimension buckets. Same text → same vector,
//! different text → near-orthogonal vectors. Sufficient for cosine-based
//! tests and for driving the rest of the pipeline without external services.

use crate::traits::embedding::{EmbeddingProvider, EmbeddingVector};
use async_trait::async_trait;

pub struct MockEmbedding {
    pub dimension: usize,
}

impl MockEmbedding {
    pub fn new(dimension: usize) -> Self {
        assert!(dimension > 0, "MockEmbedding dimension must be positive");
        Self { dimension }
    }
}

impl Default for MockEmbedding {
    fn default() -> Self {
        Self::new(64)
    }
}

#[async_trait]
impl EmbeddingProvider for MockEmbedding {
    async fn embed(&self, text: &str) -> anyhow::Result<EmbeddingVector> {
        let mut v = vec![0f32; self.dimension];
        for word in text
            .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .filter(|s| !s.is_empty())
        {
            let h = fnv1a(word);
            let idx = (h as usize) % self.dimension;
            v[idx] += 1.0;
        }
        // L2 normalize so cosine ∈ [-1, 1] regardless of token count.
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        Ok(EmbeddingVector::new(v))
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn same_text_same_vector() {
        let e = MockEmbedding::new(32);
        let a = e.embed("hello world").await.unwrap();
        let b = e.embed("hello world").await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn different_text_different_vector() {
        let e = MockEmbedding::new(32);
        let a = e.embed("apple").await.unwrap();
        let b = e.embed("xenon").await.unwrap();
        assert_ne!(a, b);
        // Not strictly required, but cosine should be below 1.0.
        assert!(a.cosine(&b) < 1.0);
    }
}
