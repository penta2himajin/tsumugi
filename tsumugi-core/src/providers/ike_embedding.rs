//! IkeEmbedding — binarized embedding wrapper (survey §4.3).
//!
//! Wraps an underlying `EmbeddingProvider` and reduces each dimension to
//! `+1.0` / `-1.0` (or `0.0` when the underlying value is exactly zero).
//! Retrieval over binarized embeddings reduces to a Hamming-like score and
//! trades some accuracy for memory / compute wins — real IKE systems go
//! further and pack bits into `u64` words, which is a Phase 4 optimization.
//! Here we preserve the `EmbeddingVector<Vec<f32>>` shape so consumers can
//! keep using `EmbeddingVector::cosine` (which becomes a scaled Hamming
//! similarity on ±1 vectors).

use crate::traits::embedding::{EmbeddingProvider, EmbeddingVector};
use async_trait::async_trait;
use std::sync::Arc;

pub struct IkeEmbedding {
    inner: Arc<dyn EmbeddingProvider>,
}

impl IkeEmbedding {
    pub fn new(inner: Arc<dyn EmbeddingProvider>) -> Self {
        Self { inner }
    }

    fn binarize(vec: EmbeddingVector) -> EmbeddingVector {
        let EmbeddingVector(values) = vec;
        let binarized: Vec<f32> = values
            .into_iter()
            .map(|v| {
                if v > 0.0 {
                    1.0
                } else if v < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            })
            .collect();
        EmbeddingVector::new(binarized)
    }
}

#[async_trait]
impl EmbeddingProvider for IkeEmbedding {
    async fn embed(&self, text: &str) -> anyhow::Result<EmbeddingVector> {
        let raw = self.inner.embed(text).await?;
        Ok(Self::binarize(raw))
    }

    async fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<EmbeddingVector>> {
        let raw = self.inner.embed_batch(texts).await?;
        Ok(raw.into_iter().map(Self::binarize).collect())
    }

    fn dimension(&self) -> usize {
        self.inner.dimension()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockEmbedding;

    #[tokio::test]
    async fn binarizes_to_plus_minus_one() {
        let inner: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedding::new(16));
        let ike = IkeEmbedding::new(inner);
        let v = ike.embed("hello world").await.unwrap();
        for value in v.as_slice() {
            assert!(
                *value == -1.0 || *value == 0.0 || *value == 1.0,
                "expected ±1 / 0, got {value}"
            );
        }
        assert_eq!(v.len(), 16);
    }

    #[tokio::test]
    async fn preserves_dimension_across_inner() {
        for dim in [8usize, 64, 256] {
            let inner: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedding::new(dim));
            let ike = IkeEmbedding::new(inner);
            assert_eq!(ike.dimension(), dim);
            assert_eq!(ike.embed("abc").await.unwrap().len(), dim);
        }
    }

    #[tokio::test]
    async fn same_text_same_binarization() {
        let inner: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedding::new(32));
        let ike = IkeEmbedding::new(inner);
        let a = ike.embed("alice").await.unwrap();
        let b = ike.embed("alice").await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn cosine_on_binarized_is_in_range() {
        let inner: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedding::new(64));
        let ike = IkeEmbedding::new(inner);
        let a = ike.embed("alice met bob at dawn").await.unwrap();
        let b = ike.embed("alice met bob at dawn").await.unwrap();
        let c = ike.embed("completely different sentence").await.unwrap();
        let self_sim = a.cosine(&b);
        let cross = a.cosine(&c);
        assert!(
            (self_sim - 1.0).abs() < 1e-6,
            "self-sim should be 1.0, got {self_sim}"
        );
        assert!(cross.abs() <= 1.0 + 1e-6);
        assert!(cross <= self_sim);
    }

    #[tokio::test]
    async fn batch_binarizes_every_output() {
        let inner: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedding::new(32));
        let ike = IkeEmbedding::new(inner);
        let texts = vec!["foo".to_string(), "bar".to_string(), "baz".to_string()];
        let out = ike.embed_batch(&texts).await.unwrap();
        assert_eq!(out.len(), 3);
        for v in &out {
            assert_eq!(v.len(), 32);
            for value in v.as_slice() {
                assert!(*value == -1.0 || *value == 0.0 || *value == 1.0);
            }
        }
    }
}
