//! RelevanceScorer implementations.

mod chapter_order;
mod composite;
mod file_proximity;
mod no_decay;
mod temporal_decay;

pub use chapter_order::ChapterOrderScorer;
pub use composite::CompositeScorer;
pub use file_proximity::FileProximityScorer;
pub use no_decay::NoDecayScorer;
pub use temporal_decay::TemporalDecayScorer;
