//! DistilBartSummarizer — paper-faithful encoder-decoder abstractive
//! summarizer for the LLM-free stack (Phase 4-γ Step 5).
//!
//! Wraps a HuggingFace Optimum `summarization-with-past` ONNX export of
//! `sshleifer/distilbart-cnn-6-6` (or any compatible BART-family
//! checkpoint) and runs greedy generation with KV-cache reuse:
//!
//! 1. **Encoder forward** (1 pass): tokenize input → run `encoder_model.onnx`
//!    once → cache `last_hidden_state` for the rest of generation.
//! 2. **Initial decoder step** (1 pass): feed `[decoder_start_token_id]`
//!    plus the encoder hidden states into `decoder_model.onnx`. Returns
//!    initial logits and a full set of `present.{layer}.{decoder|encoder}.{key|value}`
//!    tensors which become `past_key_values` for step 3.
//! 3. **Subsequent decoder steps** (N passes, `decoder_with_past_model.onnx`):
//!    feed the most recently sampled token plus cached encoder hidden
//!    states plus previous-step `past_key_values`. Argmax greedily; stop
//!    on `eos_token_id` or `max_output_length`. KV-cache reuse keeps
//!    each step at O(1) attn cost rather than O(n²).
//! 4. **Detokenize** the generated token sequence (special tokens
//!    skipped) and return the trimmed string.
//!
//! ### Why three ONNX graphs
//!
//! Optimum's `summarization-with-past` task emits three graphs because
//! the decoder's initial step has no `past_key_values` to take as input
//! (different graph topology), while subsequent steps consume the prior
//! step's `present.*` outputs as `past_key_values.*` inputs. Conflating
//! them into one graph would force every step to re-attend the encoder
//! hidden states, costing ~5× per-token latency on CPU at the lengths
//! distillbart cares about (~512 input, ~150 output).
//!
//! ### Tensor shapes (BART-large topology, 6 decoder layers)
//!
//! - Encoder I/O:
//!   - `input_ids`: `[1, src_len]` `i64`
//!   - `attention_mask`: `[1, src_len]` `i64`
//!   - `last_hidden_state`: `[1, src_len, d_model]` `f32`
//! - Decoder (no past) I/O:
//!   - `input_ids`: `[1, tgt_len]` `i64` (1 on first step)
//!   - `encoder_hidden_states`: `[1, src_len, d_model]` `f32` (cached)
//!   - `encoder_attention_mask`: `[1, src_len]` `i64` (cached, optional)
//!   - `logits`: `[1, tgt_len, vocab]` `f32`
//!   - `present.{i}.decoder.{key,value}`: `[1, num_heads, tgt_len, head_dim]` `f32`
//!   - `present.{i}.encoder.{key,value}`: `[1, num_heads, src_len, head_dim]` `f32`
//! - Decoder (with past) I/O: same as above plus `past_key_values.{i}.decoder.{key,value}`
//!   `past_key_values.{i}.encoder.{key,value}` inputs from previous-step
//!   `present.*` outputs.
//!
//! ### What this implementation does NOT do
//!
//! - **Beam search**: greedy only. Beam search adds noticeable quality
//!   for summarization (~1-2 ROUGE points) but doubles complexity for
//!   marginal gain in our LLM-free stack scope. Add as a follow-up if
//!   evaluation demands it.
//! - **No-repeat n-gram suppression**: BART-CNN's training distribution
//!   already discourages repetition; the bench harness can detect any
//!   degeneration. Add a post-decode dedup pass if it fires.
//! - **Length penalty / coverage penalty**: `min_output_length` and
//!   `max_output_length` are hard cutoffs. The token-level penalties from
//!   `transformers.BartForConditionalGeneration` aren't ported.
//!
//! ### Without the `onnx` feature
//!
//! Construction succeeds, [`summarize`](Summarizer::summarize) bails
//! with the same "requires `onnx` feature" message used by
//! [`OnnxEmbedding`](crate::providers::OnnxEmbedding) and
//! [`LlmLingua2Compressor`](crate::compressor::LlmLingua2Compressor).

use crate::domain::{Chunk, SummaryMethod};
use crate::traits::summarizer::Summarizer;
use async_trait::async_trait;
use std::path::PathBuf;

#[cfg(feature = "onnx")]
use std::sync::Arc;
#[cfg(feature = "onnx")]
use tokio::sync::OnceCell;

/// BART config defaults for `sshleifer/distilbart-cnn-6-6` (and most
/// BART-CNN family checkpoints). Override at construction time when the
/// model card disagrees.
pub const DEFAULT_BOS_TOKEN_ID: i64 = 0;
pub const DEFAULT_EOS_TOKEN_ID: i64 = 2;
pub const DEFAULT_PAD_TOKEN_ID: i64 = 1;
/// BART uses `</s>` (id=2) as the decoder-start token, not `<s>` (id=0).
/// This is a BART-specific convention and must match what the encoder /
/// decoder ONNX graphs were trained against.
pub const DEFAULT_DECODER_START_TOKEN_ID: i64 = 2;

pub const DEFAULT_MAX_INPUT_LENGTH: usize = 1024;
pub const DEFAULT_MAX_OUTPUT_LENGTH: usize = 142;
pub const DEFAULT_MIN_OUTPUT_LENGTH: usize = 56;

pub struct DistilBartSummarizer {
    pub encoder_path: PathBuf,
    pub decoder_path: PathBuf,
    pub decoder_with_past_path: PathBuf,
    pub tokenizer_path: PathBuf,

    pub max_input_length: usize,
    pub max_output_length: usize,
    pub min_output_length: usize,

    pub bos_token_id: i64,
    pub eos_token_id: i64,
    pub pad_token_id: i64,
    pub decoder_start_token_id: i64,

    #[cfg(feature = "onnx")]
    state: OnceCell<Arc<InferenceState>>,
}

#[cfg(feature = "onnx")]
struct InferenceState {
    encoder: std::sync::Mutex<ort::session::Session>,
    decoder: std::sync::Mutex<ort::session::Session>,
    decoder_with_past: std::sync::Mutex<ort::session::Session>,
    tokenizer: tokenizers::Tokenizer,
    /// Names of past_key_values input slots in the decoder_with_past graph,
    /// in the same order Optimum emits them. Discovered at load time and
    /// cached so we don't re-introspect each step.
    past_input_names: Vec<String>,
    /// Names of present.* output slots in the decoder graphs, ordered to
    /// match `past_input_names` 1:1 (same layer/role/key-or-value).
    present_output_names: Vec<String>,
    /// Whether the decoder graphs accept an `encoder_attention_mask`
    /// input. Some Optimum exports omit it.
    decoder_uses_encoder_attention_mask: bool,
}

impl DistilBartSummarizer {
    /// Construct a summarizer from the four files Optimum's
    /// `summarization-with-past` export produces.
    pub fn new(
        encoder_path: impl Into<PathBuf>,
        decoder_path: impl Into<PathBuf>,
        decoder_with_past_path: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            encoder_path: encoder_path.into(),
            decoder_path: decoder_path.into(),
            decoder_with_past_path: decoder_with_past_path.into(),
            tokenizer_path: tokenizer_path.into(),
            max_input_length: DEFAULT_MAX_INPUT_LENGTH,
            max_output_length: DEFAULT_MAX_OUTPUT_LENGTH,
            min_output_length: DEFAULT_MIN_OUTPUT_LENGTH,
            bos_token_id: DEFAULT_BOS_TOKEN_ID,
            eos_token_id: DEFAULT_EOS_TOKEN_ID,
            pad_token_id: DEFAULT_PAD_TOKEN_ID,
            decoder_start_token_id: DEFAULT_DECODER_START_TOKEN_ID,
            #[cfg(feature = "onnx")]
            state: OnceCell::new(),
        }
    }

    /// Convenience constructor that points at a directory containing the
    /// canonical Optimum filenames (`encoder_model.onnx`,
    /// `decoder_model.onnx`, `decoder_with_past_model.onnx`,
    /// `tokenizer.json`). This is what `download_distilbart.sh` produces.
    pub fn from_dir(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        Self::new(
            dir.join("encoder_model.onnx"),
            dir.join("decoder_model.onnx"),
            dir.join("decoder_with_past_model.onnx"),
            dir.join("tokenizer.json"),
        )
    }

    pub fn with_max_input_length(mut self, n: usize) -> Self {
        self.max_input_length = n;
        self
    }

    pub fn with_max_output_length(mut self, n: usize) -> Self {
        self.max_output_length = n;
        self
    }

    pub fn with_min_output_length(mut self, n: usize) -> Self {
        self.min_output_length = n;
        self
    }

    pub fn with_special_tokens(mut self, bos: i64, eos: i64, pad: i64, decoder_start: i64) -> Self {
        self.bos_token_id = bos;
        self.eos_token_id = eos;
        self.pad_token_id = pad;
        self.decoder_start_token_id = decoder_start;
        self
    }
}

#[async_trait]
impl Summarizer for DistilBartSummarizer {
    async fn summarize(&self, chunk: &Chunk) -> anyhow::Result<String> {
        self.summarize_inner(&chunk.text).await
    }

    fn method(&self) -> SummaryMethod {
        SummaryMethod::DistilBart
    }
}

#[cfg(not(feature = "onnx"))]
impl DistilBartSummarizer {
    async fn summarize_inner(&self, _text: &str) -> anyhow::Result<String> {
        anyhow::bail!(
            "DistilBartSummarizer::summarize requires the `onnx` feature \
             (encoder_path = {:?}). Rebuild with `--features onnx` or use \
             ExtractiveBM25Summarizer / LlmSummarizer instead.",
            self.encoder_path
        )
    }
}

// The ONNX-backed `summarize_inner` and friends live in a sibling module so
// the file stays scannable. See `distilbart_inference.rs` immediately
// following this declaration in `summarizer/mod.rs`.

#[cfg(feature = "onnx")]
mod inference;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_overrides_defaults() {
        let s = DistilBartSummarizer::new(
            "/tmp/enc.onnx",
            "/tmp/dec.onnx",
            "/tmp/dec_past.onnx",
            "/tmp/tok.json",
        )
        .with_max_input_length(256)
        .with_max_output_length(64)
        .with_min_output_length(8)
        .with_special_tokens(10, 20, 30, 40);
        assert_eq!(s.max_input_length, 256);
        assert_eq!(s.max_output_length, 64);
        assert_eq!(s.min_output_length, 8);
        assert_eq!(s.bos_token_id, 10);
        assert_eq!(s.eos_token_id, 20);
        assert_eq!(s.pad_token_id, 30);
        assert_eq!(s.decoder_start_token_id, 40);
    }

    #[test]
    fn from_dir_uses_canonical_filenames() {
        let s = DistilBartSummarizer::from_dir("/models/distilbart");
        assert_eq!(
            s.encoder_path,
            PathBuf::from("/models/distilbart/encoder_model.onnx")
        );
        assert_eq!(
            s.decoder_path,
            PathBuf::from("/models/distilbart/decoder_model.onnx")
        );
        assert_eq!(
            s.decoder_with_past_path,
            PathBuf::from("/models/distilbart/decoder_with_past_model.onnx")
        );
        assert_eq!(
            s.tokenizer_path,
            PathBuf::from("/models/distilbart/tokenizer.json")
        );
    }

    #[test]
    fn method_is_distilbart() {
        let s = DistilBartSummarizer::from_dir("/tmp");
        assert_eq!(s.method(), SummaryMethod::DistilBart);
    }

    #[cfg(not(feature = "onnx"))]
    #[tokio::test]
    async fn summarize_without_onnx_feature_bails() {
        let s = DistilBartSummarizer::from_dir("/tmp");
        let chunk = Chunk::raw_leaf("anything");
        let err = s.summarize(&chunk).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("`onnx` feature"),
            "expected feature-required bail, got: {msg}"
        );
    }

    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn summarize_with_missing_paths_returns_load_error() {
        let s = DistilBartSummarizer::from_dir("/tmp/tsumugi-distilbart-test-does-not-exist");
        let chunk = Chunk::raw_leaf("anything");
        let err = s.summarize(&chunk).await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("encoder")
                || msg.contains("tokenizer")
                || msg.contains("ONNX")
                || msg.contains("does-not-exist"),
            "expected load error, got: {msg}"
        );
    }

    /// 実重みがある場合の summarization smoke。
    /// `TSUMUGI_DISTILBART_DIR` 環境変数が設定されているときだけ走る。
    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn summarize_real_weights_returns_short_string() {
        let dir = match std::env::var("TSUMUGI_DISTILBART_DIR") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_DISTILBART_DIR not set");
                return;
            }
        };
        let s = DistilBartSummarizer::from_dir(dir).with_max_output_length(80);
        let mut chunk = Chunk::raw_leaf("");
        // CNN/DM-style article. distilbart-cnn-6-6 was fine-tuned on this
        // distribution so a coherent multi-sentence summary is the
        // expected output.
        chunk.text =
            "The Apollo 11 mission successfully landed astronauts Neil Armstrong and Buzz \
             Aldrin on the Moon on July 20, 1969. Their lunar module, Eagle, touched down \
             in the Sea of Tranquility while Michael Collins remained in lunar orbit aboard \
             the command module. Armstrong stepped onto the lunar surface and declared, \
             'That's one small step for man, one giant leap for mankind.' The astronauts \
             collected 47.5 pounds of lunar samples, deployed scientific instruments, and \
             planted an American flag. They returned safely to Earth on July 24, 1969, \
             splashing down in the Pacific Ocean."
                .to_string();
        let summary = s.summarize(&chunk).await.unwrap();
        assert!(!summary.is_empty(), "expected non-empty summary");
        assert!(
            summary.split_whitespace().count() < chunk.text.split_whitespace().count(),
            "summary should be shorter than input: {summary}"
        );
    }
}
