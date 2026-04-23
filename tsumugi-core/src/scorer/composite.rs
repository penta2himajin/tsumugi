//! CompositeScorer — weighted average of child scorers.

use crate::domain::Chunk;
use crate::traits::scorer::{RelevanceScorer, ScoringContext};
use std::sync::Arc;

pub struct CompositeScorer {
    children: Vec<(Arc<dyn RelevanceScorer>, f32)>,
}

impl CompositeScorer {
    pub fn new() -> Self {
        Self { children: vec![] }
    }

    pub fn add(mut self, scorer: Arc<dyn RelevanceScorer>, weight: f32) -> Self {
        self.children.push((scorer, weight));
        self
    }
}

impl Default for CompositeScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl RelevanceScorer for CompositeScorer {
    fn score(&self, chunk: &Chunk, ctx: &ScoringContext<'_>) -> f32 {
        if self.children.is_empty() {
            return ctx.retrieval_hit.map_or(0.0, |h| h.score);
        }
        let total_weight: f32 = self.children.iter().map(|(_, w)| *w).sum();
        if total_weight <= 0.0 {
            return 0.0;
        }
        let mut acc = 0f32;
        for (s, w) in &self.children {
            acc += s.score(chunk, ctx) * w;
        }
        acc / total_weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorer::NoDecayScorer;
    use crate::traits::retriever::RetrievalHit;
    use chrono::Utc;

    #[test]
    fn empty_composite_returns_retrieval_score() {
        let c = Chunk::raw_leaf("x");
        let hit = RetrievalHit {
            chunk_id: c.id,
            score: 0.7,
        };
        let ctx = ScoringContext {
            retrieval_hit: Some(&hit),
            ..ScoringContext::new(Utc::now())
        };
        let cs = CompositeScorer::new();
        assert_eq!(cs.score(&c, &ctx), 0.7);
    }

    #[test]
    fn weighted_average_of_children() {
        let c = Chunk::raw_leaf("x");
        let hit = RetrievalHit {
            chunk_id: c.id,
            score: 0.4,
        };
        let ctx = ScoringContext {
            retrieval_hit: Some(&hit),
            ..ScoringContext::new(Utc::now())
        };
        let cs = CompositeScorer::new()
            .add(Arc::new(NoDecayScorer), 1.0)
            .add(Arc::new(NoDecayScorer), 3.0);
        // Average of 0.4 and 0.4 weighted (1,3) = 0.4
        assert!((cs.score(&c, &ctx) - 0.4).abs() < 1e-6);
    }
}
