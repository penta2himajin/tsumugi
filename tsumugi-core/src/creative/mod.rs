//! Creative-domain extension (feature = "creative").
//!
//! Mirrors `models/tsumugi/creative.als` and adds runtime fields.

mod character;
mod ids;
mod lore;
mod scene;
mod style;

pub use character::{Character, Formality, SpeechTraits};
pub use ids::{CharacterId, LoreEntryId};
pub use lore::{LoreEntry, LoreScope};
pub use scene::SceneView;
pub use style::{PoV, StylePreset, Tense};
