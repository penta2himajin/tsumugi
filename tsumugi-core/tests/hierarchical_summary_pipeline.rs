//! End-to-end test for the hierarchical summarization pipeline with edit
//! protection. Exercises `HierarchicalSummarizer` dispatching by level and
//! `apply_summary_update` respecting the user-edit / lock flags.

use std::sync::Arc;

use tsumugi_core::domain::{Chunk, SummaryMethod};
use tsumugi_core::providers::MockLLMProvider;
use tsumugi_core::summarizer::{
    apply_summary_update, ExtractiveBM25Summarizer, HierarchicalSummarizer, LlmSummarizer,
    SummaryUpdate, SummaryUpdateOutcome,
};
use tsumugi_core::traits::llm::LLMProvider;
use tsumugi_core::traits::summarizer::Summarizer;

fn leaf_with_text(text: &str) -> Chunk {
    let mut c = Chunk::raw_leaf(text);
    c.text = text.to_string();
    c
}

#[tokio::test]
async fn pipeline_summarizes_each_level_and_respects_edit_protection() {
    let llm: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::new("[L3]"));
    let tier1: Arc<dyn Summarizer> = Arc::new(ExtractiveBM25Summarizer::new(2));
    let tier3: Arc<dyn Summarizer> = Arc::new(LlmSummarizer::new(llm.clone()));

    let hierarchical = HierarchicalSummarizer::new()
        .with_level(1, tier1.clone())
        .with_level(3, tier3.clone())
        .with_default(tier1);

    // Level 1: extractive.
    let mut lvl1 =
        leaf_with_text("Alice set out. The road was quiet. Bob appeared. They walked together.");
    lvl1.summary_level = 1;
    let s1 = hierarchical.summarize(&lvl1).await.unwrap();
    let s1_clone = s1.clone();
    let outcome1 = apply_summary_update(
        &mut lvl1,
        s1,
        hierarchical.method_for(1),
        SummaryUpdate::default(),
    );
    assert_eq!(outcome1, SummaryUpdateOutcome::Applied);
    assert_eq!(lvl1.summary, s1_clone);
    assert_eq!(lvl1.summary_method, SummaryMethod::ExtractiveBM25);

    // Level 3: LLM.
    let mut lvl3 = leaf_with_text("higher-level arc spanning many chapters");
    lvl3.summary_level = 3;
    let s3 = hierarchical.summarize(&lvl3).await.unwrap();
    assert!(s3.starts_with("[L3]"));
    let outcome3 = apply_summary_update(
        &mut lvl3,
        s3,
        hierarchical.method_for(3),
        SummaryUpdate::default(),
    );
    assert_eq!(outcome3, SummaryUpdateOutcome::Applied);
    assert_eq!(lvl3.summary_method, SummaryMethod::LlmFull);

    // User edits the level-1 summary; auto-update must now skip.
    lvl1.summary = "HUMAN-WRITTEN SUMMARY".to_string();
    lvl1.edited_by_user = true;
    let refreshed = hierarchical.summarize(&lvl1).await.unwrap();
    let outcome_skipped = apply_summary_update(
        &mut lvl1,
        refreshed,
        hierarchical.method_for(1),
        SummaryUpdate::default(),
    );
    assert_eq!(outcome_skipped, SummaryUpdateOutcome::SkippedUserEdited);
    assert_eq!(lvl1.summary, "HUMAN-WRITTEN SUMMARY");

    // Explicit "regenerate" flow forces the overwrite and resets the flag.
    let refreshed2 = hierarchical.summarize(&lvl1).await.unwrap();
    let outcome_forced = apply_summary_update(
        &mut lvl1,
        refreshed2,
        hierarchical.method_for(1),
        SummaryUpdate::forced(),
    );
    assert_eq!(outcome_forced, SummaryUpdateOutcome::Applied);
    assert!(!lvl1.edited_by_user);

    // Pin the chunk; even forced updates must skip now.
    lvl1.auto_update_locked = true;
    let refreshed3 = hierarchical.summarize(&lvl1).await.unwrap();
    let outcome_locked = apply_summary_update(
        &mut lvl1,
        refreshed3,
        hierarchical.method_for(1),
        SummaryUpdate::forced(),
    );
    assert_eq!(outcome_locked, SummaryUpdateOutcome::SkippedLocked);
}
