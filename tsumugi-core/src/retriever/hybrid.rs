//! HybridRetriever — weighted score-level fusion of BM25 + cosine.

use crate::traits::retriever::{RetrievalHit, Retriever};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

pub struct HybridRetriever {
    keyword: Arc<dyn Retriever>,
    vector: Arc<dyn Retriever>,
    /// Weight applied to keyword scores. 0.5 is a common starting point.
    pub keyword_weight: f32,
    /// Weight applied to vector scores.
    pub vector_weight: f32,
}

impl HybridRetriever {
    pub fn new(keyword: Arc<dyn Retriever>, vector: Arc<dyn Retriever>) -> Self {
        Self {
            keyword,
            vector,
            keyword_weight: 0.5,
            vector_weight: 0.5,
        }
    }

    pub fn with_weights(mut self, keyword: f32, vector: f32) -> Self {
        self.keyword_weight = keyword;
        self.vector_weight = vector;
        self
    }
}

#[async_trait]
impl Retriever for HybridRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalHit>> {
        // Retrieve wider than top_k from each leg so fusion sees enough
        // candidates. Simple multiplier; Phase 2 can tune.
        let leg_k = top_k * 4;
        let kw = self.keyword.retrieve(query, leg_k).await?;
        let vec = self.vector.retrieve(query, leg_k).await?;

        // Normalize each leg's scores to [0, 1] then combine.
        let kw_max = kw.iter().map(|h| h.score).fold(0f32, f32::max).max(1e-6);
        let vec_max = vec.iter().map(|h| h.score).fold(0f32, f32::max).max(1e-6);

        let mut merged: HashMap<_, f32> = HashMap::new();
        for hit in &kw {
            let norm = hit.score / kw_max;
            *merged.entry(hit.chunk_id).or_default() += self.keyword_weight * norm;
        }
        for hit in &vec {
            let norm = hit.score / vec_max;
            *merged.entry(hit.chunk_id).or_default() += self.vector_weight * norm;
        }

        let mut hits: Vec<RetrievalHit> = merged
            .into_iter()
            .map(|(chunk_id, score)| RetrievalHit { chunk_id, score })
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
    use crate::domain::ChunkId;
    use crate::retriever::Bm25Retriever;
    use crate::traits::embedding::{EmbeddingProvider, EmbeddingVector};
    use async_trait::async_trait;

    struct FixedEmbedding(Vec<f32>);

    #[async_trait]
    impl EmbeddingProvider for FixedEmbedding {
        async fn embed(&self, _: &str) -> anyhow::Result<EmbeddingVector> {
            Ok(EmbeddingVector::new(self.0.clone()))
        }
        fn dimension(&self) -> usize {
            self.0.len()
        }
    }

    #[tokio::test]
    async fn hybrid_combines_both_legs() {
        use crate::retriever::CosineRetriever;
        let a = ChunkId::new();
        let b = ChunkId::new();
        let bm25 = Arc::new(Bm25Retriever::new(vec![
            (a, "quick brown fox".to_string()),
            (b, "lazy dog".to_string()),
        ]));
        let provider: Arc<dyn EmbeddingProvider> = Arc::new(FixedEmbedding(vec![1.0, 0.0, 0.0]));
        let cos = Arc::new(CosineRetriever::new(
            vec![
                (a, EmbeddingVector::new(vec![0.9, 0.1, 0.0])),
                (b, EmbeddingVector::new(vec![0.1, 0.9, 0.0])),
            ],
            provider.clone(),
        ));
        let hybrid = HybridRetriever::new(bm25, cos);
        let hits = hybrid.retrieve("quick brown fox", 10).await.unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].chunk_id, a);
    }
}
