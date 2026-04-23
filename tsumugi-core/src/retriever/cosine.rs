//! Cosine-similarity retriever over pre-computed embeddings.
//!
//! Given a corpus of `(ChunkId, EmbeddingVector)` pairs and a query
//! `EmbeddingVector`, returns the top-k chunks by cosine similarity.
//! Supplying the query embedding is the caller's responsibility — this
//! retriever deliberately does not take an `EmbeddingProvider` so it
//! composes cleanly with caching and batching.

use crate::domain::ChunkId;
use crate::traits::embedding::{EmbeddingProvider, EmbeddingVector};
use crate::traits::retriever::{RetrievalHit, Retriever};
use async_trait::async_trait;
use std::sync::Arc;

pub struct CosineRetriever {
    embeddings: Vec<(ChunkId, EmbeddingVector)>,
    provider: Arc<dyn EmbeddingProvider>,
}

impl CosineRetriever {
    pub fn new(
        embeddings: Vec<(ChunkId, EmbeddingVector)>,
        provider: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            embeddings,
            provider,
        }
    }
}

#[async_trait]
impl Retriever for CosineRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalHit>> {
        let q = self.provider.embed(query).await?;
        let mut hits: Vec<RetrievalHit> = self
            .embeddings
            .iter()
            .map(|(id, v)| RetrievalHit {
                chunk_id: *id,
                score: q.cosine(v),
            })
            .filter(|h| h.score.is_finite() && h.score > 0.0)
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(top_k);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct IdentityEmbedding;

    #[async_trait]
    impl EmbeddingProvider for IdentityEmbedding {
        async fn embed(&self, text: &str) -> anyhow::Result<EmbeddingVector> {
            // Bag-of-chars embedding — byte buckets 0..32
            let mut v = vec![0f32; 32];
            for (i, b) in text.bytes().enumerate() {
                v[i % 32] += (b as f32) / 255.0;
            }
            Ok(EmbeddingVector::new(v))
        }
        fn dimension(&self) -> usize {
            32
        }
    }

    #[tokio::test]
    async fn cosine_prefers_matching_text() {
        let provider = Arc::new(IdentityEmbedding);
        let a = ChunkId::new();
        let b = ChunkId::new();
        let ea = provider.embed("quick brown fox").await.unwrap();
        let eb = provider
            .embed("something entirely different")
            .await
            .unwrap();
        let retriever = CosineRetriever::new(vec![(a, ea), (b, eb)], provider.clone());
        let hits = retriever.retrieve("quick brown fox", 10).await.unwrap();
        assert_eq!(hits.first().map(|h| h.chunk_id), Some(a));
    }
}
