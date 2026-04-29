//! HTTP-backed providers.
//!
//! When the `network` feature is enabled, `OpenAiCompatibleProvider::complete`
//! and `LmStudioEmbedding::embed` hit the corresponding REST endpoints via
//! reqwest. Without the feature they remain type-surface stubs that return
//! a clear error — callers can fall back to the `Mock*` providers without
//! changing trait wiring.

use crate::traits::embedding::{EmbeddingProvider, EmbeddingVector};
#[cfg(feature = "network")]
use crate::traits::llm::GrammarSpec;
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
    #[cfg(feature = "network")]
    client: reqwest::Client,
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
            #[cfg(feature = "network")]
            client: reqwest::Client::new(),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_metadata(mut self, metadata: ModelMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    #[cfg(feature = "network")]
    fn chat_url(&self) -> String {
        // Accept both `http://host` and `http://host/v1` — append the missing
        // path segment so either form works.
        let base = self.base_url.trim_end_matches('/');
        if base.ends_with("/v1") {
            format!("{base}/chat/completions")
        } else {
            format!("{base}/v1/chat/completions")
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAiCompatibleProvider {
    #[cfg(feature = "network")]
    async fn complete(&self, request: &CompletionRequest) -> anyhow::Result<CompletionResponse> {
        let body = build_chat_body(&self.model, request);
        let mut req = self.client.post(self.chat_url()).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req.send().await?.error_for_status()?;
        let parsed: ChatResponse = resp.json().await?;
        Ok(parsed.into_completion())
    }

    #[cfg(not(feature = "network"))]
    async fn complete(&self, _request: &CompletionRequest) -> anyhow::Result<CompletionResponse> {
        anyhow::bail!(
            "OpenAiCompatibleProvider::complete requires the `network` feature \
             (base_url = {}, model = {}). Rebuild with `--features network` \
             or switch to MockLLMProvider for tests.",
            self.base_url,
            self.model
        )
    }

    fn metadata(&self) -> ModelMetadata {
        self.metadata.clone()
    }
}

/// LM Studio's `/v1/embeddings` endpoint. Same shape as OpenAI embeddings.
pub struct LmStudioEmbedding {
    pub base_url: String,
    pub model: String,
    pub dimension: usize,
    pub api_key: Option<String>,
    #[cfg(feature = "network")]
    client: reqwest::Client,
}

impl LmStudioEmbedding {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, dimension: usize) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            dimension,
            api_key: None,
            #[cfg(feature = "network")]
            client: reqwest::Client::new(),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    #[cfg(feature = "network")]
    fn embeddings_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        if base.ends_with("/v1") {
            format!("{base}/embeddings")
        } else {
            format!("{base}/v1/embeddings")
        }
    }
}

#[async_trait]
impl EmbeddingProvider for LmStudioEmbedding {
    #[cfg(feature = "network")]
    async fn embed(&self, text: &str) -> anyhow::Result<EmbeddingVector> {
        let body = serde_json::json!({
            "model": self.model,
            "input": text,
        });
        let mut req = self.client.post(self.embeddings_url()).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req.send().await?.error_for_status()?;
        let parsed: EmbeddingsResponse = resp.json().await?;
        let Some(first) = parsed.data.into_iter().next() else {
            anyhow::bail!(
                "embeddings response had no `data` items (model = {})",
                self.model
            );
        };
        if first.embedding.len() != self.dimension {
            anyhow::bail!(
                "embedding dimension mismatch: expected {}, got {}",
                self.dimension,
                first.embedding.len()
            );
        }
        Ok(EmbeddingVector::new(first.embedding))
    }

    #[cfg(not(feature = "network"))]
    async fn embed(&self, _text: &str) -> anyhow::Result<EmbeddingVector> {
        anyhow::bail!(
            "LmStudioEmbedding::embed requires the `network` feature \
             (base_url = {}, model = {}). Rebuild with `--features network` \
             or switch to MockEmbedding for tests.",
            self.base_url,
            self.model
        )
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

// ---------------------------------------------------------------------------
// Wire-format structs (network feature only)
// ---------------------------------------------------------------------------

#[cfg(feature = "network")]
fn build_chat_body(model: &str, request: &CompletionRequest) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": request.prompt }],
    });
    if let Some(max) = request.max_tokens {
        body["max_tokens"] = max.into();
    }
    if let Some(t) = request.temperature {
        body["temperature"] = t.into();
    }
    if let Some(stop) = &request.stop {
        body["stop"] = serde_json::to_value(stop).unwrap_or(serde_json::Value::Null);
    }
    if let Some(grammar) = &request.grammar {
        // OpenAI / LM Studio exposes JSON schema via `response_format`; GBNF
        // is LM Studio / llama.cpp specific and passed as `grammar`.
        match grammar {
            GrammarSpec::JsonSchema(schema) => {
                body["response_format"] = serde_json::json!({
                    "type": "json_schema",
                    "json_schema": { "name": "response", "schema": schema }
                });
            }
            GrammarSpec::Gbnf(gbnf) => {
                body["grammar"] = serde_json::Value::String(gbnf.clone());
            }
        }
    }
    body
}

#[cfg(feature = "network")]
#[derive(serde::Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<ChatUsage>,
}

#[cfg(feature = "network")]
#[derive(serde::Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[cfg(feature = "network")]
#[derive(serde::Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
    /// llama.cpp 系サーバーは Qwen3 等の thinking モデルで
    /// `<think>...</think>` 部分を `reasoning_content` に分離して返す。
    /// `content` が空でも `reasoning_content` に答えが残っているケースが
    /// あるため、`CompletionResponse.reasoning_text` で露出する。
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[cfg(feature = "network")]
#[derive(serde::Deserialize)]
struct ChatUsage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
}

#[cfg(feature = "network")]
impl ChatResponse {
    fn into_completion(self) -> CompletionResponse {
        let first = self.choices.into_iter().next().map(|c| c.message);
        let (text, reasoning_text) = match first {
            Some(m) => (
                m.content.unwrap_or_default(),
                m.reasoning_content.filter(|s| !s.is_empty()),
            ),
            None => (String::new(), None),
        };
        CompletionResponse {
            text,
            reasoning_text,
            prompt_tokens: self.usage.as_ref().and_then(|u| u.prompt_tokens),
            completion_tokens: self.usage.and_then(|u| u.completion_tokens),
        }
    }
}

#[cfg(feature = "network")]
#[derive(serde::Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingsDatum>,
}

#[cfg(feature = "network")]
#[derive(serde::Deserialize)]
struct EmbeddingsDatum {
    embedding: Vec<f32>,
}

#[cfg(all(test, feature = "network"))]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn openai_compatible_roundtrip() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{ "message": { "role": "assistant", "content": "pong" } }],
                "usage": { "prompt_tokens": 3, "completion_tokens": 1 }
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatibleProvider::new(server.uri(), "mock-model");
        let resp = provider
            .complete(&CompletionRequest {
                prompt: "ping".into(),
                max_tokens: Some(16),
                temperature: Some(0.0),
                grammar: None,
                stop: None,
            })
            .await
            .unwrap();
        assert_eq!(resp.text, "pong");
        assert_eq!(resp.reasoning_text, None);
        assert_eq!(resp.prompt_tokens, Some(3));
        assert_eq!(resp.completion_tokens, Some(1));
    }

    #[tokio::test]
    async fn openai_compatible_extracts_reasoning_content() {
        // llama.cpp 系の Qwen3 thinking モデルでは `<think>...</think>` 部分が
        // `reasoning_content` に分離され、`content` が空のまま返ってくることが
        // ある (実機 oracle smoke #4-#5 で観測)。両フィールドを正しく拾えること
        // を保証する。
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "",
                        "reasoning_content": "Let me think about this step by step. The user repainted to blue."
                    }
                }],
                "usage": { "prompt_tokens": 100, "completion_tokens": 64 }
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatibleProvider::new(server.uri(), "qwen3.5-4b");
        let resp = provider
            .complete(&CompletionRequest {
                prompt: "what color".into(),
                max_tokens: Some(64),
                temperature: Some(0.0),
                grammar: None,
                stop: None,
            })
            .await
            .unwrap();
        assert_eq!(resp.text, "");
        assert!(resp
            .reasoning_text
            .as_deref()
            .is_some_and(|t| t.contains("blue")));
    }

    #[tokio::test]
    async fn openai_compatible_propagates_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let provider = OpenAiCompatibleProvider::new(server.uri(), "mock-model");
        let err = provider
            .complete(&CompletionRequest {
                prompt: "ping".into(),
                max_tokens: None,
                temperature: None,
                grammar: None,
                stop: None,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("500"));
    }

    #[tokio::test]
    async fn lmstudio_embed_roundtrip() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{ "embedding": [0.1f32, 0.2, 0.3, 0.4] }]
            })))
            .mount(&server)
            .await;
        let provider = LmStudioEmbedding::new(server.uri(), "mock-embed", 4);
        let v = provider.embed("hello").await.unwrap();
        assert_eq!(v.len(), 4);
        assert!((v.as_slice()[0] - 0.1).abs() < 1e-6);
    }

    #[tokio::test]
    async fn lmstudio_embed_dim_mismatch_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{ "embedding": [0.1f32, 0.2] }]
            })))
            .mount(&server)
            .await;
        let provider = LmStudioEmbedding::new(server.uri(), "mock-embed", 4);
        let err = provider.embed("hello").await.unwrap_err();
        assert!(err.to_string().contains("dimension"));
    }

    #[tokio::test]
    async fn base_url_tolerates_trailing_and_v1() {
        let plain = OpenAiCompatibleProvider::new("http://host", "m");
        let suffixed = OpenAiCompatibleProvider::new("http://host/v1", "m");
        assert_eq!(plain.chat_url(), "http://host/v1/chat/completions");
        assert_eq!(suffixed.chat_url(), "http://host/v1/chat/completions");
    }
}

#[cfg(all(test, not(feature = "network")))]
mod stub_tests {
    use super::*;

    #[tokio::test]
    async fn stub_returns_error_without_network_feature() {
        let provider = OpenAiCompatibleProvider::new("http://unreachable", "mock");
        let err = provider
            .complete(&CompletionRequest {
                prompt: "x".into(),
                max_tokens: None,
                temperature: None,
                grammar: None,
                stop: None,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("network"));
    }
}
