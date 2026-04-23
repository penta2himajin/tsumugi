//! ChapterOrderScorer — linear decay on the chapter-order distance between
//! the current chunk's `order_in_parent` and the candidate chunk's.

use crate::domain::Chunk;
use crate::traits::scorer::{RelevanceScorer, ScoringContext};

pub struct ChapterOrderScorer {
    pub decay_per_step: f32,
}

impl ChapterOrderScorer {
    pub fn new(decay_per_step: f32) -> Self {
        Self {
            decay_per_step: decay_per_step.clamp(0.0, 1.0),
        }
    }
}

impl RelevanceScorer for ChapterOrderScorer {
    fn score(&self, chunk: &Chunk, ctx: &ScoringContext<'_>) -> f32 {
        let base = ctx.retrieval_hit.map_or(1.0, |h| h.score);
        let distance = ctx
            .current_order
            .map(|cur| (cur - chunk.order_in_parent).unsigned_abs() as f32)
            .unwrap_or(0.0);
        (base * (1.0 - self.decay_per_step * distance)).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn same_order_is_base() {
        let mut c = Chunk::raw_leaf("x");
        c.order_in_parent = 5;
        let ctx = ScoringContext {
            current_order: Some(5),
            ..ScoringContext::new(Utc::now())
        };
        assert!((ChapterOrderScorer::new(0.1).score(&c, &ctx) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn distance_decays_linearly() {
        let mut c = Chunk::raw_leaf("x");
        c.order_in_parent = 3;
        let ctx = ScoringContext {
            current_order: Some(5),
            ..ScoringContext::new(Utc::now())
        };
        let s = ChapterOrderScorer::new(0.25).score(&c, &ctx);
        assert!((s - 0.5).abs() < 1e-6, "got {s}");
    }

    #[test]
    fn score_floors_at_zero() {
        let mut c = Chunk::raw_leaf("x");
        c.order_in_parent = 0;
        let ctx = ScoringContext {
            current_order: Some(100),
            ..ScoringContext::new(Utc::now())
        };
        let s = ChapterOrderScorer::new(0.5).score(&c, &ctx);
        assert_eq!(s, 0.0);
    }
}
