//! SelectiveContextCompressor — sentence-level self-information approximation.
//!
//! Real SelectiveContext (Li, 2023) drops low-self-information *tokens* using
//! a decoder LM's log-probabilities. Without an in-process decoder we
//! approximate at the *sentence* level: score each sentence by BM25 against
//! the rest of the document, keep enough top-scoring sentences (in original
//! order) to fit the budget, preserving any `preserve_tail_tokens` tail
//! verbatim.
//!
//! This is weaker than the paper (coarser granularity) but:
//! - is deterministic,
//! - requires no external model,
//! - composes with the rest of the pipeline today,
//! - is swap-compatible with an ML-backed version in Phase 3+.

use crate::retriever::{Tokenizer, WhitespaceTokenizer};
use crate::traits::compressor::{CompressionHint, PromptCompressor};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct SelectiveContextCompressor;

fn split_sentences(text: &str) -> Vec<String> {
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

fn score_sentence(tokens: &[String], df: &HashMap<&str, u32>, n_docs: f32, avg_len: f32) -> f32 {
    // BM25-ish self-information: rare tokens contribute more, short sentences
    // with many rare tokens score highest.
    let k1 = 1.2f32;
    let b = 0.75f32;
    let dl = tokens.len() as f32;
    let mut tf_counts: HashMap<&str, u32> = HashMap::new();
    for t in tokens {
        *tf_counts.entry(t.as_str()).or_default() += 1;
    }
    tf_counts
        .iter()
        .map(|(t, tf)| {
            let n_q = *df.get(t).unwrap_or(&0) as f32;
            let idf = ((n_docs - n_q + 0.5) / (n_q + 0.5) + 1.0).ln();
            let norm = 1.0 - b + b * (dl / avg_len.max(1.0));
            let tf = *tf as f32;
            idf * (tf * (k1 + 1.0)) / (tf + k1 * norm)
        })
        .sum()
}

#[async_trait]
impl PromptCompressor for SelectiveContextCompressor {
    async fn compress(&self, prompt: &str, hint: CompressionHint) -> anyhow::Result<String> {
        let sentences = split_sentences(prompt);
        if sentences.is_empty() {
            return Ok(String::new());
        }

        // Token counts per sentence (whitespace approximation).
        let tokenizer = WhitespaceTokenizer;
        let token_sets: Vec<Vec<String>> =
            sentences.iter().map(|s| tokenizer.tokenize(s)).collect();
        let sentence_token_counts: Vec<usize> = token_sets.iter().map(|t| t.len()).collect();
        let total: usize = sentence_token_counts.iter().sum();
        if total <= hint.target_budget_tokens as usize {
            return Ok(prompt.to_string());
        }

        // Preserve the tail verbatim up to `preserve_tail_tokens`.
        let mut tail_kept: Vec<usize> = Vec::new();
        let mut tail_budget = hint.preserve_tail_tokens as usize;
        for i in (0..sentences.len()).rev() {
            if tail_budget == 0 {
                break;
            }
            let cost = sentence_token_counts[i];
            if cost == 0 {
                continue;
            }
            if cost > tail_budget {
                break;
            }
            tail_budget -= cost;
            tail_kept.push(i);
        }
        tail_kept.reverse();
        let tail_tokens_used: usize = tail_kept.iter().map(|i| sentence_token_counts[*i]).sum();
        let head_budget = (hint.target_budget_tokens as usize).saturating_sub(tail_tokens_used);

        // Score non-tail sentences.
        let mut df: HashMap<&str, u32> = HashMap::new();
        let n_docs = token_sets.len() as f32;
        for toks in &token_sets {
            let mut seen = std::collections::HashSet::new();
            for t in toks {
                if seen.insert(t.as_str()) {
                    *df.entry(t.as_str()).or_default() += 1;
                }
            }
        }
        let avg_len: f32 = token_sets.iter().map(|t| t.len() as f32).sum::<f32>() / n_docs;

        let mut candidates: Vec<(usize, f32)> = (0..sentences.len())
            .filter(|i| !tail_kept.contains(i))
            .map(|i| (i, score_sentence(&token_sets[i], &df, n_docs, avg_len)))
            .collect();
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut head_kept: Vec<usize> = Vec::new();
        let mut used = 0usize;
        for (idx, _) in candidates {
            let cost = sentence_token_counts[idx];
            if used + cost > head_budget {
                continue;
            }
            head_kept.push(idx);
            used += cost;
            if used >= head_budget {
                break;
            }
        }

        let mut keep: Vec<usize> = head_kept.into_iter().chain(tail_kept).collect();
        keep.sort_unstable();
        keep.dedup();

        if keep.is_empty() {
            // Budget too small to hold even one sentence — return an ellipsis
            // followed by the tail, following TruncateCompressor's behavior.
            return Ok("…".to_string());
        }

        let out = keep
            .iter()
            .map(|i| sentences[*i].clone())
            .collect::<Vec<_>>()
            .join(" ");
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn under_budget_is_noop() {
        let c = SelectiveContextCompressor;
        let p = "one. two. three.";
        let out = c.compress(p, CompressionHint::new(100, 10)).await.unwrap();
        assert_eq!(out, p);
    }

    #[tokio::test]
    async fn over_budget_keeps_high_signal_and_tail() {
        let c = SelectiveContextCompressor;
        let p = "Alice met Bob. Nothing happened. Charlie arrived. End of chapter.";
        let out = c.compress(p, CompressionHint::new(6, 2)).await.unwrap();
        assert!(out.contains("End of chapter"), "tail preserved, got: {out}");
        // Total output should fit budget + tail.
        assert!(out.split_whitespace().count() <= 8, "got: {out}");
    }

    #[tokio::test]
    async fn empty_input() {
        let c = SelectiveContextCompressor;
        let out = c.compress("", CompressionHint::new(10, 2)).await.unwrap();
        assert_eq!(out, "");
    }
}
