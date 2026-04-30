//! SetFitClassifier — paper-faithful SetFit + MiniLM (Tunstall et al., 2022)
//! query router.
//!
//! Composes `OnnxEmbedding` (mean-pool + L2 normalize) with a small linear
//! classification head loaded from a JSON file. Inference path:
//! query → ONNX session → 384-dim L2-normalized vector → matmul with the
//! head's weight matrix → bias add → argmax → label string → `QueryClass`.
//!
//! ### Training data lives downstream
//!
//! `tsumugi-core` ships only the inference code and the file format for
//! the linear head (see [`LinearHeadFile`]). Training (per-class examples,
//! contrastive fine-tuning of the encoder, fit of the head) happens in
//! whatever downstream pipeline knows the application's label
//! distribution. Generate via Python `setfit`, export the head as JSON in
//! the format documented below, and pass the resulting paths into
//! [`SetFitClassifier::new`].
//!
//! ### Head JSON format
//!
//! ```json
//! {
//!   "labels": ["Literal", "Narrative", "Analytical", "Unknown"],
//!   "embedding_dim": 384,
//!   "weights": [[w_00, w_01, ..., w_0_383], ..., [w_3_0, ...]],
//!   "bias":    [b_0, b_1, b_2, b_3]
//! }
//! ```
//!
//! `labels[i]` is matched case-insensitively against the four `QueryClass`
//! variants when the head's argmax lands on row `i`. Labels that don't map
//! (e.g. domain-specific names) fall through to the configured `default`
//! class. Use [`SetFitClassifier::with_default`] to override it from the
//! built-in `Unknown`.
//!
//! ### Multilingual fallback
//!
//! The default constructor assumes a 384-dim sentence-transformers encoder
//! (MiniLM-L6-v2 is the canonical choice). MiniLM-L6-v2 is English-only;
//! for multilingual queries (Japanese in particular) swap the embedder for
//! `paraphrase-multilingual-MiniLM-L12-v2` (~118M, 384-dim) via
//! [`SetFitClassifier::with_embedder`]. The head JSON's `embedding_dim`
//! must match whichever encoder you choose.
//!
//! Without the `onnx` feature the type compiles to a stub: construction
//! succeeds, [`classify`](QueryClassifier::classify) bails with a clear
//! "requires `onnx` feature" message inherited from `OnnxEmbedding`.

use crate::providers::OnnxEmbedding;
use crate::traits::classifier::{QueryClass, QueryClassifier};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;

#[cfg(feature = "onnx")]
use crate::traits::embedding::EmbeddingProvider;
#[cfg(feature = "onnx")]
use std::sync::Arc;
#[cfg(feature = "onnx")]
use tokio::sync::OnceCell;

/// MiniLM-L6-v2 embedding dimension. Used as the default for the embedder
/// constructor; override via [`SetFitClassifier::with_embedder`] if your
/// fine-tuned encoder uses a different size (e.g. 768 for MiniLM-L12).
pub const DEFAULT_MINI_LM_DIM: usize = 384;

pub struct SetFitClassifier {
    pub embedder: OnnxEmbedding,
    pub head_path: PathBuf,
    /// Class returned when the head's argmax label doesn't case-insensitively
    /// match any `QueryClass` variant. Defaults to `Unknown`.
    pub default: QueryClass,
    #[cfg(feature = "onnx")]
    head: OnceCell<Arc<LinearHead>>,
}

/// On-disk representation of the linear classification head. Public so that
/// downstream training pipelines can `serde_json::to_writer` directly into
/// the file format the runtime expects.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct LinearHeadFile {
    /// Class names. Order matches the rows of `weights` and `bias`.
    pub labels: Vec<String>,
    /// Encoder output dimension (MiniLM-L6-v2 = 384, MiniLM-L12 = 384,
    /// e5-small = 384). Must match the encoder you pair this head with.
    pub embedding_dim: usize,
    /// Row-major weight matrix: `weights[i]` is the row for `labels[i]` and
    /// must have length `embedding_dim`.
    pub weights: Vec<Vec<f32>>,
    /// Per-label bias. Same length as `labels`.
    pub bias: Vec<f32>,
}

#[cfg(any(feature = "onnx", test))]
#[derive(Debug, Clone)]
struct LinearHead {
    labels: Vec<String>,
    embedding_dim: usize,
    /// Row-major flat layout `[label_0_dim_0, label_0_dim_1, ..., label_n_dim_d]`
    /// to keep the dot product hot loop branch-free over Vec<Vec<f32>>.
    weights_flat: Vec<f32>,
    bias: Vec<f32>,
}

#[cfg(any(feature = "onnx", test))]
impl LinearHead {
    fn from_file(file: LinearHeadFile) -> anyhow::Result<Self> {
        let n = file.labels.len();
        if n == 0 {
            anyhow::bail!("linear head must contain at least one label");
        }
        if file.weights.len() != n {
            anyhow::bail!(
                "weights row count ({}) does not match labels count ({})",
                file.weights.len(),
                n
            );
        }
        if file.bias.len() != n {
            anyhow::bail!(
                "bias length ({}) does not match labels count ({})",
                file.bias.len(),
                n
            );
        }
        let mut flat = Vec::with_capacity(n * file.embedding_dim);
        for (i, row) in file.weights.iter().enumerate() {
            if row.len() != file.embedding_dim {
                anyhow::bail!(
                    "weights row {} has length {}, expected embedding_dim={}",
                    i,
                    row.len(),
                    file.embedding_dim
                );
            }
            flat.extend_from_slice(row);
        }
        Ok(Self {
            labels: file.labels,
            embedding_dim: file.embedding_dim,
            weights_flat: flat,
            bias: file.bias,
        })
    }

    fn from_json_path(path: &std::path::Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("failed to read head file {path:?}: {e}"))?;
        let file: LinearHeadFile = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow::anyhow!("failed to parse head JSON {path:?}: {e}"))?;
        Self::from_file(file)
    }

    fn argmax(&self, embedding: &[f32]) -> anyhow::Result<usize> {
        if embedding.len() != self.embedding_dim {
            anyhow::bail!(
                "embedding dim {} does not match head's embedding_dim {}",
                embedding.len(),
                self.embedding_dim
            );
        }
        let mut best_idx = 0usize;
        let mut best_score = f32::NEG_INFINITY;
        for (i, &b) in self.bias.iter().enumerate() {
            let base = i * self.embedding_dim;
            let row = &self.weights_flat[base..base + self.embedding_dim];
            let mut score = b;
            for (w, e) in row.iter().zip(embedding.iter()) {
                score += w * e;
            }
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }
        Ok(best_idx)
    }
}

impl SetFitClassifier {
    pub fn new(
        model_path: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
        head_path: impl Into<PathBuf>,
    ) -> Self {
        let embedder = OnnxEmbedding::new(model_path, tokenizer_path, DEFAULT_MINI_LM_DIM);
        Self {
            embedder,
            head_path: head_path.into(),
            default: QueryClass::Unknown,
            #[cfg(feature = "onnx")]
            head: OnceCell::new(),
        }
    }

    /// Convenience constructor for the canonical filename layout that
    /// `scripts/train_setfit.py` produces. Given a directory and a stem
    /// (e.g. `"all-MiniLM-L6-v2-default"`), expects:
    ///
    /// - `{dir}/{stem}.onnx`             — encoder weights (raw last_hidden_state)
    /// - `{dir}/{stem}.tokenizer.json`   — sentence-transformers tokenizer
    /// - `{dir}/{stem}.head.json`        — `LinearHeadFile`
    ///
    /// The repo ships one trained head under `models/setfit/` with stem
    /// `all-MiniLM-L6-v2-default` (English, 4 labels: Literal / Narrative /
    /// Analytical / Unknown). Re-train against your own examples by editing
    /// `models/setfit-training/queries.jsonl` and running
    /// `python3 scripts/train_setfit.py`.
    pub fn from_dir_and_stem(dir: impl AsRef<std::path::Path>, stem: &str) -> Self {
        let dir = dir.as_ref();
        Self::new(
            dir.join(format!("{stem}.onnx")),
            dir.join(format!("{stem}.tokenizer.json")),
            dir.join(format!("{stem}.head.json")),
        )
    }

    /// Replace the default 384-dim MiniLM embedder. Use this to swap in
    /// `paraphrase-multilingual-MiniLM-L12-v2` for Japanese / multilingual
    /// queries, or any other sentence-transformers encoder whose output
    /// dim matches the head JSON's `embedding_dim`.
    pub fn with_embedder(mut self, embedder: OnnxEmbedding) -> Self {
        self.embedder = embedder;
        self
    }

    /// Override the fallback class returned when the argmax label does not
    /// case-insensitively match any `QueryClass` variant. Defaults to
    /// `Unknown`.
    pub fn with_default(mut self, class: QueryClass) -> Self {
        self.default = class;
        self
    }

    #[cfg(any(feature = "onnx", test))]
    fn label_to_class(&self, label: &str) -> QueryClass {
        match label.trim().to_ascii_lowercase().as_str() {
            "literal" => QueryClass::Literal,
            "narrative" => QueryClass::Narrative,
            "analytical" => QueryClass::Analytical,
            "unknown" => QueryClass::Unknown,
            _ => self.default,
        }
    }
}

#[cfg(feature = "onnx")]
impl SetFitClassifier {
    async fn ensure_head(&self) -> anyhow::Result<Arc<LinearHead>> {
        let head_path = self.head_path.clone();
        let arc = self
            .head
            .get_or_try_init(|| async move {
                tokio::task::spawn_blocking(move || LinearHead::from_json_path(&head_path))
                    .await
                    .map_err(|e| anyhow::anyhow!("SetFit head load task panicked: {e}"))?
                    .map(Arc::new)
            })
            .await?;
        Ok(Arc::clone(arc))
    }
}

#[async_trait]
impl QueryClassifier for SetFitClassifier {
    #[cfg(feature = "onnx")]
    async fn classify(&self, query: &str) -> anyhow::Result<QueryClass> {
        let head = self.ensure_head().await?;
        let embedding = self.embedder.embed(query).await?;
        let idx = head.argmax(embedding.as_slice())?;
        Ok(self.label_to_class(&head.labels[idx]))
    }

    #[cfg(not(feature = "onnx"))]
    async fn classify(&self, _query: &str) -> anyhow::Result<QueryClass> {
        // Surface the same feature-required error that the embedder would
        // produce, with classifier-context phrasing so callers know which
        // type to swap.
        anyhow::bail!(
            "SetFitClassifier::classify requires the `onnx` feature \
             (model_path = {:?}). Rebuild with `--features onnx` or use \
             RegexClassifier instead.",
            self.embedder.model_path
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_head_json() -> LinearHeadFile {
        // 4 labels × 4 embedding_dim toy head. Designed so that the unit
        // vector e_i argmaxes to label i: weights[i] is e_i, biases zero.
        LinearHeadFile {
            labels: vec![
                "Literal".to_string(),
                "Narrative".to_string(),
                "Analytical".to_string(),
                "Unknown".to_string(),
            ],
            embedding_dim: 4,
            weights: vec![
                vec![1.0, 0.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0, 0.0],
                vec![0.0, 0.0, 1.0, 0.0],
                vec![0.0, 0.0, 0.0, 1.0],
            ],
            bias: vec![0.0, 0.0, 0.0, 0.0],
        }
    }

    #[test]
    fn linear_head_argmax_picks_dominant_dimension() {
        let head = LinearHead::from_file(fake_head_json()).unwrap();
        assert_eq!(head.argmax(&[1.0, 0.0, 0.0, 0.0]).unwrap(), 0);
        assert_eq!(head.argmax(&[0.0, 1.0, 0.0, 0.0]).unwrap(), 1);
        assert_eq!(head.argmax(&[0.0, 0.0, 1.0, 0.0]).unwrap(), 2);
        assert_eq!(head.argmax(&[0.0, 0.0, 0.0, 1.0]).unwrap(), 3);
    }

    #[test]
    fn linear_head_rejects_dim_mismatch() {
        let head = LinearHead::from_file(fake_head_json()).unwrap();
        let err = head.argmax(&[1.0, 0.0, 0.0]).unwrap_err();
        assert!(err.to_string().contains("embedding_dim"), "got: {err}");
    }

    #[test]
    fn linear_head_rejects_misshaped_file() {
        let mut f = fake_head_json();
        f.bias.pop();
        let err = LinearHead::from_file(f).unwrap_err();
        assert!(err.to_string().contains("bias length"), "got: {err}");
    }

    #[test]
    fn linear_head_rejects_empty_labels() {
        let f = LinearHeadFile {
            labels: vec![],
            embedding_dim: 4,
            weights: vec![],
            bias: vec![],
        };
        let err = LinearHead::from_file(f).unwrap_err();
        assert!(err.to_string().contains("at least one label"), "got: {err}");
    }

    #[test]
    fn linear_head_loads_from_json_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("head.json");
        let json = serde_json::to_string(&fake_head_json()).unwrap();
        std::fs::write(&path, json).unwrap();
        let head = LinearHead::from_json_path(&path).unwrap();
        assert_eq!(head.labels.len(), 4);
        assert_eq!(head.embedding_dim, 4);
    }

    #[test]
    fn label_to_class_is_case_insensitive() {
        let c = SetFitClassifier::new("/tmp/m.onnx", "/tmp/t.json", "/tmp/h.json");
        assert_eq!(c.label_to_class("Literal"), QueryClass::Literal);
        assert_eq!(c.label_to_class("NARRATIVE"), QueryClass::Narrative);
        assert_eq!(c.label_to_class("  analytical  "), QueryClass::Analytical);
        assert_eq!(c.label_to_class("Unknown"), QueryClass::Unknown);
    }

    #[test]
    fn label_to_class_falls_back_to_default_for_unrecognized() {
        let c = SetFitClassifier::new("/tmp/m.onnx", "/tmp/t.json", "/tmp/h.json")
            .with_default(QueryClass::Analytical);
        assert_eq!(c.label_to_class("Custom"), QueryClass::Analytical);
        assert_eq!(c.label_to_class(""), QueryClass::Analytical);
    }

    #[test]
    fn from_dir_and_stem_uses_canonical_filenames() {
        let c = SetFitClassifier::from_dir_and_stem("/models/setfit", "all-MiniLM-L6-v2-default");
        assert_eq!(
            c.embedder.model_path,
            PathBuf::from("/models/setfit/all-MiniLM-L6-v2-default.onnx")
        );
        assert_eq!(
            c.embedder.tokenizer_path,
            PathBuf::from("/models/setfit/all-MiniLM-L6-v2-default.tokenizer.json")
        );
        assert_eq!(
            c.head_path,
            PathBuf::from("/models/setfit/all-MiniLM-L6-v2-default.head.json")
        );
    }

    #[cfg(not(feature = "onnx"))]
    #[tokio::test]
    async fn classify_without_onnx_feature_bails() {
        let c = SetFitClassifier::new("/tmp/m.onnx", "/tmp/t.json", "/tmp/h.json");
        let err = c.classify("anything").await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("`onnx` feature"),
            "expected feature-required bail, got: {msg}"
        );
    }

    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn classify_with_missing_paths_returns_load_error() {
        let c = SetFitClassifier::new(
            "/tmp/tsumugi-setfit-test-does-not-exist.onnx",
            "/tmp/tsumugi-setfit-test-does-not-exist-tok.json",
            "/tmp/tsumugi-setfit-test-does-not-exist-head.json",
        );
        let err = c.classify("anything").await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("head") || msg.contains("tokenizer") || msg.contains("ONNX"),
            "expected load error, got: {msg}"
        );
    }

    /// 実重みがある場合の classification smoke。
    /// `TSUMUGI_MINILM_MODEL_PATH` / `TSUMUGI_MINILM_TOKENIZER_PATH` /
    /// `TSUMUGI_SETFIT_HEAD_PATH` の 3 つすべてが設定されているときだけ走る。
    /// default の cargo test では skip。
    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn classify_real_weights_returns_known_class() {
        let model = match std::env::var("TSUMUGI_MINILM_MODEL_PATH") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_MINILM_MODEL_PATH not set");
                return;
            }
        };
        let tokenizer = match std::env::var("TSUMUGI_MINILM_TOKENIZER_PATH") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_MINILM_TOKENIZER_PATH not set");
                return;
            }
        };
        let head = match std::env::var("TSUMUGI_SETFIT_HEAD_PATH") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_SETFIT_HEAD_PATH not set");
                return;
            }
        };

        let c = SetFitClassifier::new(model, tokenizer, head);
        // We don't assert which class — that depends on the user-trained
        // head. We only verify the pipeline runs end to end without error
        // and yields one of the four enum variants (or the configured
        // default). This is a "did the wiring fire" smoke, not a quality
        // benchmark.
        let class = c.classify("What is the capital of France?").await.unwrap();
        let _: QueryClass = class;
    }
}
