//! HTTP-backed provider stubs. The Phase 1 deliverable is the trait surface
//! and the construction API; actual HTTP wiring (reqwest + retry policy +
//! streaming) lands in Phase 2 once the external-dependency footprint
//! is reviewed.
//!
//! Calling `embed` / `complete` on these stubs returns `anyhow::Error`
//! with a clear "not yet implemented" message so callers can fall back
//! to mocks in tests while keeping the type-level integration in place.

use crate::traits::embedding::{EmbeddingProvider, EmbeddingVector};
use crate::traits::llm::{
    CompletionRequest, CompletionResponse, LLMProvider, ModelFamily, ModelMetadata,
};
use async_trait::async_trait;

/// OpenAI-compatible chat/completion provider. Works against OpenAI proper,
/// LM Studio (`http://localhost:1234/v1`), and Ollama (`http://localhost:11434/v1`).
pub struct OpenAiCompatibleProvider {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub metadata: ModelMetadata,
}

impl OpenAiCompatibleProvider {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        let model = model.into();
        Self {
            base_url: base_url.into(),
            metadata: ModelMetadata {
                name: model.clone(),
                family: ModelFamily::Other("openai-compatible".into()),
                context_window: 8192,
                supports_grammar: true,
                supports_kv_cache_quantization: false,
            },
            model,
            api_key: None,
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }
}

#[async_trait]
impl LLMProvider for OpenAiCompatibleProvider {
    async fn complete(&self, _request: &CompletionRequest) -> anyhow::Result<CompletionResponse> {
        anyhow::bail!(
            "OpenAiCompatibleProvider::complete is not wired in Phase 1 — \
             switch to MockLLMProvider for tests, and track Phase 2 for the \
             reqwest-based implementation (base_url = {}, model = {})",
            self.base_url,
            self.model
        )
    }

    fn metadata(&self) -> ModelMetadata {
        self.metadata.clone()
    }
}

/// LM Studio's `/v1/embeddings` endpoint. Same Phase 1 stub treatment as
/// `OpenAiCompatibleProvider`.
pub struct LmStudioEmbedding {
    pub base_url: String,
    pub model: String,
    pub dimension: usize,
}

impl LmStudioEmbedding {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, dimension: usize) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            dimension,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for LmStudioEmbedding {
    async fn embed(&self, _text: &str) -> anyhow::Result<EmbeddingVector> {
        anyhow::bail!(
            "LmStudioEmbedding::embed is not wired in Phase 1 — switch to \
             MockEmbedding for tests, Phase 2 adds the reqwest call \
             (base_url = {}, model = {})",
            self.base_url,
            self.model
        )
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
