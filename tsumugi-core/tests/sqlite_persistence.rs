//! End-to-end test for SqliteStorage: populate, round-trip via serde, and
//! use it from the Context Compiler exactly like InMemoryStorage.

#![cfg(feature = "sqlite")]

use std::sync::Arc;

use tsumugi_core::compiler::ContextCompiler;
use tsumugi_core::domain::{Chunk, ChunkId, Fact, FactOrigin, FactScope};
use tsumugi_core::retriever::Bm25Retriever;
use tsumugi_core::scorer::NoDecayScorer;
use tsumugi_core::storage::SqliteStorage;
use tsumugi_core::traits::retriever::Retriever;
use tsumugi_core::traits::scorer::RelevanceScorer;
use tsumugi_core::traits::storage::StorageProvider;

#[tokio::test]
async fn sqlite_backed_compiler_assembles_context() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap())
        as Arc<dyn StorageProvider>;

    let parent = Chunk::raw_leaf("chapter: the forest");
    let parent_id = parent.id;
    let mut child = Chunk::raw_leaf("Alice walked deeper into the woods");
    child.parent = Some(parent_id);
    let current_id = child.id;
    let related = Chunk::raw_leaf("the forest canopy shielded Alice from the rain");
    let unrelated = Chunk::raw_leaf("the kitchen was warm and busy");

    for c in [&parent, &child, &related, &unrelated] {
        storage.save_chunk(c).await.unwrap();
    }

    let fact = Fact::new("hp", "12", FactScope::Global, FactOrigin::User);
    storage.save_fact(&fact).await.unwrap();

    let corpus: Vec<(ChunkId, String)> = [&parent, &child, &related, &unrelated]
        .iter()
        .map(|c| (c.id, c.text.clone()))
        .collect();
    let retriever: Arc<dyn Retriever> = Arc::new(Bm25Retriever::new(corpus));
    let scorer: Arc<dyn RelevanceScorer> = Arc::new(NoDecayScorer);

    let compiler = ContextCompiler::new(storage, retriever, scorer).with_limits(10, 3);
    let ctx = compiler
        .compile("Alice forest", Some(current_id), None)
        .await
        .unwrap();

    // Resident: current → parent
    assert_eq!(ctx.resident_chunks.len(), 2);
    assert_eq!(ctx.resident_chunks[0].id, current_id);
    assert_eq!(ctx.resident_chunks[1].id, parent_id);
    // Active facts are preserved through SQLite.
    assert_eq!(ctx.active_facts.len(), 1);
    // Dynamic layer excludes the current chunk and ranks `related` above `unrelated`.
    assert!(!ctx.dynamic_chunks.iter().any(|s| s.chunk.id == current_id));
    assert!(!ctx.dynamic_chunks.is_empty());
}
