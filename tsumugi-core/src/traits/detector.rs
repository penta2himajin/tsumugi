//! EventDetector: domain-event detection from incoming chunks / turns.
//!
//! Used in 4-tier cascades (keyword → embedding similarity → LLM classifier).

use crate::domain::Chunk;
use async_trait::async_trait;

#[async_trait]
pub trait EventDetector: Send + Sync {
    type Event: Send + Sync;

    async fn detect(
        &self,
        chunk: &Chunk,
        new_turn: &serde_json::Value,
    ) -> anyhow::Result<Vec<Self::Event>>;
}
