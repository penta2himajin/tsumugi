//! HierarchicalSummarizer — picks a summarizer per `summary_level`.
//!
//! Level 0 is a raw leaf and typically should not be summarized; level 1 is
//! a first-pass (extractive) summary suitable for fast updates; level 2+ is
//! an LLM-driven abstractive summary. Callers register concrete summarizers
//! per level and `HierarchicalSummarizer::summarize` dispatches to the match
//! with the greatest level ≤ chunk's `summary_level`, falling back to the
//! `default` summarizer if no exact match exists.

use crate::domain::{Chunk, SummaryMethod};
use crate::traits::summarizer::Summarizer;
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct HierarchicalSummarizer {
    levels: BTreeMap<u32, Arc<dyn Summarizer>>,
    default: Option<Arc<dyn Summarizer>>,
}

impl HierarchicalSummarizer {
    pub fn new() -> Self {
        Self {
            levels: BTreeMap::new(),
            default: None,
        }
    }

    pub fn with_level(mut self, level: u32, summarizer: Arc<dyn Summarizer>) -> Self {
        self.levels.insert(level, summarizer);
        self
    }

    pub fn with_default(mut self, summarizer: Arc<dyn Summarizer>) -> Self {
        self.default = Some(summarizer);
        self
    }

    fn pick(&self, level: u32) -> Option<&Arc<dyn Summarizer>> {
        // Greatest registered level ≤ requested level.
        self.levels
            .range(..=level)
            .next_back()
            .map(|(_, s)| s)
            .or(self.default.as_ref())
    }
}

impl Default for HierarchicalSummarizer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Summarizer for HierarchicalSummarizer {
    async fn summarize(&self, chunk: &Chunk) -> anyhow::Result<String> {
        let level = chunk.summary_level.max(1);
        let Some(summarizer) = self.pick(level) else {
            anyhow::bail!(
                "HierarchicalSummarizer has no summarizer for level {level} and no default"
            );
        };
        summarizer.summarize(chunk).await
    }

    fn method(&self) -> SummaryMethod {
        // Method varies by level; report the default's method when present,
        // else fall back to None. Callers that care about the specific method
        // should inspect the dispatched summarizer directly via `pick_for`.
        self.default
            .as_ref()
            .map(|s| s.method())
            .unwrap_or(SummaryMethod::None)
    }
}

impl HierarchicalSummarizer {
    /// Expose the dispatched summarizer for a given level so the Context
    /// Compiler can record the exact `SummaryMethod` on the generated chunk.
    pub fn method_for(&self, level: u32) -> SummaryMethod {
        self.pick(level.max(1))
            .map(|s| s.method())
            .unwrap_or(SummaryMethod::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summarizer::ExtractiveBM25Summarizer;

    /// Fixed-output summarizer for hierarchical dispatch tests. Replaces
    /// the LLM-delegated summarizer that this test originally used; the
    /// LLM-removal PR took out `LlmSummarizer` so we exercise tier-3
    /// dispatch via a sentinel string.
    struct FixedSummarizer {
        text: String,
        method: SummaryMethod,
    }

    #[async_trait::async_trait]
    impl Summarizer for FixedSummarizer {
        async fn summarize(&self, _chunk: &Chunk) -> anyhow::Result<String> {
            Ok(self.text.clone())
        }

        fn method(&self) -> SummaryMethod {
            self.method
        }
    }

    #[tokio::test]
    async fn dispatches_by_level() {
        let tier1: Arc<dyn Summarizer> = Arc::new(ExtractiveBM25Summarizer::new(2));
        let tier3: Arc<dyn Summarizer> = Arc::new(FixedSummarizer {
            text: "[TIER3] sentinel".into(),
            method: SummaryMethod::DistilBart,
        });

        let h = HierarchicalSummarizer::new()
            .with_level(1, tier1)
            .with_level(3, tier3);

        let mut lvl1 = Chunk::raw_leaf("");
        lvl1.summary_level = 1;
        lvl1.text = "first. second. third.".to_string();
        let out1 = h.summarize(&lvl1).await.unwrap();
        assert!(!out1.starts_with("[TIER3]"));

        let mut lvl3 = Chunk::raw_leaf("");
        lvl3.summary_level = 3;
        lvl3.text = "higher-level content".to_string();
        let out3 = h.summarize(&lvl3).await.unwrap();
        assert!(out3.starts_with("[TIER3]"));
    }

    #[tokio::test]
    async fn falls_back_to_nearest_lower_level() {
        let tier1: Arc<dyn Summarizer> = Arc::new(ExtractiveBM25Summarizer::new(2));
        let h = HierarchicalSummarizer::new().with_level(1, tier1);

        // No summarizer registered at level 2 — picks level-1.
        let mut lvl2 = Chunk::raw_leaf("");
        lvl2.summary_level = 2;
        lvl2.text = "a. b. c.".to_string();
        h.summarize(&lvl2).await.unwrap();
    }

    #[tokio::test]
    async fn errors_when_no_summarizer() {
        let h = HierarchicalSummarizer::new();
        let chunk = Chunk::raw_leaf("x");
        let err = h.summarize(&chunk).await.unwrap_err();
        assert!(err.to_string().contains("no summarizer"));
    }

    #[test]
    fn method_for_reports_dispatched_method() {
        let t1: Arc<dyn Summarizer> = Arc::new(ExtractiveBM25Summarizer::new(2));
        let h = HierarchicalSummarizer::new().with_level(1, t1);
        assert_eq!(h.method_for(1), SummaryMethod::ExtractiveBM25);
        assert_eq!(h.method_for(5), SummaryMethod::ExtractiveBM25);
        assert_eq!(
            HierarchicalSummarizer::new().method_for(1),
            SummaryMethod::None
        );
    }
}
