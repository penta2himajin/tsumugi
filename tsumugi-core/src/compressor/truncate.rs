//! TruncateCompressor — Tier 0 compressor that keeps head and tail tokens
//! separated by an ellipsis when the prompt exceeds the budget.
//!
//! Token counting uses whitespace splitting (rough approximation). Tier 2-3
//! compressors (LLMLingua-2, SelectiveContext) land in Phase 2.

use crate::traits::compressor::{CompressionHint, PromptCompressor};
use async_trait::async_trait;

pub struct TruncateCompressor;

#[async_trait]
impl PromptCompressor for TruncateCompressor {
    async fn compress(&self, prompt: &str, hint: CompressionHint) -> anyhow::Result<String> {
        let tokens: Vec<&str> = prompt.split_whitespace().collect();
        if tokens.len() <= hint.target_budget_tokens as usize {
            return Ok(prompt.to_string());
        }
        let tail_keep = hint.preserve_tail_tokens.min(hint.target_budget_tokens) as usize;
        let head_keep = (hint.target_budget_tokens as usize).saturating_sub(tail_keep);
        let head = tokens[..head_keep.min(tokens.len())].join(" ");
        let tail_start = tokens.len().saturating_sub(tail_keep);
        let tail = tokens[tail_start..].join(" ");
        Ok(if head.is_empty() {
            format!("… {tail}")
        } else if tail.is_empty() {
            format!("{head} …")
        } else {
            format!("{head} … {tail}")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn under_budget_is_noop() {
        let p = "alpha beta gamma";
        let out = TruncateCompressor
            .compress(p, CompressionHint::new(10, 2))
            .await
            .unwrap();
        assert_eq!(out, p);
    }

    #[tokio::test]
    async fn over_budget_keeps_head_and_tail() {
        let p = (1..=20)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let out = TruncateCompressor
            .compress(&p, CompressionHint::new(6, 2))
            .await
            .unwrap();
        assert!(out.contains("1"));
        assert!(out.contains("20"));
        assert!(out.contains("…"));
    }
}
