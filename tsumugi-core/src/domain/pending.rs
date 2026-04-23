//! PendingItem: open plot threads, TODOs, unresolved clues.
//!
//! Mirrors `models/tsumugi/core.als` (`PendingItem`, `Priority`).
//!
//! Lifecycle invariant (enforced structurally in Alloy via `happens_before`
//! and runtime-checkable here): `introduced_at` must not happen after
//! `resolved_at` or `expected_resolution_chunk`.

use super::ids::{ChunkId, PendingItemId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingItem {
    pub id: PendingItemId,
    /// Free-form tag ("plot" / "clue" / "todo" / "refactor" / …).
    pub kind: String,
    pub description: String,
    pub introduced_at: ChunkId,
    pub expected_resolution_chunk: Option<ChunkId>,
    pub resolved_at: Option<ChunkId>,
    pub priority: Priority,
}

impl PendingItem {
    pub fn new(
        kind: impl Into<String>,
        description: impl Into<String>,
        introduced_at: ChunkId,
        priority: Priority,
    ) -> Self {
        Self {
            id: PendingItemId::new(),
            kind: kind.into(),
            description: description.into(),
            introduced_at,
            expected_resolution_chunk: None,
            resolved_at: None,
            priority,
        }
    }

    pub fn is_resolved(&self) -> bool {
        self.resolved_at.is_some()
    }

    pub fn resolve(&mut self, resolution_chunk: ChunkId) {
        self.resolved_at = Some(resolution_chunk);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_pending_item_is_unresolved() {
        let p = PendingItem::new("plot", "Find the key", ChunkId::new(), Priority::High);
        assert!(!p.is_resolved());
    }

    #[test]
    fn resolve_sets_resolved_at() {
        let mut p = PendingItem::new("plot", "Find the key", ChunkId::new(), Priority::High);
        let resolution = ChunkId::new();
        p.resolve(resolution);
        assert_eq!(p.resolved_at, Some(resolution));
        assert!(p.is_resolved());
    }
}
