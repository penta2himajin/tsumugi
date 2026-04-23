//! The 9 core trait definitions.
//!
//! See `docs/tech-architecture.md` §核心 trait. Each trait lives in its own
//! submodule so implementations can be attached without crate-level clutter.

pub mod classifier;
pub mod compressor;
pub mod detector;
pub mod embedding;
pub mod llm;
pub mod retriever;
pub mod scorer;
pub mod storage;
pub mod summarizer;

pub use classifier::{QueryClass, QueryClassifier};
pub use compressor::{CompressionHint, PromptCompressor};
pub use detector::EventDetector;
pub use embedding::{EmbeddingProvider, EmbeddingVector};
pub use llm::{
    CompletionRequest, CompletionResponse, GrammarSpec, LLMProvider, ModelFamily, ModelMetadata,
};
pub use retriever::{RetrievalHit, Retriever};
pub use scorer::{RelevanceScorer, ScoringContext};
pub use storage::StorageProvider;
pub use summarizer::Summarizer;
