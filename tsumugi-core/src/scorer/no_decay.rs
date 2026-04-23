//! NoDecayScorer — passes the retrieval score through untouched (1.0 when
//! no retrieval hit is supplied). Used as a neutral element in
//! `CompositeScorer` or when time/location signals are not applicable.

use crate::domain::Chunk;
use crate::traits::scorer::{RelevanceScorer, ScoringContext};

pub struct NoDecayScorer;

impl RelevanceScorer for NoDecayScorer {
    fn score(&self, _chunk: &Chunk, ctx: &ScoringContext<'_>) -> f32 {
        ctx.retrieval_hit.map_or(1.0, |h| h.score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::retriever::RetrievalHit;
    use chrono::Utc;

    #[test]
    fn returns_retrieval_score_when_present() {
        let chunk = Chunk::raw_leaf("x");
        let hit = RetrievalHit {
            chunk_id: chunk.id,
            score: 0.42,
        };
        let ctx = ScoringContext {
            retrieval_hit: Some(&hit),
            ..ScoringContext::new(Utc::now())
        };
        assert_eq!(NoDecayScorer.score(&chunk, &ctx), 0.42);
    }

    #[test]
    fn defaults_to_one_without_retrieval() {
        let chunk = Chunk::raw_leaf("x");
        let ctx = ScoringContext::new(Utc::now());
        assert_eq!(NoDecayScorer.score(&chunk, &ctx), 1.0);
    }
}
