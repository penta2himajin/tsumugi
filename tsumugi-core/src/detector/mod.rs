//! EventDetector implementations.

mod cascade;
mod embedding_similarity;
mod keyword;
mod llm_classifier;
mod nli_zero_shot;

pub use cascade::CascadeDetector;
pub use embedding_similarity::EmbeddingSimilarityDetector;
pub use keyword::{DetectedEvent, KeywordDetector};
pub use llm_classifier::LLMClassifierDetector;
pub use nli_zero_shot::{NliZeroShotDetector, DEFAULT_ENTAILMENT_CLASS_INDEX, DEFAULT_THRESHOLD};
