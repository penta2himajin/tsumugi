//! LoreEntry: lorebook keyword-triggered reference.

use super::ids::LoreEntryId;
use crate::domain::{ChunkId, Keyword};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoreEntry {
    pub id: LoreEntryId,
    pub category: String,
    pub title: String,
    pub content: String,
    pub scope: LoreScope,
    #[serde(default)]
    pub keywords: Vec<Keyword>,
}

impl LoreEntry {
    pub fn new(
        category: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
        scope: LoreScope,
    ) -> Self {
        Self {
            id: LoreEntryId::new(),
            category: category.into(),
            title: title.into(),
            content: content.into(),
            scope,
            keywords: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoreScope {
    Global,
    ChunkLocal(ChunkId),
    /// Conditional expression (product-defined predicate string).
    /// Must be non-empty — enforced via the `ConditionalScope` constructor.
    Conditional(ConditionalScope),
}

impl LoreScope {
    pub fn conditional(expr: impl Into<String>) -> Result<Self, &'static str> {
        ConditionalScope::new(expr).map(LoreScope::Conditional)
    }
}

/// Non-empty conditional expression wrapping a `String`. Mirrors the Alloy
/// TODO `LoreEntry.scope Conditional non-empty` invariant: enforced here on
/// the Rust side since opaque-string non-emptiness is awkward in Alloy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ConditionalScope(String);

impl ConditionalScope {
    pub fn new(expr: impl Into<String>) -> Result<Self, &'static str> {
        let s = expr.into();
        if s.trim().is_empty() {
            Err("LoreScope::Conditional expression must be non-empty")
        } else {
            Ok(Self(s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ConditionalScope {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ConditionalScope> for String {
    fn from(value: ConditionalScope) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditional_empty_string_rejected() {
        assert!(ConditionalScope::new("").is_err());
        assert!(ConditionalScope::new("   ").is_err());
        assert!(ConditionalScope::new("has_sword && hp > 0").is_ok());
    }

    #[test]
    fn lore_entry_round_trip() {
        let e = LoreEntry::new(
            "item",
            "Sword of Dawn",
            "A legendary blade.",
            LoreScope::Global,
        );
        let j = serde_json::to_string(&e).unwrap();
        let back: LoreEntry = serde_json::from_str(&j).unwrap();
        assert_eq!(e, back);
    }
}
