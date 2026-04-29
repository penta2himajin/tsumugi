//! Context Compiler — assembles a `CompiledContext` from storage, retriever,
//! scorer, and optional classifier / compressor / summarizer components.
//!
//! The compiler separates two layers:
//! - **Resident layer**: always-included chunks (current chunk + its
//!   ancestors) and active facts. Cheap to assemble; no retrieval cost.
//! - **Dynamic layer**: retrieval-driven candidates rescored by the
//!   `RelevanceScorer`. This is where the hot path lives.
//!
//! Optional hooks let products inject query classification and prompt
//! compression without forking the pipeline.

use crate::domain::{Chunk, ChunkId, Fact, SourceLocationValue};
use crate::traits::{
    classifier::{QueryClass, QueryClassifier},
    compressor::{CompressionHint, PromptCompressor},
    retriever::Retriever,
    scorer::{RelevanceScorer, ScoringContext},
    storage::StorageProvider,
};
use chrono::Utc;
use std::sync::Arc;

/// Output of the compile step. Keeps the two layers separate so downstream
/// prompt builders can stitch them in the order the product needs.
#[derive(Debug)]
pub struct CompiledContext {
    pub query: String,
    pub query_class: Option<QueryClass>,
    pub resident_chunks: Vec<Chunk>,
    pub active_facts: Vec<Fact>,
    pub dynamic_chunks: Vec<ScoredChunk>,
}

#[derive(Clone, Debug)]
pub struct ScoredChunk {
    pub chunk: Chunk,
    pub score: f32,
}

/// Compiler configuration. Parameters are moved into the compiler so callers
/// can reuse it across queries.
pub struct ContextCompiler {
    pub storage: Arc<dyn StorageProvider>,
    pub retriever: Arc<dyn Retriever>,
    pub scorer: Arc<dyn RelevanceScorer>,
    pub classifier: Option<Arc<dyn QueryClassifier>>,
    pub compressor: Option<Arc<dyn PromptCompressor>>,
    /// Retrieval pool size before scorer reranking.
    pub retrieval_top_k: usize,
    /// How many dynamic chunks to keep after reranking.
    pub dynamic_top_k: usize,
}

impl ContextCompiler {
    pub fn new(
        storage: Arc<dyn StorageProvider>,
        retriever: Arc<dyn Retriever>,
        scorer: Arc<dyn RelevanceScorer>,
    ) -> Self {
        Self {
            storage,
            retriever,
            scorer,
            classifier: None,
            compressor: None,
            retrieval_top_k: 32,
            dynamic_top_k: 8,
        }
    }

    pub fn with_classifier(mut self, classifier: Arc<dyn QueryClassifier>) -> Self {
        self.classifier = Some(classifier);
        self
    }

    pub fn with_compressor(mut self, compressor: Arc<dyn PromptCompressor>) -> Self {
        self.compressor = Some(compressor);
        self
    }

    pub fn with_limits(mut self, retrieval_top_k: usize, dynamic_top_k: usize) -> Self {
        self.retrieval_top_k = retrieval_top_k;
        self.dynamic_top_k = dynamic_top_k;
        self
    }

    /// Compile context for a query anchored at `current_chunk_id`.
    pub async fn compile(
        &self,
        query: &str,
        current_chunk_id: Option<ChunkId>,
        current_location: Option<&SourceLocationValue>,
    ) -> anyhow::Result<CompiledContext> {
        // 0. Query classification (optional).
        let query_class = match &self.classifier {
            Some(c) => Some(c.classify(query).await?),
            None => None,
        };

        // 1. Resident layer: current chunk and its parent chain.
        let mut resident_chunks = Vec::new();
        if let Some(id) = current_chunk_id {
            let mut cursor = Some(id);
            while let Some(next_id) = cursor {
                match self.storage.load_chunk(next_id).await {
                    Ok(c) => {
                        cursor = c.parent;
                        resident_chunks.push(c);
                    }
                    Err(_) => break,
                }
            }
        }

        // 2. Active facts (non-superseded).
        let mut active_facts = Vec::new();
        for fact_id in self.storage.list_facts().await? {
            if let Ok(fact) = self.storage.load_fact(fact_id).await {
                if fact.is_active() {
                    active_facts.push(fact);
                }
            }
        }

        // 3. Dynamic layer: retrieve → rescore → top-k.
        let hits = self.retriever.retrieve(query, self.retrieval_top_k).await?;
        let now = Utc::now();
        let current_chunk_order = if let Some(Some(c)) =
            current_chunk_id.map(|id| resident_chunks.iter().find(|c| c.id == id).cloned())
        {
            Some(c.order_in_parent)
        } else {
            None
        };

        let mut scored = Vec::with_capacity(hits.len());
        for hit in &hits {
            if Some(hit.chunk_id) == current_chunk_id {
                continue;
            }
            let chunk = match self.storage.load_chunk(hit.chunk_id).await {
                Ok(c) => c,
                Err(_) => continue,
            };
            let ctx = ScoringContext {
                current_chunk_id,
                current_time: now,
                current_order: current_chunk_order,
                current_location,
                retrieval_hit: Some(hit),
            };
            let score = self.scorer.score(&chunk, &ctx);
            scored.push(ScoredChunk { chunk, score });
        }
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(self.dynamic_top_k);

        let context = CompiledContext {
            query: query.to_string(),
            query_class,
            resident_chunks,
            active_facts,
            dynamic_chunks: scored,
        };

        Ok(context)
    }

    /// Render a CompiledContext into a single prompt string, optionally
    /// running it through the configured `PromptCompressor` to fit a budget.
    pub async fn render(
        &self,
        context: &CompiledContext,
        budget: Option<CompressionHint>,
    ) -> anyhow::Result<String> {
        let mut buf = String::new();
        for c in context.resident_chunks.iter().rev() {
            buf.push_str(&c.text);
            buf.push_str("\n\n");
        }
        if !context.active_facts.is_empty() {
            buf.push_str("## Facts\n");
            for f in &context.active_facts {
                buf.push_str(&format!("- {}: {}\n", f.key, f.value));
            }
            buf.push('\n');
        }
        if !context.dynamic_chunks.is_empty() {
            buf.push_str("## Related\n");
            for sc in &context.dynamic_chunks {
                buf.push_str(&sc.chunk.text);
                buf.push_str("\n\n");
            }
        }
        buf.push_str("## Query\n");
        buf.push_str(&context.query);

        if let (Some(hint), Some(comp)) = (budget, &self.compressor) {
            return comp.compress(&buf, hint).await;
        }
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Fact, FactOrigin, FactScope};
    use crate::retriever::Bm25Retriever;
    use crate::scorer::NoDecayScorer;
    use crate::storage::InMemoryStorage;

    #[tokio::test]
    async fn compile_assembles_resident_and_dynamic_layers() {
        let storage: Arc<dyn StorageProvider> = Arc::new(InMemoryStorage::new());

        let parent = Chunk::raw_leaf("CHAPTER 1 root");
        let parent_id = parent.id;
        let mut child = Chunk::raw_leaf("the hero drew a sword");
        child.parent = Some(parent_id);
        let child_id = child.id;
        let unrelated = Chunk::raw_leaf("the weather was nice and calm");
        let related = Chunk::raw_leaf("a sword forged in the northern hills by a hero");
        let related_id = related.id;

        for c in [&parent, &child, &unrelated, &related] {
            storage.save_chunk(c).await.unwrap();
        }

        let fact = Fact::new("hp", "12", FactScope::Global, FactOrigin::User);
        storage.save_fact(&fact).await.unwrap();

        let corpus: Vec<(ChunkId, String)> = vec![
            (parent.id, parent.text.clone()),
            (child.id, child.text.clone()),
            (unrelated.id, unrelated.text.clone()),
            (related.id, related.text.clone()),
        ];
        let retriever: Arc<dyn Retriever> = Arc::new(Bm25Retriever::new(corpus));
        let scorer: Arc<dyn RelevanceScorer> = Arc::new(NoDecayScorer);

        let compiler = ContextCompiler::new(storage, retriever, scorer).with_limits(10, 5);

        let ctx = compiler
            .compile("sword hero", Some(child_id), None)
            .await
            .unwrap();
        // Resident: child → parent (2 levels)
        assert_eq!(ctx.resident_chunks.len(), 2);
        assert_eq!(ctx.resident_chunks[0].id, child_id);
        assert_eq!(ctx.resident_chunks[1].id, parent_id);
        // Active facts include our fact
        assert_eq!(ctx.active_facts.len(), 1);
        // Dynamic chunks: exclude current, rank by BM25
        assert!(!ctx.dynamic_chunks.iter().any(|s| s.chunk.id == child_id));
        assert!(ctx.dynamic_chunks.iter().any(|s| s.chunk.id == related_id));

        // Render smoke-test
        let rendered = compiler.render(&ctx, None).await.unwrap();
        assert!(rendered.contains("CHAPTER 1 root"));
        assert!(rendered.contains("sword hero"));
    }
}
