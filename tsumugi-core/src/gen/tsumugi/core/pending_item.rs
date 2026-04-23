use super::Chunk;
use super::Priority;

/// Invariant: PendingItemIntroducerLink
/// Invariant: PendingItemResolvedAtAfterIntroduction
/// Invariant: PendingItemExpectedResolutionAfterIntroduction
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingItem {
    pub introduced_at: Box<Chunk>,
    pub expected_resolution_chunk: Option<Box<Chunk>>,
    pub resolved_at: Option<Box<Chunk>>,
    pub priority: Priority,
}
