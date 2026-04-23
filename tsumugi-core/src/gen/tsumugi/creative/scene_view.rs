use std::collections::{BTreeSet};
use crate::tsumugi::core::{Chunk};
use super::Character;

/// Invariant: SceneViewParticipantsUnbounded
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneView {
    pub viewed_chunk: Chunk,
    pub participants: BTreeSet<Character>,
}

