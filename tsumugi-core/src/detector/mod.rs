//! EventDetector implementations.

mod cascade;
mod embedding_similarity;
mod keyword;
mod llm_classifier;

pub use cascade::CascadeDetector;
pub use embedding_similarity::EmbeddingSimilarityDetector;
pub use keyword::KeywordDetector;
pub use llm_classifier::LLMClassifierDetector;
