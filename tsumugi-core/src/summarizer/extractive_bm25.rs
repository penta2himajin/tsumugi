//! ExtractiveBM25Summarizer — Tier 1 extractive summarizer.
//!
//! Picks the top-N sentences by BM25 self-similarity (each sentence scored
//! against the rest of the chunk as the corpus). Deterministic and requires
//! no external model — suitable for fast ingestion paths.

use crate::domain::{Chunk, SummaryMethod};
use crate::retriever::{Tokenizer, WhitespaceTokenizer};
use crate::traits::summarizer::Summarizer;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct ExtractiveBM25Summarizer {
    pub max_sentences: usize,
}

impl ExtractiveBM25Summarizer {
    pub fn new(max_sentences: usize) -> Self {
        Self {
            max_sentences: max_sentences.max(1),
        }
    }
}

impl Default for ExtractiveBM25Summarizer {
    fn default() -> Self {
        Self::new(3)
    }
}

fn split_sentences(text: &str) -> Vec<String> {
    // Pragmatic ASCII/CJK-aware split on `.`, `!`, `?`, `。`, `！`, `？`.
    let mut out = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        buf.push(ch);
        if matches!(ch, '.' | '!' | '?' | '。' | '！' | '？') {
            let trimmed = buf.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
            buf.clear();
        }
    }
    let trailing = buf.trim();
    if !trailing.is_empty() {
        out.push(trailing.to_string());
    }
    out
}

#[async_trait]
impl Summarizer for ExtractiveBM25Summarizer {
    async fn summarize(&self, chunk: &Chunk) -> anyhow::Result<String> {
        let sentences = split_sentences(&chunk.text);
        if sentences.is_empty() {
            return Ok(String::new());
        }
        if sentences.len() <= self.max_sentences {
            return Ok(sentences.join(" "));
        }

        let tokenizer = WhitespaceTokenizer;
        let token_sets: Vec<Vec<String>> =
            sentences.iter().map(|s| tokenizer.tokenize(s)).collect();

        // Document frequency across sentences (treating each as a doc).
        let mut df: HashMap<&str, u32> = HashMap::new();
        for toks in &token_sets {
            let mut seen = std::collections::HashSet::new();
            for t in toks {
                if seen.insert(t.as_str()) {
                    *df.entry(t.as_str()).or_default() += 1;
                }
            }
        }
        let n = sentences.len() as f32;
        let avg_len: f32 = token_sets.iter().map(|t| t.len() as f32).sum::<f32>() / n;

        // Score each sentence as sum over tokens of BM25 weight wrt the chunk.
        let k1 = 1.2f32;
        let b = 0.75f32;
        let mut scored: Vec<(usize, f32)> = token_sets
            .iter()
            .enumerate()
            .map(|(i, toks)| {
                let dl = toks.len() as f32;
                let mut tf_counts: HashMap<&str, u32> = HashMap::new();
                for t in toks {
                    *tf_counts.entry(t.as_str()).or_default() += 1;
                }
                let score: f32 = tf_counts
                    .iter()
                    .map(|(t, tf)| {
                        let n_q = *df.get(t).unwrap_or(&0) as f32;
                        let idf = ((n - n_q + 0.5) / (n_q + 0.5) + 1.0).ln();
                        let norm = 1.0 - b + b * (dl / avg_len.max(1.0));
                        let tf = *tf as f32;
                        idf * (tf * (k1 + 1.0)) / (tf + k1 * norm)
                    })
                    .sum();
                (i, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Preserve original sentence order for the top N.
        let mut keep: Vec<usize> = scored
            .iter()
            .take(self.max_sentences)
            .map(|(i, _)| *i)
            .collect();
        keep.sort_unstable();
        Ok(keep
            .iter()
            .map(|i| sentences[*i].clone())
            .collect::<Vec<_>>()
            .join(" "))
    }

    fn method(&self) -> SummaryMethod {
        SummaryMethod::ExtractiveBM25
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn short_text_returns_as_is() {
        let mut c = Chunk::raw_leaf("hello");
        c.text = "Hello world.".into();
        let out = ExtractiveBM25Summarizer::new(3)
            .summarize(&c)
            .await
            .unwrap();
        assert_eq!(out, "Hello world.");
    }

    #[tokio::test]
    async fn selects_top_sentences_in_order() {
        let mut c = Chunk::raw_leaf("x");
        c.text =
            "The sword gleamed. Nothing else happened. The hero grasped the sword firmly. End."
                .into();
        let out = ExtractiveBM25Summarizer::new(2)
            .summarize(&c)
            .await
            .unwrap();
        // Top-2 by BM25 IDF weighting: the longer information-dense sentence
        // ("hero grasped sword firmly") is selected along with another
        // unique-token-heavy sentence. We don't pin the exact pick to avoid
        // fragility on IDF rounding, but the summary must be shorter than
        // the original and include the highest-signal sentence.
        assert!(out.contains("grasped"), "got: {out}");
        let sentence_count = out.matches(['.', '!', '?']).count();
        assert_eq!(sentence_count, 2, "expected 2 sentences, got: {out}");
    }
}
