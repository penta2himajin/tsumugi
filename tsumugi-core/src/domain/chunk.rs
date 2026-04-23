//! Chunk: hierarchical narrative unit.
//!
//! Mirrors `models/tsumugi/core.als` (`Chunk`) and adds the runtime-only
//! payload fields (strings, keywords, metadata, timestamps, boolean flags)
//! that are intentionally outside of Alloy's structural scope.

use super::ids::{ChunkId, FactId, PendingItemId};
use super::source_location::SourceLocationValue;
use super::summary::SummaryMethod;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A unique keyword surface form. Kept as a string newtype for clarity at
/// call sites; normalization is the product's responsibility.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Keyword(pub String);

impl From<&str> for Keyword {
    fn from(value: &str) -> Self {
        Keyword(value.to_string())
    }
}

impl From<String> for Keyword {
    fn from(value: String) -> Self {
        Keyword(value)
    }
}

impl std::fmt::Display for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Hierarchical narrative unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub id: ChunkId,

    /// Normalized display-facing text.
    pub text: String,

    /// Serialized domain events. Non-empty when `summary_level == 0`,
    /// empty when the chunk is a summary node (`summary_level > 0`).
    #[serde(default)]
    pub items: Vec<serde_json::Value>,

    #[serde(default)]
    pub summary: String,

    #[serde(default)]
    pub keywords: HashSet<Keyword>,

    #[serde(default)]
    pub facts: Vec<FactId>,

    #[serde(default)]
    pub pending: Vec<PendingItemId>,

    #[serde(default)]
    pub parent: Option<ChunkId>,

    #[serde(default)]
    pub children: Vec<ChunkId>,

    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,

    pub last_active_at: DateTime<Utc>,

    #[serde(default)]
    pub order_in_parent: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<SourceLocationValue>,

    /// 0 = raw leaf (items non-empty). Positive values indicate higher
    /// abstraction levels (children non-empty).
    #[serde(default)]
    pub summary_level: u32,

    #[serde(default)]
    pub summary_method: SummaryMethod,

    /// Runtime flag: set to true when a human has edited the summary.
    /// Used to guard against silent overwrite by the auto-summarizer.
    #[serde(default)]
    pub edited_by_user: bool,

    /// Runtime flag: when true, automated updates are blocked regardless of
    /// `edited_by_user`. Used for UX-pinned chunks.
    #[serde(default)]
    pub auto_update_locked: bool,
}

impl Chunk {
    /// Construct a new raw-leaf chunk with the given text. `items` and
    /// `summary_level == 0` correspond to Alloy's `RawLeafHasItems` and
    /// `RawLeafHasNoMethod` invariants.
    pub fn raw_leaf(text: impl Into<String>) -> Self {
        Self {
            id: ChunkId::new(),
            text: text.into(),
            items: vec![],
            summary: String::new(),
            keywords: HashSet::new(),
            facts: vec![],
            pending: vec![],
            parent: None,
            children: vec![],
            metadata: serde_json::Map::new(),
            last_active_at: Utc::now(),
            order_in_parent: 0,
            source_location: None,
            summary_level: 0,
            summary_method: SummaryMethod::None,
            edited_by_user: false,
            auto_update_locked: false,
        }
    }

    pub fn with_source(mut self, location: SourceLocationValue) -> Self {
        self.source_location = Some(location);
        self
    }

    pub fn is_raw_leaf(&self) -> bool {
        self.summary_level == 0
    }

    pub fn is_summary_node(&self) -> bool {
        self.summary_level > 0
    }

    /// Validate the core hierarchical-summary invariants (Alloy mirror).
    pub fn validate_summary_invariants(&self) -> Result<(), &'static str> {
        if self.is_raw_leaf() {
            if self.summary_method != SummaryMethod::None {
                return Err("raw leaf (summary_level = 0) must have SummaryMethod::None");
            }
        } else {
            if self.children.is_empty() {
                return Err("summary node (summary_level > 0) must have children");
            }
            if self.summary_method == SummaryMethod::None {
                return Err("summary node must have a non-None SummaryMethod");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SourceLocationValue;

    #[test]
    fn raw_leaf_default_is_valid() {
        let c = Chunk::raw_leaf("hello");
        c.validate_summary_invariants().unwrap();
        assert!(c.is_raw_leaf());
    }

    #[test]
    fn summary_node_needs_children_and_method() {
        let mut c = Chunk::raw_leaf("hello");
        c.summary_level = 1;
        assert!(c.validate_summary_invariants().is_err());
        c.children.push(ChunkId::new());
        assert!(c.validate_summary_invariants().is_err()); // still None method
        c.summary_method = SummaryMethod::ExtractiveBM25;
        c.validate_summary_invariants().unwrap();
    }

    #[test]
    fn with_source_sets_location() {
        let c = Chunk::raw_leaf("hello").with_source(SourceLocationValue::file("src/x.rs"));
        assert!(c.source_location.is_some());
    }

    #[test]
    fn keyword_from_str() {
        let k: Keyword = "sword".into();
        assert_eq!(k.to_string(), "sword");
    }
}
