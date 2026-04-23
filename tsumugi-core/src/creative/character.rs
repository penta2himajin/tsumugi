//! Character: participant / narrator / NPC / PC.

use super::ids::CharacterId;
use crate::domain::ChunkId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Character {
    pub id: CharacterId,
    pub name: String,
    #[serde(default)]
    pub voice_samples: Vec<String>,
    #[serde(default)]
    pub speech_traits: Option<SpeechTraits>,
    #[serde(default)]
    pub relationship_notes: HashMap<CharacterId, String>,
    #[serde(default)]
    pub sheet: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub first_appearance: Option<ChunkId>,
    #[serde(default)]
    pub style_tags: Vec<String>,
}

impl Character {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: CharacterId::new(),
            name: name.into(),
            voice_samples: vec![],
            speech_traits: None,
            relationship_notes: HashMap::new(),
            sheet: serde_json::Map::new(),
            first_appearance: None,
            style_tags: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechTraits {
    pub formality: Formality,
    #[serde(default)]
    pub quirks: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Formality {
    Casual,
    Neutral,
    Formal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_character_has_default_fields() {
        let c = Character::new("Alice");
        assert_eq!(c.name, "Alice");
        assert!(c.speech_traits.is_none());
    }
}
