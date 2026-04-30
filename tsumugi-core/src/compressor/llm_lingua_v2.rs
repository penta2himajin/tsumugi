//! LlmLingua2Compressor — paper-faithful LLMLingua-2 (Pan et al., 2024) impl.
//!
//! Per-token binary classifier that predicts keep/discard for each subword
//! token of a multilingual BERT-base graph. Works against the released
//! `microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank` weights
//! (110M, Apache-2.0, ONNX-exportable via Optimum) but accepts any token
//! classifier with `[batch, seq, num_classes]` logits output and a
//! tokenizer.json compatible with `tokenizers::Tokenizer::from_file`.
//!
//! Compression flow:
//! 1. Tokenize the prompt with the model's tokenizer (special tokens added).
//! 2. Forward through the ONNX session to obtain per-token logits.
//! 3. Softmax over the class axis → keep probability per subword token.
//! 4. Convert the user's whitespace-token budget into a subword budget by
//!    measuring how many whitespace tokens the original input has and
//!    proportionally scaling the subword count.
//! 5. Force-keep the suffix tokens corresponding to `preserve_tail_tokens`
//!    whitespace tokens (typical RAG layout: question lives at the end and
//!    must survive verbatim).
//! 6. Of the remaining tokens, take the top-K by keep probability where K
//!    is whatever fills the remaining subword budget.
//! 7. Sort kept indices back to original order and decode through the
//!    tokenizer's `decode` (skip_special_tokens = true), which concatenates
//!    WordPiece subwords correctly.
//!
//! Without the `onnx` feature, the type compiles to a stub that returns a
//! clear error from `compress`. This mirrors the pattern used by
//! `OnnxEmbedding` so consumers can declare a `PromptCompressor` of type
//! `LlmLingua2Compressor` even in default-feature builds.

use crate::traits::compressor::{CompressionHint, PromptCompressor};
use async_trait::async_trait;
use std::path::PathBuf;

#[cfg(feature = "onnx")]
use std::sync::Arc;
#[cfg(feature = "onnx")]
use tokio::sync::OnceCell;

/// LLMLingua-2-style per-token classifier compressor.
///
/// Construct with the path to a HF `tokenizer.json` and an exported ONNX
/// model that emits `[batch, seq, num_classes]` logits. Defaults assume
/// the canonical `microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank`
/// graph, where `keep_class_index = 1` (label `"1"` = preserve).
pub struct LlmLingua2Compressor {
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    /// Index of the "keep" class within the per-token logits axis.
    /// 0 or 1 depending on the model's `id2label` mapping.
    pub keep_class_index: usize,
    pub max_sequence_length: usize,
    /// Lower bound for the subword keep ratio — guards against degenerate
    /// budgets that would compress everything away. 0.05 = always keep at
    /// least 5% of subwords.
    pub min_keep_ratio: f32,
    #[cfg(feature = "onnx")]
    state: OnceCell<Arc<InferenceState>>,
}

#[cfg(feature = "onnx")]
struct InferenceState {
    // ort 2.0.0-rc.10 の Session::run は &mut self を要求するため、
    // OnnxEmbedding と同様 std::sync::Mutex で直列化する。CPU 推論
    // (300-1500ms / 10K-tok prompt) が支配的で lock 競合は無視できる。
    session: std::sync::Mutex<ort::session::Session>,
    tokenizer: tokenizers::Tokenizer,
    needs_token_type_ids: bool,
}

impl LlmLingua2Compressor {
    pub fn new(model_path: impl Into<PathBuf>, tokenizer_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            tokenizer_path: tokenizer_path.into(),
            keep_class_index: 1,
            max_sequence_length: 512,
            min_keep_ratio: 0.05,
            #[cfg(feature = "onnx")]
            state: OnceCell::new(),
        }
    }

    pub fn with_keep_class_index(mut self, idx: usize) -> Self {
        self.keep_class_index = idx;
        self
    }

    pub fn with_max_sequence_length(mut self, n: usize) -> Self {
        self.max_sequence_length = n;
        self
    }

    pub fn with_min_keep_ratio(mut self, r: f32) -> Self {
        self.min_keep_ratio = r;
        self
    }
}

#[async_trait]
impl PromptCompressor for LlmLingua2Compressor {
    async fn compress(&self, prompt: &str, hint: CompressionHint) -> anyhow::Result<String> {
        // Cheap shared guard — applies in both feature paths so unit tests
        // that don't touch the ONNX runtime can still exercise the no-op
        // behaviour.
        let original_word_count = prompt.split_whitespace().count();
        if original_word_count <= hint.target_budget_tokens as usize {
            return Ok(prompt.to_string());
        }
        self.compress_with_classifier(prompt, hint, original_word_count)
            .await
    }
}

#[cfg(not(feature = "onnx"))]
impl LlmLingua2Compressor {
    async fn compress_with_classifier(
        &self,
        _prompt: &str,
        _hint: CompressionHint,
        _original_word_count: usize,
    ) -> anyhow::Result<String> {
        anyhow::bail!(
            "LlmLingua2Compressor::compress requires the `onnx` feature \
             (model_path = {:?}). Rebuild with `--features onnx` or use \
             TruncateCompressor / LlmDelegationCompressor instead.",
            self.model_path
        )
    }
}

#[cfg(feature = "onnx")]
impl LlmLingua2Compressor {
    async fn compress_with_classifier(
        &self,
        prompt: &str,
        hint: CompressionHint,
        original_word_count: usize,
    ) -> anyhow::Result<String> {
        let state = self.ensure_state().await?;
        let prompt_owned = prompt.to_string();
        let max_seq = self.max_sequence_length;
        let keep_idx = self.keep_class_index;
        let min_ratio = self.min_keep_ratio;
        let target_words = hint.target_budget_tokens as usize;
        let preserve_tail_words = hint.preserve_tail_tokens as usize;
        let state_for_blocking = Arc::clone(&state);
        tokio::task::spawn_blocking(move || {
            run_compression(
                &state_for_blocking,
                &prompt_owned,
                max_seq,
                keep_idx,
                min_ratio,
                target_words,
                preserve_tail_words,
                original_word_count,
            )
        })
        .await
        .map_err(|e| anyhow::anyhow!("LlmLingua2Compressor inference task panicked: {e}"))?
    }

    async fn ensure_state(&self) -> anyhow::Result<Arc<InferenceState>> {
        let model_path = self.model_path.clone();
        let tokenizer_path = self.tokenizer_path.clone();
        let cell = &self.state;
        let arc = cell
            .get_or_try_init(|| async move {
                tokio::task::spawn_blocking(move || load_state(&model_path, &tokenizer_path))
                    .await
                    .map_err(|e| anyhow::anyhow!("LlmLingua2Compressor init task panicked: {e}"))?
                    .map(Arc::new)
            })
            .await?;
        Ok(Arc::clone(arc))
    }
}

#[cfg(feature = "onnx")]
fn load_state(model_path: &PathBuf, tokenizer_path: &PathBuf) -> anyhow::Result<InferenceState> {
    use anyhow::Context;

    let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path)
        .map_err(|e| anyhow::anyhow!("failed to load tokenizer from {tokenizer_path:?}: {e}"))?;
    let session = ort::session::Session::builder()
        .context("failed to create ort SessionBuilder")?
        .commit_from_file(model_path)
        .with_context(|| format!("failed to load ONNX model from {model_path:?}"))?;
    let needs_token_type_ids = session.inputs.iter().any(|i| i.name == "token_type_ids");
    Ok(InferenceState {
        session: std::sync::Mutex::new(session),
        tokenizer,
        needs_token_type_ids,
    })
}

#[cfg(feature = "onnx")]
#[allow(clippy::too_many_arguments)]
fn run_compression(
    state: &InferenceState,
    prompt: &str,
    max_seq: usize,
    keep_class_index: usize,
    min_keep_ratio: f32,
    target_words: usize,
    preserve_tail_words: usize,
    original_word_count: usize,
) -> anyhow::Result<String> {
    use ndarray::Array2;
    use ort::value::Value;

    // Tokenize once with special tokens. We keep [CLS] at position 0 and
    // [SEP] at position seq-1 so that the model's training distribution
    // matches exactly. Keep probabilities for those positions are dropped
    // before token selection.
    let encoding = state
        .tokenizer
        .encode(prompt, true)
        .map_err(|e| anyhow::anyhow!("tokenizer encode failed: {e}"))?;
    let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let mut mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .map(|&x| x as i64)
        .collect();
    let special_mask: Vec<bool> = encoding
        .get_special_tokens_mask()
        .iter()
        .map(|&x| x != 0)
        .collect();
    if ids.len() > max_seq {
        ids.truncate(max_seq);
        mask.truncate(max_seq);
    }
    let seq_len = ids.len();
    if seq_len == 0 {
        return Ok(prompt.to_string());
    }
    let special_mask = if special_mask.len() >= seq_len {
        special_mask[..seq_len].to_vec()
    } else {
        let mut v = vec![false; seq_len];
        for (i, &b) in special_mask.iter().enumerate().take(seq_len) {
            v[i] = b;
        }
        v
    };

    let input_ids = Value::from_array(Array2::<i64>::from_shape_vec((1, seq_len), ids.clone())?)?;
    let attention_mask =
        Value::from_array(Array2::<i64>::from_shape_vec((1, seq_len), mask.clone())?)?;

    // Run inference inside a tight scope so the session lock + outputs
    // borrow drop before we start ranking. We copy logits into a local
    // Vec<f32> immediately to release the ort buffers.
    let (logits, num_classes) = {
        let mut session = state
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("ort session mutex poisoned: {e}"))?;
        let outputs = if state.needs_token_type_ids {
            let token_type_ids = Value::from_array(Array2::<i64>::zeros((1, seq_len)))?;
            session.run(ort::inputs![
                "input_ids" => input_ids,
                "attention_mask" => attention_mask,
                "token_type_ids" => token_type_ids,
            ])?
        } else {
            session.run(ort::inputs![
                "input_ids" => input_ids,
                "attention_mask" => attention_mask,
            ])?
        };
        let (_name, first_output) = outputs
            .iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("ONNX session returned no outputs"))?;
        let (shape, data) = first_output
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("failed to extract f32 tensor from output: {e}"))?;
        let dims = shape.as_ref();
        if dims.len() != 3 {
            anyhow::bail!(
                "expected rank-3 output [1, seq, classes], got shape {:?}",
                dims
            );
        }
        if dims[0] != 1 || dims[1] as usize != seq_len {
            anyhow::bail!(
                "unexpected output shape {:?}: expected [1, {}, classes]",
                dims,
                seq_len
            );
        }
        let num_classes = dims[2] as usize;
        if keep_class_index >= num_classes {
            anyhow::bail!(
                "keep_class_index {} out of range for num_classes {}",
                keep_class_index,
                num_classes
            );
        }
        (data.to_vec(), num_classes)
    };

    // Per-token softmax over the class axis to produce keep probabilities.
    let mut keep_probs = vec![0f32; seq_len];
    for (t, kp) in keep_probs.iter_mut().enumerate().take(seq_len) {
        let base = t * num_classes;
        let mut max_logit = f32::NEG_INFINITY;
        for c in 0..num_classes {
            let v = logits[base + c];
            if v > max_logit {
                max_logit = v;
            }
        }
        let mut sum = 0f32;
        for c in 0..num_classes {
            sum += (logits[base + c] - max_logit).exp();
        }
        let keep_logit = logits[base + keep_class_index];
        *kp = ((keep_logit - max_logit).exp()) / sum;
    }

    // The user budget is in whitespace tokens, but we select subwords.
    // Translate via the input's word count so that the subword keep
    // ratio mirrors the requested word ratio.
    let word_ratio =
        (target_words as f32 / original_word_count.max(1) as f32).clamp(min_keep_ratio, 1.0);
    let non_special_count = special_mask.iter().filter(|&&b| !b).count();
    let target_kept_subwords = ((non_special_count as f32) * word_ratio).round() as usize;
    let target_kept_subwords =
        target_kept_subwords.max(((non_special_count as f32) * min_keep_ratio).ceil() as usize);

    // Identify the suffix subwords that map to `preserve_tail_words` whitespace
    // tokens at the end of the original prompt. Because the tokenizer
    // discarded whitespace, we approximate by walking back from the last
    // non-special token until we have collected enough word boundaries.
    let force_keep_set = if preserve_tail_words == 0 {
        std::collections::HashSet::new()
    } else {
        force_keep_suffix_indices(&state.tokenizer, &ids, &special_mask, preserve_tail_words)
    };

    // Rank candidate (non-special, non-forced) subwords by keep_prob.
    let mut candidates: Vec<(usize, f32)> = (0..seq_len)
        .filter(|&t| !special_mask[t] && !force_keep_set.contains(&t))
        .map(|t| (t, keep_probs[t]))
        .collect();
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let force_count = force_keep_set.len();
    let extra_to_keep = target_kept_subwords.saturating_sub(force_count);

    let mut keep_indices: std::collections::HashSet<usize> = force_keep_set;
    for (idx, _) in candidates.iter().take(extra_to_keep) {
        keep_indices.insert(*idx);
    }

    // Reconstruct in original order.
    let kept_token_ids: Vec<u32> = (0..seq_len)
        .filter(|t| keep_indices.contains(t))
        .map(|t| ids[t] as u32)
        .collect();
    if kept_token_ids.is_empty() {
        // Pathological: nothing kept. Fall back to the original prompt to
        // avoid emitting an empty string downstream.
        return Ok(prompt.to_string());
    }

    let decoded = state
        .tokenizer
        .decode(&kept_token_ids, true)
        .map_err(|e| anyhow::anyhow!("tokenizer decode failed: {e}"))?;
    Ok(decoded)
}

/// Walk back from the end of the non-special token range until we cover
/// `n_words` whitespace tokens worth of WordPiece pieces. Subwords starting
/// with `##` are continuation pieces and don't count as a new word.
#[cfg(feature = "onnx")]
fn force_keep_suffix_indices(
    tokenizer: &tokenizers::Tokenizer,
    ids: &[i64],
    special_mask: &[bool],
    n_words: usize,
) -> std::collections::HashSet<usize> {
    let mut out = std::collections::HashSet::new();
    if n_words == 0 {
        return out;
    }
    let mut words_seen: usize = 0;
    for (t, _) in ids.iter().enumerate().rev() {
        if special_mask[t] {
            continue;
        }
        let piece = tokenizer.id_to_token(ids[t] as u32).unwrap_or_default();
        out.insert(t);
        // BERT WordPiece: continuation pieces start with "##". Anything
        // else marks a fresh word boundary. SentencePiece-based models
        // (XLM-R) prefix new words with "▁"; LLMLingua-2-mBERT is BERT
        // not XLM-R, so we use the WordPiece convention.
        if !piece.starts_with("##") {
            words_seen += 1;
            if words_seen >= n_words {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_sets_fields() {
        let c = LlmLingua2Compressor::new("/tmp/m.onnx", "/tmp/t.json")
            .with_keep_class_index(0)
            .with_max_sequence_length(256)
            .with_min_keep_ratio(0.1);
        assert_eq!(c.keep_class_index, 0);
        assert_eq!(c.max_sequence_length, 256);
        assert!((c.min_keep_ratio - 0.1).abs() < 1e-6);
    }

    #[tokio::test]
    async fn under_budget_is_noop() {
        // No ML runtime touched — short input shorts the under-budget guard
        // before any model load. Works in default-feature builds too.
        let c = LlmLingua2Compressor::new("/tmp/m.onnx", "/tmp/t.json");
        let out = c
            .compress("one two three", CompressionHint::new(10, 2))
            .await
            .unwrap();
        assert_eq!(out, "one two three");
    }

    #[cfg(not(feature = "onnx"))]
    #[tokio::test]
    async fn over_budget_without_onnx_feature_bails() {
        let c = LlmLingua2Compressor::new("/tmp/m.onnx", "/tmp/t.json");
        let long_prompt = (1..=30)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let err = c
            .compress(&long_prompt, CompressionHint::new(10, 2))
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("`onnx` feature"),
            "expected feature-required bail, got: {msg}"
        );
    }

    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn over_budget_with_missing_paths_returns_load_error() {
        let c = LlmLingua2Compressor::new(
            "/tmp/tsumugi-llmlingua2-test-does-not-exist.onnx",
            "/tmp/tsumugi-llmlingua2-test-does-not-exist.json",
        );
        let long_prompt = (1..=30)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let err = c
            .compress(&long_prompt, CompressionHint::new(10, 2))
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("tokenizer") || msg.contains("ONNX") || msg.contains("does-not-exist"),
            "expected load error from real ort path, got: {msg}"
        );
    }

    /// 実重みがある場合の compression smoke。`TSUMUGI_LLMLINGUA2_MODEL_PATH`
    /// と `TSUMUGI_LLMLINGUA2_TOKENIZER_PATH` の両方が設定されている
    /// ときだけ走る。default の cargo test では skip。
    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn compress_real_weights_shrinks_prompt() {
        let model_path = match std::env::var("TSUMUGI_LLMLINGUA2_MODEL_PATH") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_LLMLINGUA2_MODEL_PATH not set");
                return;
            }
        };
        let tokenizer_path = match std::env::var("TSUMUGI_LLMLINGUA2_TOKENIZER_PATH") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_LLMLINGUA2_TOKENIZER_PATH not set");
                return;
            }
        };

        let c = LlmLingua2Compressor::new(model_path, tokenizer_path);
        let prompt = "The quick brown fox jumps over the lazy dog. \
            Meanwhile, the rain in Spain falls mainly on the plain. \
            Apollo 11 landed on the Moon on July 20, 1969 at 20:17 UTC. \
            What was the date of the Moon landing?";
        let original_words = prompt.split_whitespace().count();
        let target_budget = (original_words / 2) as u32;
        let out = c
            .compress(prompt, CompressionHint::new(target_budget, 4))
            .await
            .unwrap();
        let out_words = out.split_whitespace().count();
        assert!(
            out_words < original_words,
            "expected compression: original={} got={}",
            original_words,
            out_words
        );
        // preserve_tail_tokens = 4 should keep the question words at the end.
        assert!(
            out.to_ascii_lowercase().contains("moon")
                || out.to_ascii_lowercase().contains("landing"),
            "expected suffix 'Moon landing' to survive, got: {out}"
        );
    }
}
