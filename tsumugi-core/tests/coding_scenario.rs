//! End-to-end smoke test for the default (core-only) feature set.
//!
//! Mimics a lightweight "コーディング agent" use case (→ `つくも`): chunks
//! carry file source locations, retrieval is BM25, scoring favors files
//! close to the current location.

use std::sync::Arc;

use tsumugi_core::compiler::ContextCompiler;
use tsumugi_core::domain::{Chunk, ChunkId, SourceLocationValue};
use tsumugi_core::providers::MockEmbedding;
use tsumugi_core::retriever::{Bm25Retriever, CosineRetriever, HybridRetriever};
use tsumugi_core::scorer::{CompositeScorer, FileProximityScorer, NoDecayScorer};
use tsumugi_core::storage::InMemoryStorage;
use tsumugi_core::traits::embedding::{EmbeddingProvider, EmbeddingVector};
use tsumugi_core::traits::retriever::Retriever;
use tsumugi_core::traits::scorer::RelevanceScorer;
use tsumugi_core::traits::storage::StorageProvider;

async fn build_corpus() -> (
    Arc<dyn StorageProvider>,
    Vec<(ChunkId, String)>,
    Vec<(ChunkId, EmbeddingVector)>,
    ChunkId,
) {
    let storage: Arc<dyn StorageProvider> = Arc::new(InMemoryStorage::new());
    let embedder = MockEmbedding::new(64);

    let mut records = Vec::new();
    let entries: &[(&str, &str)] = &[
        ("src/parser/mod.rs", "pub mod lexer; pub mod ast;"),
        ("src/parser/lexer.rs", "parse tokens from input stream"),
        ("src/parser/ast.rs", "abstract syntax tree node definitions"),
        (
            "src/ir/lowering.rs",
            "lower ast to ir with constant folding",
        ),
        ("docs/readme.md", "project overview and onboarding guide"),
        (
            "tests/parser_tests.rs",
            "integration tests for lexer and parser",
        ),
    ];

    for (path, text) in entries {
        let chunk = Chunk::raw_leaf(*text).with_source(SourceLocationValue::file(*path));
        storage.save_chunk(&chunk).await.unwrap();
        let embedding = embedder.embed(text).await.unwrap();
        records.push((chunk.id, text.to_string(), embedding));
    }

    let current_id = records[0].0; // src/parser/mod.rs
    let corpus = records
        .iter()
        .map(|(id, text, _)| (*id, text.clone()))
        .collect();
    let embeddings = records.iter().map(|(id, _, v)| (*id, v.clone())).collect();
    (storage, corpus, embeddings, current_id)
}

#[tokio::test]
async fn file_proximity_drives_ranking_on_coding_queries() {
    let (storage, corpus, embeddings, current_id) = build_corpus().await;

    let bm25: Arc<dyn Retriever> = Arc::new(Bm25Retriever::new(corpus));
    let provider = Arc::new(MockEmbedding::new(64));
    let cosine: Arc<dyn Retriever> = Arc::new(CosineRetriever::new(embeddings, provider.clone()));
    let retriever: Arc<dyn Retriever> = Arc::new(HybridRetriever::new(bm25, cosine));

    let scorer: Arc<dyn RelevanceScorer> = Arc::new(
        CompositeScorer::new()
            .add(Arc::new(NoDecayScorer), 1.0)
            .add(Arc::new(FileProximityScorer::new(1.0)), 2.0),
    );

    let compiler = ContextCompiler::new(storage.clone(), retriever, scorer).with_limits(10, 4);
    let current = SourceLocationValue::file("src/parser/mod.rs");
    let ctx = compiler
        .compile("lexer tokens parser", Some(current_id), Some(&current))
        .await
        .unwrap();

    // With FileProximityScorer weighted higher, files under src/parser/ rank
    // above docs/ and tests/.
    let paths: Vec<_> = ctx
        .dynamic_chunks
        .iter()
        .filter_map(|s| s.chunk.source_location.as_ref())
        .filter_map(|sl| match sl {
            SourceLocationValue::File(f) => Some(f.path.clone()),
            _ => None,
        })
        .collect();

    assert!(!paths.is_empty(), "no dynamic chunks returned");
    let top_path = &paths[0];
    assert!(
        top_path.starts_with("src/parser/"),
        "expected a src/parser/ file at the top, got {top_path}; full: {paths:?}"
    );
}
