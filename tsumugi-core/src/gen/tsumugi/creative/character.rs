use super::SpeechTraits;
use crate::tsumugi::core::Chunk;

/// Invariant: CharacterFirstAppearanceWellFormed
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Character {
    pub speech_traits: Option<SpeechTraits>,
    pub first_appearance: Option<Chunk>,
}
