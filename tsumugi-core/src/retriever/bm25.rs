//! BM25 keyword retriever.
//!
//! Operates over an in-memory corpus of `(ChunkId, text)` pairs supplied at
//! construction time. For large corpora, the index can be rebuilt after
//! ingestion batches; streaming updates are Phase 2.

use super::tokenizer::{Tokenizer, WhitespaceTokenizer};
use crate::domain::ChunkId;
use crate::traits::retriever::{RetrievalHit, Retriever};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Bm25Retriever {
    tokenizer: Arc<dyn Tokenizer>,
    docs: Vec<Doc>,
    avgdl: f32,
    df: HashMap<String, u32>,
    k1: f32,
    b: f32,
}

struct Doc {
    id: ChunkId,
    tf: HashMap<String, u32>,
    len: u32,
}

impl Bm25Retriever {
    pub fn new(corpus: impl IntoIterator<Item = (ChunkId, String)>) -> Self {
        Self::with_tokenizer(corpus, Arc::new(WhitespaceTokenizer))
    }

    pub fn with_tokenizer(
        corpus: impl IntoIterator<Item = (ChunkId, String)>,
        tokenizer: Arc<dyn Tokenizer>,
    ) -> Self {
        let mut docs = Vec::new();
        let mut df: HashMap<String, u32> = HashMap::new();
        let mut total_len: u64 = 0;
        for (id, text) in corpus {
            let toks = tokenizer.tokenize(&text);
            let len = toks.len() as u32;
            total_len += len as u64;
            let mut tf: HashMap<String, u32> = HashMap::new();
            for t in &toks {
                *tf.entry(t.clone()).or_default() += 1;
            }
            for t in tf.keys() {
                *df.entry(t.clone()).or_default() += 1;
            }
            docs.push(Doc { id, tf, len });
        }
        let avgdl = if docs.is_empty() {
            0.0
        } else {
            total_len as f32 / docs.len() as f32
        };
        Self {
            tokenizer,
            docs,
            avgdl,
            df,
            k1: 1.2,
            b: 0.75,
        }
    }

    fn score(&self, query_tokens: &[String], doc: &Doc) -> f32 {
        let n = self.docs.len() as f32;
        let mut score = 0.0f32;
        for q in query_tokens {
            let tf = *doc.tf.get(q).unwrap_or(&0) as f32;
            if tf == 0.0 {
                continue;
            }
            let n_q = *self.df.get(q).unwrap_or(&0) as f32;
            // BM25 IDF variant (Robertson-Spärck Jones) with smoothing.
            let idf = ((n - n_q + 0.5) / (n_q + 0.5) + 1.0).ln();
            let dl = doc.len as f32;
            let norm = 1.0 - self.b + self.b * (dl / self.avgdl.max(1.0));
            let denom = tf + self.k1 * norm;
            let term = idf * ((tf * (self.k1 + 1.0)) / denom);
            score += term;
        }
        score
    }
}

#[async_trait]
impl Retriever for Bm25Retriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalHit>> {
        let q_toks = self.tokenizer.tokenize(query);
        if q_toks.is_empty() || self.docs.is_empty() {
            return Ok(vec![]);
        }
        let mut hits: Vec<RetrievalHit> = self
            .docs
            .iter()
            .map(|d| RetrievalHit {
                chunk_id: d.id,
                score: self.score(&q_toks, d),
            })
            .filter(|h| h.score > 0.0)
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(top_k);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bm25_ranks_exact_match_first() {
        let a = ChunkId::new();
        let b = ChunkId::new();
        let c = ChunkId::new();
        let corpus = vec![
            (a, "the quick brown fox".to_string()),
            (b, "lazy dogs sleep all day".to_string()),
            (c, "quick and brown is the fox".to_string()),
        ];
        let r = Bm25Retriever::new(corpus);
        let hits = r.retrieve("quick fox", 10).await.unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|h| h.chunk_id == a));
        assert!(hits[0].score > 0.0);
    }

    #[tokio::test]
    async fn bm25_no_match_yields_empty() {
        let a = ChunkId::new();
        let r = Bm25Retriever::new(vec![(a, "foo bar".to_string())]);
        let hits = r.retrieve("completely unrelated", 10).await.unwrap();
        assert!(hits.is_empty());
    }
}
