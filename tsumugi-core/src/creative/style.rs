//! StylePreset: writing-style configuration.

use super::character::Formality;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StylePreset {
    pub pov: PoV,
    pub tense: Tense,
    pub formality: Formality,
    #[serde(default)]
    pub reference_samples: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PoV {
    First,
    Second,
    Third,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tense {
    Present,
    Past,
}
