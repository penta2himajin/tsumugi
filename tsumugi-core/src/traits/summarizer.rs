//! Summarizer: produce a compact summary string for a Chunk.

use crate::domain::{Chunk, SummaryMethod};
use async_trait::async_trait;

#[async_trait]
pub trait Summarizer: Send + Sync {
    /// Produce a summary for the given chunk. Implementations should
    /// identify their method via `method()` so the Context Compiler can
    /// record it on the generated summary Chunk.
    async fn summarize(&self, chunk: &Chunk) -> anyhow::Result<String>;

    fn method(&self) -> SummaryMethod;
}
