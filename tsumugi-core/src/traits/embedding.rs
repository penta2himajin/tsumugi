//! EmbeddingProvider: text → vector.

use async_trait::async_trait;

/// Dense vector representation produced by an embedding model.
///
/// Dimension is implementation-dependent. Callers should verify
/// `len()` matches the expected dimension before doing arithmetic
/// across embeddings produced by different providers.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddingVector(pub Vec<f32>);

impl EmbeddingVector {
    pub fn new(values: Vec<f32>) -> Self {
        Self(values)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.0
    }

    /// Cosine similarity in `[-1.0, 1.0]`. Returns 0.0 when either side is
    /// the zero vector or dimensions mismatch.
    pub fn cosine(&self, other: &Self) -> f32 {
        if self.len() != other.len() || self.is_empty() {
            return 0.0;
        }
        let mut dot = 0.0f32;
        let mut na = 0.0f32;
        let mut nb = 0.0f32;
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            dot += a * b;
            na += a * a;
            nb += b * b;
        }
        if na == 0.0 || nb == 0.0 {
            return 0.0;
        }
        dot / (na.sqrt() * nb.sqrt())
    }
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> anyhow::Result<EmbeddingVector>;

    async fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<EmbeddingVector>> {
        let mut out = Vec::with_capacity(texts.len());
        for t in texts {
            out.push(self.embed(t).await?);
        }
        Ok(out)
    }

    fn dimension(&self) -> usize;
}
