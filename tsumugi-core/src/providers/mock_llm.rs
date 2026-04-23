//! Mock LLM provider — deterministic echo-with-prefix. Drives the pipeline
//! in tests without requiring a running inference server.

use crate::traits::llm::{
    CompletionRequest, CompletionResponse, LLMProvider, ModelFamily, ModelMetadata,
};
use async_trait::async_trait;

pub struct MockLLMProvider {
    prefix: String,
    metadata: ModelMetadata,
}

impl MockLLMProvider {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            metadata: ModelMetadata {
                name: "mock".into(),
                family: ModelFamily::Other("mock".into()),
                context_window: 8192,
                supports_grammar: false,
                supports_kv_cache_quantization: false,
            },
        }
    }
}

impl Default for MockLLMProvider {
    fn default() -> Self {
        Self::new("[MOCK]")
    }
}

#[async_trait]
impl LLMProvider for MockLLMProvider {
    async fn complete(&self, request: &CompletionRequest) -> anyhow::Result<CompletionResponse> {
        let text = format!("{} {}", self.prefix, request.prompt);
        let prompt_tokens = request.prompt.split_whitespace().count() as u32;
        let completion_tokens = text.split_whitespace().count() as u32 - prompt_tokens;
        Ok(CompletionResponse {
            text,
            prompt_tokens: Some(prompt_tokens),
            completion_tokens: Some(completion_tokens),
        })
    }

    fn metadata(&self) -> ModelMetadata {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_prepends_prefix() {
        let llm = MockLLMProvider::new("[TEST]");
        let req = CompletionRequest {
            prompt: "hello".into(),
            max_tokens: None,
            temperature: None,
            grammar: None,
            stop: None,
        };
        let res = llm.complete(&req).await.unwrap();
        assert!(res.text.starts_with("[TEST]"));
        assert!(res.text.contains("hello"));
    }
}
