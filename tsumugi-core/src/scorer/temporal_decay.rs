//! TemporalDecayScorer — exponential decay on the age of a Chunk.
//!
//! `score = base * exp(-ln(2) * age / half_life)`. The base is the retrieval
//! score if supplied, else 1.0.

use crate::domain::Chunk;
use crate::traits::scorer::{RelevanceScorer, ScoringContext};
use chrono::Duration;

pub struct TemporalDecayScorer {
    pub half_life: Duration,
}

impl TemporalDecayScorer {
    pub fn new(half_life: Duration) -> Self {
        Self { half_life }
    }
}

impl RelevanceScorer for TemporalDecayScorer {
    fn score(&self, chunk: &Chunk, ctx: &ScoringContext<'_>) -> f32 {
        let age = ctx
            .current_time
            .signed_duration_since(chunk.last_active_at)
            .num_seconds()
            .max(0) as f32;
        let half_life_secs = self.half_life.num_seconds().max(1) as f32;
        let decay = (-std::f32::consts::LN_2 * age / half_life_secs).exp();
        let base = ctx.retrieval_hit.map_or(1.0, |h| h.score);
        base * decay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn fresh_chunk_scores_near_base() {
        let mut chunk = Chunk::raw_leaf("fresh");
        let now = Utc::now();
        chunk.last_active_at = now;
        let ctx = ScoringContext::new(now);
        let s = TemporalDecayScorer::new(Duration::hours(1)).score(&chunk, &ctx);
        assert!((s - 1.0).abs() < 1e-6);
    }

    #[test]
    fn one_half_life_gives_half_score() {
        let mut chunk = Chunk::raw_leaf("old");
        let now = Utc::now();
        chunk.last_active_at = now - Duration::hours(1);
        let ctx = ScoringContext::new(now);
        let s = TemporalDecayScorer::new(Duration::hours(1)).score(&chunk, &ctx);
        assert!((s - 0.5).abs() < 1e-3, "got {s}");
    }
}
