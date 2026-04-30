//! Embedding provider implementations.
//!
//! - `MockEmbedding`: deterministic in-process embedding for tests and
//!   dry-runs. Always available.
//! - `IkeEmbedding`: binarization wrapper over any `EmbeddingProvider`.
//!   Emits ±1 per dimension (from the sign of the upstream vector) so
//!   retrieval reduces to a Hamming-like score. Survey §4.3.
//! - `OnnxEmbedding`: ONNX-backed embedding provider (e.g.
//!   `multilingual-e5-small`) behind the `onnx` feature flag.
//!
//! `LLMProvider` impls (`MockLLMProvider`, `OpenAiCompatibleProvider`)
//! and `LmStudioEmbedding` were removed in the LLM-removal PR. tsumugi
//! is encoder-only; bridge to external LLM / HTTP-backed embedding
//! services in downstream application code.

mod ike_embedding;
mod mock_embedding;
mod onnx_embedding;

pub use ike_embedding::IkeEmbedding;
pub use mock_embedding::MockEmbedding;
pub use onnx_embedding::OnnxEmbedding;
