//! RelevanceScorer: reorder / filter retrieved Chunks by arbitrary signals.

use crate::domain::{Chunk, ChunkId, SourceLocationValue};
use crate::traits::retriever::RetrievalHit;
use chrono::{DateTime, Utc};

/// Context passed to each scorer invocation. All fields are optional so
/// domain-specific scorers only pay for what they use.
#[derive(Clone, Debug)]
pub struct ScoringContext<'a> {
    pub current_chunk_id: Option<ChunkId>,
    pub current_time: DateTime<Utc>,
    pub current_order: Option<i64>,
    pub current_location: Option<&'a SourceLocationValue>,
    pub retrieval_hit: Option<&'a RetrievalHit>,
}

impl<'a> ScoringContext<'a> {
    pub fn new(current_time: DateTime<Utc>) -> Self {
        Self {
            current_chunk_id: None,
            current_time,
            current_order: None,
            current_location: None,
            retrieval_hit: None,
        }
    }
}

pub trait RelevanceScorer: Send + Sync {
    fn score(&self, chunk: &Chunk, ctx: &ScoringContext<'_>) -> f32;
}
