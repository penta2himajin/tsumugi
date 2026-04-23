//! EmbeddingSimilarityDetector — Tier 1 semantic matcher. Emits events
//! when a chunk's embedding is within `threshold` cosine similarity of any
//! reference embedding supplied for a label.

use super::keyword::DetectedEvent;
use crate::domain::Chunk;
use crate::traits::detector::EventDetector;
use crate::traits::embedding::{EmbeddingProvider, EmbeddingVector};
use async_trait::async_trait;
use std::sync::Arc;

pub struct EmbeddingSimilarityDetector {
    provider: Arc<dyn EmbeddingProvider>,
    /// `(label, reference_embedding)` tuples.
    references: Vec<(String, EmbeddingVector)>,
    threshold: f32,
}

impl EmbeddingSimilarityDetector {
    pub fn new(provider: Arc<dyn EmbeddingProvider>, threshold: f32) -> Self {
        Self {
            provider,
            references: vec![],
            threshold,
        }
    }

    pub fn with_reference(mut self, label: impl Into<String>, embedding: EmbeddingVector) -> Self {
        self.references.push((label.into(), embedding));
        self
    }
}

#[async_trait]
impl EventDetector for EmbeddingSimilarityDetector {
    type Event = DetectedEvent;

    async fn detect(
        &self,
        chunk: &Chunk,
        _new_turn: &serde_json::Value,
    ) -> anyhow::Result<Vec<Self::Event>> {
        let embed = self.provider.embed(&chunk.text).await?;
        let mut out = Vec::new();
        for (label, reference) in &self.references {
            let sim = embed.cosine(reference);
            if sim >= self.threshold {
                out.push(DetectedEvent {
                    label: label.clone(),
                    matched_keyword: format!("similarity={sim:.3}"),
                });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockEmbedding;
    use serde_json::json;

    #[tokio::test]
    async fn detects_self_similarity() {
        let provider: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedding::new(32));
        let reference = provider.embed("dragon fire attack").await.unwrap();
        let d = EmbeddingSimilarityDetector::new(provider.clone(), 0.9)
            .with_reference("combat", reference);
        let chunk = Chunk::raw_leaf("dragon fire attack");
        let events = d.detect(&chunk, &json!({})).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].label, "combat");
    }

    #[tokio::test]
    async fn skips_dissimilar_text() {
        let provider: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbedding::new(32));
        let reference = provider.embed("dragon fire attack").await.unwrap();
        let d = EmbeddingSimilarityDetector::new(provider.clone(), 0.9)
            .with_reference("combat", reference);
        let chunk = Chunk::raw_leaf("peaceful tea ceremony");
        let events = d.detect(&chunk, &json!({})).await.unwrap();
        assert!(events.is_empty());
    }
}
