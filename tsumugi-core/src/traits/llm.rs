//! LLMProvider: prompt → completion, with model metadata and grammar-constrained output.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Request passed to the LLM provider.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Optional grammar constraint (JSON schema, GBNF, …). Providers that
    /// do not support the specified grammar should return an error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar: Option<GrammarSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

/// Provider's reply.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
}

/// Grammar constraint that narrows provider output.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GrammarSpec {
    JsonSchema(serde_json::Value),
    Gbnf(String),
}

/// Coarse model family classification for cost / capability routing.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelFamily {
    Llama,
    Qwen,
    Gemma,
    Phi,
    Mistral,
    OpenAi,
    Anthropic,
    Other(String),
}

/// Metadata advertised by a provider. Lets callers pick a provider by
/// context window, grammar support, or cache-quantization capability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub name: String,
    pub family: ModelFamily,
    pub context_window: u32,
    #[serde(default)]
    pub supports_grammar: bool,
    #[serde(default)]
    pub supports_kv_cache_quantization: bool,
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn complete(&self, request: &CompletionRequest) -> anyhow::Result<CompletionResponse>;

    fn metadata(&self) -> ModelMetadata;
}
