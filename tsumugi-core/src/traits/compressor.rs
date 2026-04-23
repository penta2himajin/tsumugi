//! PromptCompressor: compress a prompt payload to fit a budget.

use async_trait::async_trait;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompressionHint {
    pub target_budget_tokens: u32,
    pub preserve_tail_tokens: u32,
}

impl CompressionHint {
    pub fn new(target_budget_tokens: u32, preserve_tail_tokens: u32) -> Self {
        Self {
            target_budget_tokens,
            preserve_tail_tokens,
        }
    }
}

#[async_trait]
pub trait PromptCompressor: Send + Sync {
    async fn compress(&self, prompt: &str, hint: CompressionHint) -> anyhow::Result<String>;
}
