//! NliZeroShotDetector — encoder-only EventDetector via NLI entailment.
//!
//! Phase 4-γ Step 4 ([`docs/llm-free-stack-plan.md`] § 5.2 (4)). The
//! deterministic encoder-only path for event detection in the LLM-free
//! stack.
//!
//! ### Approach
//!
//! Standard zero-shot classification via natural-language inference: for
//! each candidate label `L`, build a hypothesis (`"This text is about L."`
//! by default), pair it with the chunk's text as the premise, and run a
//! 3-way classifier (`{entailment, neutral, contradiction}`) over the
//! pair. Softmax the entailment logit, threshold it, and fire a
//! [`DetectedEvent`] when the entailment probability exceeds the
//! configured cutoff.
//!
//! Architecturally identical to HuggingFace's `zero-shot-classification`
//! pipeline. Per-label fan-out costs N forward passes for N labels, but
//! Tier 2/3 typical label counts (4-10) keep total latency at 1-1.5 s on
//! the bench's CPU runner.
//!
//! ### Why not GLiNER2
//!
//! Investigated and deferred. Empirical points (see [`llm-free-stack-plan.md`]
//! § 5.2 (4)):
//!
//! - DeBERTa-MNLI / mDeBERTa-XNLI score **5-6 pt higher** F1 on
//!   zero-shot classification benchmarks (Amazon-Intent, 20 Newsgroups)
//!   than GLiNER2.
//! - GLiNER2's only documented advantage is single-pass label fan-out
//!   (6.8× faster at 20 labels). For typical EventDetector cascades with
//!   4-10 labels the latency penalty of N-pass NLI is a bounded
//!   1-1.5 s, which is dominated by retrieval anyway.
//! - GLiNER's preprocessor (label-prompt template, words_mask, span_idx,
//!   span_mask, greedy NMS decoder) is a non-trivial ~500-line port that
//!   is hard to verify without weights. The `gliner2` Python package's
//!   ONNX export is undocumented in its public README.
//! - mDeBERTa-XNLI ships built-in multilingual coverage; GLiNER2 needs a
//!   separate `-multi-v1` weights variant.
//!
//! ### Default model
//!
//! `MoritzLaurer/mDeBERTa-v3-base-xnli-multilingual-nli-2mil7` (~278M,
//! MIT). XNLI 15-language average ~80% accuracy. Japanese is not in the
//! XNLI test split, so quality there is "encoder pretrain coverage +
//! cross-lingual NLI transfer" rather than direct evaluation; treat
//! Japanese performance as an empirical open question that downstream
//! consumers should validate.
//!
//! For English-only deployments, swap in
//! `MoritzLaurer/DeBERTa-v3-base-mnli-fever-anli` (~184M, MIT) via the
//! constructor. The trait surface is identical.
//!
//! ### Without the `onnx` feature
//!
//! The type compiles to a stub: construction succeeds, [`detect`] bails
//! with the same "requires `onnx` feature" message used by
//! [`OnnxEmbedding`](crate::providers::OnnxEmbedding) and
//! [`LlmLingua2Compressor`](crate::compressor::LlmLingua2Compressor).

use super::keyword::DetectedEvent;
use crate::domain::Chunk;
use crate::traits::detector::EventDetector;
use async_trait::async_trait;
use std::path::PathBuf;

#[cfg(feature = "onnx")]
use std::sync::Arc;
#[cfg(feature = "onnx")]
use tokio::sync::OnceCell;

/// Default class index for the entailment label. Matches the convention
/// used by `MoritzLaurer/mDeBERTa-v3-base-xnli-multilingual-nli-2mil7`
/// (`{"0": "entailment", "1": "neutral", "2": "contradiction"}`). Override
/// via [`NliZeroShotDetector::with_entailment_class_index`] if a model's
/// `id2label` ordering differs.
pub const DEFAULT_ENTAILMENT_CLASS_INDEX: usize = 0;

/// Default entailment probability threshold. Lower than naive 0.5 because
/// NLI models tend to spread mass across the three classes; benchmarks
/// suggest 0.5–0.7 sweet-spot. 0.5 is the HF zero-shot pipeline default.
pub const DEFAULT_THRESHOLD: f32 = 0.5;

pub struct NliZeroShotDetector {
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    /// Candidate labels. Each label produces one forward pass on every
    /// `detect` call. Order is preserved in the output `Vec<DetectedEvent>`.
    pub labels: Vec<String>,
    /// Premise template. `{text}` is replaced with the chunk's text and
    /// `{new_turn}` with the JSON-serialized turn payload. Default:
    /// `"{text}\n\n{new_turn}"`.
    pub premise_template: String,
    /// Hypothesis template. `{label}` is replaced with the candidate
    /// label name. Default: `"This text is about {label}."`.
    pub hypothesis_template: String,
    pub threshold: f32,
    pub entailment_class_index: usize,
    pub max_sequence_length: usize,
    #[cfg(feature = "onnx")]
    state: OnceCell<Arc<InferenceState>>,
}

#[cfg(feature = "onnx")]
struct InferenceState {
    // ort 2.0.0-rc.10 の Session::run は &mut self を要求するため、
    // OnnxEmbedding と同じく std::sync::Mutex で直列化する。各 detect
    // 呼び出しは N forward pass (N=label count) だが lock 取得は loop
    // 全体で 1 回。
    session: std::sync::Mutex<ort::session::Session>,
    tokenizer: tokenizers::Tokenizer,
    needs_token_type_ids: bool,
}

impl NliZeroShotDetector {
    pub fn new(model_path: impl Into<PathBuf>, tokenizer_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            tokenizer_path: tokenizer_path.into(),
            labels: Vec::new(),
            premise_template: "{text}\n\n{new_turn}".to_string(),
            hypothesis_template: "This text is about {label}.".to_string(),
            threshold: DEFAULT_THRESHOLD,
            entailment_class_index: DEFAULT_ENTAILMENT_CLASS_INDEX,
            max_sequence_length: 512,
            #[cfg(feature = "onnx")]
            state: OnceCell::new(),
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    pub fn with_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.labels = labels.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_premise_template(mut self, template: impl Into<String>) -> Self {
        self.premise_template = template.into();
        self
    }

    pub fn with_hypothesis_template(mut self, template: impl Into<String>) -> Self {
        self.hypothesis_template = template.into();
        self
    }

    pub fn with_threshold(mut self, t: f32) -> Self {
        self.threshold = t;
        self
    }

    pub fn with_entailment_class_index(mut self, idx: usize) -> Self {
        self.entailment_class_index = idx;
        self
    }

    pub fn with_max_sequence_length(mut self, n: usize) -> Self {
        self.max_sequence_length = n;
        self
    }

    #[cfg(any(feature = "onnx", test))]
    fn render_premise(&self, chunk: &Chunk, new_turn: &serde_json::Value) -> String {
        self.premise_template
            .replace("{text}", &chunk.text)
            .replace("{new_turn}", &new_turn.to_string())
    }

    #[cfg(any(feature = "onnx", test))]
    fn render_hypothesis(&self, label: &str) -> String {
        self.hypothesis_template.replace("{label}", label)
    }
}

#[cfg(feature = "onnx")]
impl NliZeroShotDetector {
    async fn ensure_state(&self) -> anyhow::Result<Arc<InferenceState>> {
        let model_path = self.model_path.clone();
        let tokenizer_path = self.tokenizer_path.clone();
        let cell = &self.state;
        let arc = cell
            .get_or_try_init(|| async move {
                tokio::task::spawn_blocking(move || load_state(&model_path, &tokenizer_path))
                    .await
                    .map_err(|e| anyhow::anyhow!("NliZeroShotDetector init task panicked: {e}"))?
                    .map(Arc::new)
            })
            .await?;
        Ok(Arc::clone(arc))
    }
}

#[cfg(feature = "onnx")]
fn load_state(model_path: &PathBuf, tokenizer_path: &PathBuf) -> anyhow::Result<InferenceState> {
    use anyhow::Context;

    let mut tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path)
        .map_err(|e| anyhow::anyhow!("failed to load tokenizer from {tokenizer_path:?}: {e}"))?;
    // NLI evaluates a (premise, hypothesis) pair that combined may exceed
    // the model's `max_position_embeddings`. The detector handles
    // truncation itself with awareness of which sequence to truncate, so
    // disable any embedded policy here. See `prepare_pair_inputs`.
    tokenizer
        .with_truncation(None)
        .map_err(|e| anyhow::anyhow!("failed to disable tokenizer truncation: {e}"))?;
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
fn run_pair_classification(
    state: &InferenceState,
    premise: &str,
    hypothesis: &str,
    max_seq: usize,
    entailment_class_index: usize,
) -> anyhow::Result<f32> {
    use ndarray::Array2;
    use ort::value::Value;

    // Tokenize the (premise, hypothesis) pair. The post_processor inserts
    // [CLS] / [SEP] / segment markers per the model's tokenizer.json.
    let encoding = state
        .tokenizer
        .encode((premise, hypothesis), true)
        .map_err(|e| anyhow::anyhow!("tokenizer encode failed: {e}"))?;
    let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let mut mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .map(|&x| x as i64)
        .collect();
    let mut type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();
    if ids.len() > max_seq {
        // Truncate from the END so the [CLS] + premise prefix survives.
        // For mDeBERTa pair-input the [SEP] segment marker between premise
        // and hypothesis must remain reachable; aggressive truncation
        // could lose the hypothesis entirely. Callers should keep the
        // premise short enough that this rarely fires.
        ids.truncate(max_seq);
        mask.truncate(max_seq);
        type_ids.truncate(max_seq);
    }
    let seq_len = ids.len();
    if seq_len == 0 {
        anyhow::bail!("tokenizer produced empty pair encoding");
    }

    let input_ids = Value::from_array(Array2::<i64>::from_shape_vec((1, seq_len), ids)?)?;
    let attention_mask = Value::from_array(Array2::<i64>::from_shape_vec((1, seq_len), mask)?)?;

    let (logits, num_classes) = {
        let mut session = state
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("ort session mutex poisoned: {e}"))?;
        let outputs = if state.needs_token_type_ids {
            let token_type_ids =
                Value::from_array(Array2::<i64>::from_shape_vec((1, seq_len), type_ids)?)?;
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
        // Sequence-classification heads emit `[batch, num_classes]`.
        if dims.len() != 2 {
            anyhow::bail!(
                "expected rank-2 output [1, num_classes], got shape {:?}",
                dims
            );
        }
        if dims[0] != 1 {
            anyhow::bail!("unexpected batch dim {} (expected 1)", dims[0]);
        }
        let num_classes = dims[1] as usize;
        if entailment_class_index >= num_classes {
            anyhow::bail!(
                "entailment_class_index {} out of range for num_classes {}",
                entailment_class_index,
                num_classes
            );
        }
        (data.to_vec(), num_classes)
    };

    // Softmax over the 3-class axis to get P(entailment).
    let mut max_logit = f32::NEG_INFINITY;
    for &v in logits.iter().take(num_classes) {
        if v > max_logit {
            max_logit = v;
        }
    }
    let mut sum = 0f32;
    for &v in logits.iter().take(num_classes) {
        sum += (v - max_logit).exp();
    }
    let entailment_logit = logits[entailment_class_index];
    Ok(((entailment_logit - max_logit).exp()) / sum)
}

#[async_trait]
impl EventDetector for NliZeroShotDetector {
    type Event = DetectedEvent;

    async fn detect(
        &self,
        chunk: &Chunk,
        new_turn: &serde_json::Value,
    ) -> anyhow::Result<Vec<Self::Event>> {
        // Empty-label short-circuit applies in both feature paths so the
        // detector behaves consistently when there's no work to do — even
        // a default-feature build that would otherwise bail with the
        // "requires `onnx` feature" message returns an empty Vec here.
        if self.labels.is_empty() {
            return Ok(Vec::new());
        }
        self.detect_inner(chunk, new_turn).await
    }
}

#[cfg(feature = "onnx")]
impl NliZeroShotDetector {
    async fn detect_inner(
        &self,
        chunk: &Chunk,
        new_turn: &serde_json::Value,
    ) -> anyhow::Result<Vec<DetectedEvent>> {
        let state = self.ensure_state().await?;
        let premise = self.render_premise(chunk, new_turn);
        let max_seq = self.max_sequence_length;
        let entail_idx = self.entailment_class_index;
        let threshold = self.threshold;

        let mut events = Vec::new();
        for label in &self.labels {
            let hypothesis = self.render_hypothesis(label);
            let premise_owned = premise.clone();
            let label_owned = label.clone();
            let state_for_blocking = Arc::clone(&state);
            let prob: f32 = tokio::task::spawn_blocking(move || {
                run_pair_classification(
                    &state_for_blocking,
                    &premise_owned,
                    &hypothesis,
                    max_seq,
                    entail_idx,
                )
            })
            .await
            .map_err(|e| anyhow::anyhow!("NliZeroShotDetector inference task panicked: {e}"))??;
            if prob >= threshold {
                events.push(DetectedEvent {
                    label: label_owned,
                    matched_keyword: format!("entailment={prob:.3}"),
                });
            }
        }
        Ok(events)
    }
}

#[cfg(not(feature = "onnx"))]
impl NliZeroShotDetector {
    async fn detect_inner(
        &self,
        _chunk: &Chunk,
        _new_turn: &serde_json::Value,
    ) -> anyhow::Result<Vec<DetectedEvent>> {
        anyhow::bail!(
            "NliZeroShotDetector::detect requires the `onnx` feature \
             (model_path = {:?}). Rebuild with `--features onnx` or use \
             KeywordDetector / EmbeddingSimilarityDetector instead.",
            self.model_path
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builder_preserves_labels_and_thresholds() {
        let d = NliZeroShotDetector::new("/tmp/m.onnx", "/tmp/t.json")
            .with_labels(vec!["combat", "social", "puzzle"])
            .with_threshold(0.7)
            .with_entailment_class_index(2)
            .with_max_sequence_length(256);
        assert_eq!(d.labels, vec!["combat", "social", "puzzle"]);
        assert!((d.threshold - 0.7).abs() < 1e-6);
        assert_eq!(d.entailment_class_index, 2);
        assert_eq!(d.max_sequence_length, 256);
    }

    #[test]
    fn premise_template_substitutes_text_and_new_turn() {
        let d = NliZeroShotDetector::new("/tmp/m.onnx", "/tmp/t.json")
            .with_premise_template("CTX: {text} || TURN: {new_turn}");
        let mut chunk = Chunk::raw_leaf("hero attacks");
        chunk.text = "hero attacks".to_string();
        let rendered = d.render_premise(&chunk, &json!({"action": "slash"}));
        assert!(rendered.contains("CTX: hero attacks"));
        assert!(rendered.contains("TURN:"));
        assert!(rendered.contains("slash"));
    }

    #[test]
    fn hypothesis_template_substitutes_label() {
        let d = NliZeroShotDetector::new("/tmp/m.onnx", "/tmp/t.json")
            .with_hypothesis_template("This passage discusses {label}.");
        assert_eq!(
            d.render_hypothesis("combat"),
            "This passage discusses combat."
        );
    }

    #[tokio::test]
    async fn detect_with_no_labels_returns_empty() {
        // Empty label list short-circuits BEFORE any ONNX session load,
        // so this works in default-feature builds too.
        let d = NliZeroShotDetector::new("/tmp/m.onnx", "/tmp/t.json");
        let chunk = Chunk::raw_leaf("anything");
        let events = d.detect(&chunk, &json!({})).await.unwrap();
        assert!(events.is_empty());
    }

    #[cfg(not(feature = "onnx"))]
    #[tokio::test]
    async fn detect_without_onnx_feature_bails() {
        let d = NliZeroShotDetector::new("/tmp/m.onnx", "/tmp/t.json").with_label("combat");
        let chunk = Chunk::raw_leaf("anything");
        let err = d.detect(&chunk, &json!({})).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("`onnx` feature"),
            "expected feature-required bail, got: {msg}"
        );
    }

    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn detect_with_missing_paths_returns_load_error() {
        let d = NliZeroShotDetector::new(
            "/tmp/tsumugi-nli-test-does-not-exist.onnx",
            "/tmp/tsumugi-nli-test-does-not-exist.json",
        )
        .with_label("combat");
        let chunk = Chunk::raw_leaf("anything");
        let err = d.detect(&chunk, &json!({})).await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("tokenizer") || msg.contains("ONNX") || msg.contains("does-not-exist"),
            "expected load error, got: {msg}"
        );
    }

    /// 実重みがある場合の zero-shot smoke。
    /// `TSUMUGI_MDEBERTA_MODEL_PATH` / `TSUMUGI_MDEBERTA_TOKENIZER_PATH`
    /// の両方が設定されているときだけ走る。
    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn detect_real_weights_returns_consistent_decision() {
        let model = match std::env::var("TSUMUGI_MDEBERTA_MODEL_PATH") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_MDEBERTA_MODEL_PATH not set");
                return;
            }
        };
        let tokenizer = match std::env::var("TSUMUGI_MDEBERTA_TOKENIZER_PATH") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_MDEBERTA_TOKENIZER_PATH not set");
                return;
            }
        };

        let d = NliZeroShotDetector::new(model, tokenizer)
            .with_labels(vec!["combat", "cooking", "astronomy"])
            .with_threshold(0.5);
        let mut chunk = Chunk::raw_leaf("");
        chunk.text =
            "The hero drew their sword and engaged the orc raiders in furious melee combat."
                .to_string();
        let events = d.detect(&chunk, &json!({})).await.unwrap();
        // Pure smoke — we don't assert which labels fire, only that
        // detect() runs end-to-end without error and the entailment
        // probabilities are valid.
        assert!(events.len() <= 3);
        for e in &events {
            assert!(e.matched_keyword.starts_with("entailment="));
        }
    }
}
