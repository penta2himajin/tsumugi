//! LLMClassifierDetector — Tier 2/3. Sends the chunk text to an LLMProvider
//! with a yes/no prompt per label. Stops early if the LLM returns
//! "yes" / "true" / "1" prefix (case-insensitive).
//!
//! Real providers are wired in Phase 2; with `MockLLMProvider`, this always
//! echoes and classifies trivially — sufficient for plumbing tests.

use super::keyword::DetectedEvent;
use crate::domain::Chunk;
use crate::traits::detector::EventDetector;
use crate::traits::llm::{CompletionRequest, LLMProvider};
use async_trait::async_trait;
use std::sync::Arc;

pub struct LLMClassifierDetector {
    provider: Arc<dyn LLMProvider>,
    /// `(label, prompt_template)` tuples. The template receives `{text}` and
    /// `{new_turn}` placeholders substituted at detection time.
    prompts: Vec<(String, String)>,
}

impl LLMClassifierDetector {
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self {
            provider,
            prompts: vec![],
        }
    }

    pub fn with_prompt(mut self, label: impl Into<String>, template: impl Into<String>) -> Self {
        self.prompts.push((label.into(), template.into()));
        self
    }

    fn render(template: &str, chunk: &Chunk, new_turn: &serde_json::Value) -> String {
        template
            .replace("{text}", &chunk.text)
            .replace("{new_turn}", &new_turn.to_string())
    }
}

#[async_trait]
impl EventDetector for LLMClassifierDetector {
    type Event = DetectedEvent;

    async fn detect(
        &self,
        chunk: &Chunk,
        new_turn: &serde_json::Value,
    ) -> anyhow::Result<Vec<Self::Event>> {
        let mut out = Vec::new();
        for (label, template) in &self.prompts {
            let prompt = Self::render(template, chunk, new_turn);
            let resp = self
                .provider
                .complete(&CompletionRequest {
                    prompt,
                    max_tokens: Some(16),
                    temperature: Some(0.0),
                    grammar: None,
                    stop: None,
                })
                .await?;
            let t = resp.text.trim().to_lowercase();
            if t.starts_with("yes") || t.starts_with("true") || t.starts_with('1') {
                out.push(DetectedEvent {
                    label: label.clone(),
                    matched_keyword: resp.text,
                });
            }
        }
        Ok(out)
    }
}
