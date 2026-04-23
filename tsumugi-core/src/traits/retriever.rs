//! Retriever: query string → ranked ChunkId hits.

use crate::domain::ChunkId;
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq)]
pub struct RetrievalHit {
    pub chunk_id: ChunkId,
    pub score: f32,
}

#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalHit>>;
}
