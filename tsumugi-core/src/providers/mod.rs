//! Embedding / LLM provider implementations.
//!
//! - `MockEmbedding` / `MockLLMProvider`: deterministic in-process
//!   implementations for tests and dry-runs. Always available.
//! - `OpenAiCompatibleProvider` / `LmStudioEmbedding`: HTTP-backed real
//!   providers. Trait-only Phase 1 deliverables — the HTTP wiring lands
//!   in Phase 2 once reqwest is taken as a dependency. See TODO.md.

mod mock_embedding;
mod mock_llm;
mod stubs;

pub use mock_embedding::MockEmbedding;
pub use mock_llm::MockLLMProvider;
pub use stubs::{LmStudioEmbedding, OpenAiCompatibleProvider};
