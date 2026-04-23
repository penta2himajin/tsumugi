//! LlmLinguaCompressor — LLM-delegated approximation of LLMLingua-2.
//!
//! Real LLMLingua-2 (Pan et al., 2024) uses a fine-tuned XLM-RoBERTa classifier
//! to predict per-token keep/discard labels. That requires a Rust ML runtime
//! (ort / candle / tch-rs), which is out of scope for Phase 2 — the Cargo
//! target graph stays minimal and CI doesn't need accelerators.
//!
//! This implementation delegates the compression decision to an `LLMProvider`
//! with an explicit instruction to preserve named entities and factual claims
//! while trimming filler. It is NOT paper-exact in compression ratio or token
//! fidelity; it's a pragmatic middleware implementation that swaps 1:1 with
//! the future ML-backed version when that lands in Phase 3+.

use crate::traits::compressor::{CompressionHint, PromptCompressor};
use crate::traits::llm::{CompletionRequest, LLMProvider};
use async_trait::async_trait;
use std::sync::Arc;

pub struct LlmLinguaCompressor {
    provider: Arc<dyn LLMProvider>,
    instruction_template: String,
}

impl LlmLinguaCompressor {
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            instruction_template: "Compress the text below to approximately {budget} tokens. Keep \
                 named entities, numbers, dates, and quoted speech verbatim. Drop \
                 filler phrases, redundant connectors, and stylistic flourishes. \
                 Output only the compressed text.\n\n{text}\n\nCompressed:"
                .to_string(),
        }
    }

    pub fn with_instruction(mut self, template: impl Into<String>) -> Self {
        self.instruction_template = template.into();
        self
    }

    fn build_prompt(&self, text: &str, hint: CompressionHint) -> String {
        self.instruction_template
            .replace("{budget}", &hint.target_budget_tokens.to_string())
            .replace("{text}", text)
    }
}

#[async_trait]
impl PromptCompressor for LlmLinguaCompressor {
    async fn compress(&self, prompt: &str, hint: CompressionHint) -> anyhow::Result<String> {
        // Cheap guard — if the prompt already fits, skip the round-trip.
        if prompt.split_whitespace().count() <= hint.target_budget_tokens as usize {
            return Ok(prompt.to_string());
        }
        let rendered = self.build_prompt(prompt, hint);
        let resp = self
            .provider
            .complete(&CompletionRequest {
                prompt: rendered,
                max_tokens: Some(hint.target_budget_tokens.max(64)),
                temperature: Some(0.0),
                grammar: None,
                stop: None,
            })
            .await?;
        Ok(resp.text.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockLLMProvider;

    #[tokio::test]
    async fn under_budget_is_noop() {
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("[COMP]"));
        let c = LlmLinguaCompressor::new(llm);
        let out = c
            .compress("one two three", CompressionHint::new(10, 2))
            .await
            .unwrap();
        assert_eq!(out, "one two three");
    }

    #[tokio::test]
    async fn over_budget_delegates_to_llm() {
        let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("[COMP]"));
        let c = LlmLinguaCompressor::new(llm);
        let long_prompt = (1..=30)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let out = c
            .compress(&long_prompt, CompressionHint::new(10, 2))
            .await
            .unwrap();
        assert!(out.starts_with("[COMP]"));
        // The MockLLMProvider echoes back the instruction prompt, so the
        // output mentions the budget.
        assert!(out.contains("approximately 10 tokens"));
    }
}
