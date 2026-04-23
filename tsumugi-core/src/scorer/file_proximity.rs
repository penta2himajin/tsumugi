//! FileProximityScorer — uses `SourceLocation::proximity` between the
//! current chunk's source and each candidate's source.

use crate::domain::{Chunk, SourceLocation};
use crate::traits::scorer::{RelevanceScorer, ScoringContext};

pub struct FileProximityScorer {
    /// Weight applied to the proximity component (0.0 keeps only retrieval
    /// score; 1.0 multiplies retrieval by proximity).
    pub proximity_weight: f32,
}

impl FileProximityScorer {
    pub fn new(proximity_weight: f32) -> Self {
        Self {
            proximity_weight: proximity_weight.clamp(0.0, 1.0),
        }
    }
}

impl Default for FileProximityScorer {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl RelevanceScorer for FileProximityScorer {
    fn score(&self, chunk: &Chunk, ctx: &ScoringContext<'_>) -> f32 {
        let base = ctx.retrieval_hit.map_or(1.0, |h| h.score);
        let Some(current) = ctx.current_location else {
            return base;
        };
        let Some(candidate) = chunk.source_location.as_ref() else {
            return base * (1.0 - self.proximity_weight);
        };
        let prox = candidate.proximity(current);
        // Interpolate between `base * (1-w)` and `base * 1.0` by proximity.
        base * ((1.0 - self.proximity_weight) + self.proximity_weight * prox)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SourceLocationValue;
    use chrono::Utc;

    #[test]
    fn higher_proximity_scores_higher() {
        let near = SourceLocationValue::file("src/a/near.rs");
        let far = SourceLocationValue::file("docs/readme.md");
        let current = SourceLocationValue::file("src/a/current.rs");

        let mut c_near = Chunk::raw_leaf("near");
        c_near.source_location = Some(near);
        let mut c_far = Chunk::raw_leaf("far");
        c_far.source_location = Some(far);

        let ctx = ScoringContext {
            current_location: Some(&current),
            ..ScoringContext::new(Utc::now())
        };
        let scorer = FileProximityScorer::new(1.0);
        let s_near = scorer.score(&c_near, &ctx);
        let s_far = scorer.score(&c_far, &ctx);
        assert!(s_near > s_far, "near={s_near}, far={s_far}");
    }
}
