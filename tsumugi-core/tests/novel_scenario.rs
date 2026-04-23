//! End-to-end smoke test for the `creative` feature set.
//!
//! Mimics a novel-writing session (→ `つづり`): chunks are chapters ordered
//! by `order_in_parent`, scoring blends BM25 retrieval with chapter-order
//! decay, and the Context Compiler emits a prompt the downstream LLM call
//! would consume.

#![cfg(feature = "creative")]

use std::sync::Arc;

use tsumugi_core::compiler::ContextCompiler;
use tsumugi_core::creative::{Character, LoreEntry, LoreScope};
use tsumugi_core::domain::Chunk;
use tsumugi_core::retriever::Bm25Retriever;
use tsumugi_core::scorer::{ChapterOrderScorer, CompositeScorer, NoDecayScorer};
use tsumugi_core::storage::InMemoryStorage;
use tsumugi_core::summarizer::ExtractiveBM25Summarizer;
use tsumugi_core::traits::retriever::Retriever;
use tsumugi_core::traits::scorer::RelevanceScorer;
use tsumugi_core::traits::storage::StorageProvider;
use tsumugi_core::traits::summarizer::Summarizer;

fn make_chapter(title: &str, text: &str, order: i64) -> Chunk {
    let mut c = Chunk::raw_leaf(format!("{title}\n\n{text}"));
    c.order_in_parent = order;
    c
}

#[tokio::test]
async fn creative_pipeline_assembles_and_summarizes() {
    let storage: Arc<dyn StorageProvider> = Arc::new(InMemoryStorage::new());

    let ch1 = make_chapter(
        "Chapter 1: Departure",
        "Alice leaves her village at dawn. The road leads north.",
        1,
    );
    let ch2 = make_chapter(
        "Chapter 2: Forest",
        "A dark forest blocks the road. Alice hears a distant wolf.",
        2,
    );
    let ch3 = make_chapter(
        "Chapter 3: Encounter",
        "Alice meets Bob at the crossroads. They agree to travel together.",
        3,
    );
    let ch4 = make_chapter(
        "Chapter 4: Mountain",
        "The pair climb the mountain pass. Snow begins to fall.",
        4,
    );
    let current_id = ch3.id;
    let current_order = ch3.order_in_parent;

    for c in [&ch1, &ch2, &ch3, &ch4] {
        storage.save_chunk(c).await.unwrap();
    }

    // Creative fixtures (character + lore).
    let alice = Character::new("Alice");
    storage.save_character(&alice).await.unwrap();
    let lore = LoreEntry::new(
        "location",
        "Crossroads",
        "A waypoint where multiple roads meet.",
        LoreScope::Global,
    );
    storage.save_lore(&lore).await.unwrap();

    // Build retriever + scorer blending BM25 with chapter-order decay.
    let corpus: Vec<_> = [&ch1, &ch2, &ch3, &ch4]
        .iter()
        .map(|c| (c.id, c.text.clone()))
        .collect();
    let retriever: Arc<dyn Retriever> = Arc::new(Bm25Retriever::new(corpus));
    let scorer: Arc<dyn RelevanceScorer> = Arc::new(
        CompositeScorer::new()
            .add(Arc::new(NoDecayScorer), 1.0)
            .add(Arc::new(ChapterOrderScorer::new(0.2)), 2.0),
    );

    let compiler = ContextCompiler::new(storage.clone(), retriever, scorer).with_limits(10, 3);

    // Simulate asking about the journey from Chapter 3's vantage.
    let mut updated_current = ch3.clone();
    updated_current.order_in_parent = current_order;
    let ctx = compiler
        .compile("Alice road forest", Some(current_id), None)
        .await
        .unwrap();

    // Resident layer is just ch3 (no parent chain in this fixture).
    assert_eq!(ctx.resident_chunks.len(), 1);
    assert_eq!(ctx.resident_chunks[0].id, current_id);

    // Dynamic chunks should not include the current chunk.
    assert!(!ctx.dynamic_chunks.iter().any(|s| s.chunk.id == current_id));
    assert!(!ctx.dynamic_chunks.is_empty());

    // Summarizer path: produce a summary of the combined journey.
    let combined_text = format!("{} {} {}", ch1.text, ch2.text, ch4.text);
    let mut combined_chunk = Chunk::raw_leaf(combined_text);
    combined_chunk.order_in_parent = 0;
    let summarizer = ExtractiveBM25Summarizer::new(2);
    let summary = summarizer.summarize(&combined_chunk).await.unwrap();
    assert!(!summary.is_empty(), "summary should not be empty");

    // Rendered prompt surface should include the query and the current chunk.
    let rendered = compiler.render(&ctx, None).await.unwrap();
    assert!(rendered.contains("Alice"));
    assert!(rendered.contains("Chapter 3"));
    assert!(rendered.ends_with("Alice road forest"));
}
