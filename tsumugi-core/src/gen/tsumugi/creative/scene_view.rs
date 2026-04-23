use super::Character;
use crate::tsumugi::core::Chunk;
use std::collections::BTreeSet;

/// Invariant: SceneViewParticipantsUnbounded
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneView {
    pub viewed_chunk: Chunk,
    pub participants: BTreeSet<Character>,
}
