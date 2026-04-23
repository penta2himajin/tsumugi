//! Facts: stored assertions with a supersession chain.
//!
//! Mirrors `models/tsumugi/core.als` (`Fact`, `FactScope`, `FactOrigin`).

use super::ids::{ChunkId, FactId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    pub id: FactId,
    pub key: String,
    pub value: String,
    pub scope: FactScope,
    /// ID of the fact that supersedes this one, if any. Chains form a
    /// strict partial order (see `NoCyclicSupersession` in the Alloy model).
    pub superseded_by: Option<FactId>,
    pub created_at: DateTime<Utc>,
    pub origin: FactOrigin,
}

impl Fact {
    pub fn new(
        key: impl Into<String>,
        value: impl Into<String>,
        scope: FactScope,
        origin: FactOrigin,
    ) -> Self {
        Self {
            id: FactId::new(),
            key: key.into(),
            value: value.into(),
            scope,
            superseded_by: None,
            created_at: Utc::now(),
            origin,
        }
    }

    pub fn is_active(&self) -> bool {
        self.superseded_by.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactScope {
    Global,
    ChunkLocal(ChunkId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactOrigin {
    User,
    Extracted,
    Derived,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_fact_is_active() {
        let f = Fact::new("hp", "12", FactScope::Global, FactOrigin::User);
        assert!(f.is_active());
    }

    #[test]
    fn superseded_fact_is_inactive() {
        let mut f = Fact::new("hp", "12", FactScope::Global, FactOrigin::User);
        f.superseded_by = Some(FactId::new());
        assert!(!f.is_active());
    }
}
