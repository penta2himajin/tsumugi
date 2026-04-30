//! The 8 core trait definitions.
//!
//! See `docs/tech-architecture.md` §核心 trait. Each trait lives in its own
//! submodule so implementations can be attached without crate-level clutter.
//!
//! `LLMProvider` was removed in the LLM-removal PR — tsumugi is an
//! encoder-only memory layer; downstream consumers that need an LLM call
//! should bridge to one in their own application code.

pub mod classifier;
pub mod compressor;
pub mod detector;
pub mod embedding;
pub mod retriever;
pub mod scorer;
pub mod storage;
pub mod summarizer;

pub use classifier::{QueryClass, QueryClassifier};
pub use compressor::{CompressionHint, PromptCompressor};
pub use detector::EventDetector;
pub use embedding::{EmbeddingProvider, EmbeddingVector};
pub use retriever::{RetrievalHit, Retriever};
pub use scorer::{RelevanceScorer, ScoringContext};
pub use storage::StorageProvider;
pub use summarizer::Summarizer;
