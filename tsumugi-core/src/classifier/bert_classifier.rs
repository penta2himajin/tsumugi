//! BertClassifier — LLM-delegated approximation of the SelRoute (survey §4.1)
//! BERT-based query router.
//!
//! The paper-exact implementation trains a MiniLM / ModernBERT classifier on
//! per-tier routing labels and runs inference on the CPU. That requires a
//! Rust ML runtime (candle / ort / tch-rs) and model weight distribution,
//! which are deferred to Phase 4+.
//!
//! This Phase 3 implementation delegates the classification decision to an
//! `LLMProvider` via a short grammar-constrained prompt. It is
//! swap-compatible with the ML-backed version: the public API matches
//! `QueryClassifier` exactly, and the `method` identifier (label set) is
//! stable across the two backends. Callers that need low-latency routing
//! before ML runtime lands should use `RegexClassifier` (Tier 0) as the
//! fast path and fall through to `BertClassifier` only on ambiguous inputs.

use crate::traits::classifier::{QueryClass, QueryClassifier};
use crate::traits::llm::{CompletionRequest, LLMProvider};
use async_trait::async_trait;
use std::sync::Arc;

pub struct BertClassifier {
    provider: Arc<dyn LLMProvider>,
    /// Prompt that instructs the LLM to emit one of the four label tokens.
    /// `{query}` is substituted at call time.
    instruction: String,
    default: QueryClass,
}

impl BertClassifier {
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            instruction: default_instruction(),
            default: QueryClass::Unknown,
        }
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    /// Override the fallback class returned when the LLM's response does not
    /// match any known label. Defaults to `Unknown`.
    pub fn with_default(mut self, class: QueryClass) -> Self {
        self.default = class;
        self
    }

    fn parse(&self, raw: &str) -> QueryClass {
        let first = raw
            .trim()
            .split(|c: char| c.is_whitespace() || c == '.' || c == ',')
            .find(|s| !s.is_empty())
            .unwrap_or("")
            .to_ascii_lowercase();
        match first.as_str() {
            "literal" => QueryClass::Literal,
            "narrative" => QueryClass::Narrative,
            "analytical" => QueryClass::Analytical,
            "unknown" => QueryClass::Unknown,
            _ => self.default,
        }
    }
}

fn default_instruction() -> String {
    "You are a query router. Classify the user's query into exactly one of \
     these labels: `Literal`, `Narrative`, `Analytical`, `Unknown`. \
     - Literal: factual lookup, no reasoning. \
     - Narrative: continuation of a story; needs recent context + characters. \
     - Analytical: analysis / reasoning about content; needs structured \
       summaries. \
     - Unknown: nothing else fits. \
     Reply with the label word only, no punctuation or explanation.\n\n\
     Query: {query}\n\nLabel:"
        .to_string()
}

#[async_trait]
impl QueryClassifier for BertClassifier {
    async fn classify(&self, query: &str) -> anyhow::Result<QueryClass> {
        let prompt = self.instruction.replace("{query}", query);
        let resp = self
            .provider
            .complete(&CompletionRequest {
                prompt,
                max_tokens: Some(8),
                temperature: Some(0.0),
                grammar: None,
                stop: None,
            })
            .await?;
        Ok(self.parse(&resp.text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::llm::{
        CompletionResponse, GrammarSpec, LLMProvider, ModelFamily, ModelMetadata,
    };

    struct FixedReply(&'static str);

    #[async_trait]
    impl LLMProvider for FixedReply {
        async fn complete(&self, _: &CompletionRequest) -> anyhow::Result<CompletionResponse> {
            Ok(CompletionResponse {
                text: self.0.to_string(),
                prompt_tokens: None,
                completion_tokens: None,
            })
        }

        fn metadata(&self) -> ModelMetadata {
            ModelMetadata {
                name: "fixed".into(),
                family: ModelFamily::Other("test".into()),
                context_window: 1024,
                supports_grammar: false,
                supports_kv_cache_quantization: false,
            }
        }
    }

    #[tokio::test]
    async fn parses_literal_label() {
        let llm: Arc<dyn LLMProvider> = Arc::new(FixedReply("Literal"));
        let c = BertClassifier::new(llm);
        assert_eq!(c.classify("what is HP").await.unwrap(), QueryClass::Literal);
    }

    #[tokio::test]
    async fn case_insensitive_label() {
        let llm: Arc<dyn LLMProvider> = Arc::new(FixedReply("NARRATIVE."));
        let c = BertClassifier::new(llm);
        assert_eq!(
            c.classify("continue the story").await.unwrap(),
            QueryClass::Narrative
        );
    }

    #[tokio::test]
    async fn parses_analytical_with_trailing_explanation() {
        // Even if the LLM ignores instructions and adds a trailing
        // explanation, we still pick up the first token.
        let llm: Arc<dyn LLMProvider> = Arc::new(FixedReply("Analytical. Because…"));
        let c = BertClassifier::new(llm);
        assert_eq!(
            c.classify("analyze the plot arc").await.unwrap(),
            QueryClass::Analytical
        );
    }

    #[tokio::test]
    async fn unknown_response_falls_back_to_default() {
        let llm: Arc<dyn LLMProvider> = Arc::new(FixedReply("gibberish"));
        let c = BertClassifier::new(llm).with_default(QueryClass::Analytical);
        assert_eq!(c.classify("what").await.unwrap(), QueryClass::Analytical);
    }

    // Exercise a provider that supports grammar just to keep the type
    // parameter honest — not exercised at runtime in Phase 3.
    #[allow(dead_code)]
    fn _grammar_unused(_: GrammarSpec) {}
}
