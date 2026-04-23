//! LlmSummarizer — Tier 3 summarizer that delegates to an `LLMProvider`.
//!
//! Builds a short instruction prompt around the chunk text and returns the
//! LLM's completion as the summary. Honors a `max_tokens` budget so callers
//! can fit summaries into a fixed context footprint.

use crate::domain::{Chunk, SummaryMethod};
use crate::traits::llm::{CompletionRequest, LLMProvider};
use crate::traits::summarizer::Summarizer;
use async_trait::async_trait;
use std::sync::Arc;

pub struct LlmSummarizer {
    provider: Arc<dyn LLMProvider>,
    max_tokens: u32,
    /// Prompt prefix; `{text}` is substituted at call time.
    instruction: String,
}

impl LlmSummarizer {
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            max_tokens: 256,
            instruction: "Summarize the following text in 2–4 sentences. Preserve named \
                 entities and the causal / temporal order of events. Do not add \
                 speculation.\n\n{text}\n\nSummary:"
                .to_string(),
        }
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    fn render(&self, chunk: &Chunk) -> String {
        self.instruction.replace("{text}", &chunk.text)
    }
}

#[async_trait]
impl Summarizer for LlmSummarizer {
    async fn summarize(&self, chunk: &Chunk) -> anyhow::Result<String> {
        let prompt = self.render(chunk);
        let resp = self
            .provider
            .complete(&CompletionRequest {
                prompt,
                max_tokens: Some(self.max_tokens),
                temperature: Some(0.1),
                grammar: None,
                stop: None,
            })
            .await?;
        Ok(resp.text.trim().to_string())
    }

    fn method(&self) -> SummaryMethod {
        SummaryMethod::LlmFull
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockLLMProvider;

    #[tokio::test]
    async fn summarize_calls_provider() {
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("[SUM]"));
        let s = LlmSummarizer::new(llm).with_max_tokens(32);
        let mut chunk = Chunk::raw_leaf("original text");
        chunk.text = "Alice met Bob at dawn.".to_string();
        let out = s.summarize(&chunk).await.unwrap();
        assert!(out.starts_with("[SUM]"));
        assert!(out.contains("Alice met Bob"));
        assert_eq!(s.method(), SummaryMethod::LlmFull);
    }
}
