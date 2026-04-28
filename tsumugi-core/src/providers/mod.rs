//! Embedding / LLM provider implementations.
//!
//! - `MockEmbedding` / `MockLLMProvider`: deterministic in-process
//!   implementations for tests and dry-runs. Always available.
//! - `IkeEmbedding`: binarization wrapper over any `EmbeddingProvider`.
//!   Emits ±1 per dimension (from the sign of the upstream vector) so
//!   retrieval reduces to a Hamming-like score. Survey §4.3.
//! - `OpenAiCompatibleProvider` / `LmStudioEmbedding`: HTTP-backed real
//!   providers behind the `network` feature flag.
//! - `OnnxEmbedding`: ONNX-backed embedding provider (e.g.
//!   `multilingual-e5-small`) behind the `onnx` feature flag. Phase 4-α
//!   Step 1 では trait 面のみ先行追加。

mod ike_embedding;
mod mock_embedding;
mod mock_llm;
mod onnx_embedding;
mod stubs;

pub use ike_embedding::IkeEmbedding;
pub use mock_embedding::MockEmbedding;
pub use mock_llm::MockLLMProvider;
pub use onnx_embedding::OnnxEmbedding;
pub use stubs::{LmStudioEmbedding, OpenAiCompatibleProvider};
