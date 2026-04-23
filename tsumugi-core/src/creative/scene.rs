//! SceneView: specialized read-only view onto a Chunk.

use super::ids::CharacterId;
use crate::domain::Chunk;

/// A read-only reference to a Chunk combined with scene-scoped context
/// (participants, location tag, time marker) computed by the product.
#[derive(Debug)]
pub struct SceneView<'a> {
    pub chunk: &'a Chunk,
    pub participants: Vec<CharacterId>,
    pub location: Option<String>,
    pub time_marker: Option<String>,
}

impl<'a> SceneView<'a> {
    pub fn new(chunk: &'a Chunk) -> Self {
        Self {
            chunk,
            participants: vec![],
            location: None,
            time_marker: None,
        }
    }

    pub fn with_participants(mut self, participants: Vec<CharacterId>) -> Self {
        self.participants = participants;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_scene_view_binds_chunk() {
        let chunk = Chunk::raw_leaf("scene 1");
        let view = SceneView::new(&chunk);
        assert_eq!(view.chunk.id, chunk.id);
        assert!(view.participants.is_empty());
    }
}
