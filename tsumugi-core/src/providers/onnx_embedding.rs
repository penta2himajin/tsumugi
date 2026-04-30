//! ONNX-backed embedding provider.
//!
//! When the `onnx` feature is enabled, an `ort::session::Session` and a
//! `tokenizers::Tokenizer` are initialized lazily on the first `embed`
//! call from `model_path` + `tokenizer_path`. Inference path:
//! text → (instruction prefix) → tokenize → input_ids / attention_mask
//! (+ token_type_ids if the graph requires it) → forward → last hidden
//! state → mean-pool with the attention mask → L2 normalize → vector.
//!
//! Without the feature, the type is a surface-only stub that returns a
//! clear error from `embed` so consumers can declare an
//! `EmbeddingProvider` of type `OnnxEmbedding` even in default-feature
//! builds.
//!
//! 主想定モデル: `intfloat/multilingual-e5-small` (384 dim, MIT,
//! ONNX 配布あり)。BERT / XLM-R 系で `last_hidden_state` を出力する
//! どのモデルでも動作する。e5 系では `instruction_prefix` を
//! `"passage: "` / `"query: "` に設定して使う。

use crate::traits::embedding::{EmbeddingProvider, EmbeddingVector};
use async_trait::async_trait;
use std::path::PathBuf;

#[cfg(feature = "onnx")]
use std::sync::Arc;
#[cfg(feature = "onnx")]
use tokio::sync::OnceCell;

/// ONNX-backed embedding provider.
///
/// `instruction_prefix` は e5 系の `"passage: "` / `"query: "` のような
/// モデル固有プレフィックスに使う。プレフィックス不要のモデル (bge 系
/// 等) では空文字列でよい。
pub struct OnnxEmbedding {
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub dimension: usize,
    pub max_sequence_length: usize,
    pub instruction_prefix: String,
    #[cfg(feature = "onnx")]
    state: OnceCell<Arc<InferenceState>>,
}

#[cfg(feature = "onnx")]
struct InferenceState {
    // ort 2.0.0-rc.10 の Session::run は &mut self を要求するため、
    // 並行 embed 呼び出しは std::sync::Mutex で直列化する。CPU 推論
    // 自体が hot path のレイテンシ支配 (10-50ms) なので、この lock の
    // 競合は無視できる。
    session: std::sync::Mutex<ort::session::Session>,
    tokenizer: tokenizers::Tokenizer,
    needs_token_type_ids: bool,
}

impl OnnxEmbedding {
    pub fn new(
        model_path: impl Into<PathBuf>,
        tokenizer_path: impl Into<PathBuf>,
        dimension: usize,
    ) -> Self {
        Self {
            model_path: model_path.into(),
            tokenizer_path: tokenizer_path.into(),
            dimension,
            max_sequence_length: 512,
            instruction_prefix: String::new(),
            #[cfg(feature = "onnx")]
            state: OnceCell::new(),
        }
    }

    pub fn with_max_sequence_length(mut self, n: usize) -> Self {
        self.max_sequence_length = n;
        self
    }

    pub fn with_instruction_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.instruction_prefix = prefix.into();
        self
    }
}

#[async_trait]
impl EmbeddingProvider for OnnxEmbedding {
    #[cfg(feature = "onnx")]
    async fn embed(&self, text: &str) -> anyhow::Result<EmbeddingVector> {
        let state = self.ensure_state().await?;
        let prefixed = format!("{}{}", self.instruction_prefix, text);
        let max_seq = self.max_sequence_length;
        let expected_dim = self.dimension;
        let state_for_blocking = Arc::clone(&state);
        tokio::task::spawn_blocking(move || {
            run_inference(&state_for_blocking, &prefixed, max_seq, expected_dim)
        })
        .await
        .map_err(|e| anyhow::anyhow!("OnnxEmbedding inference task panicked: {e}"))?
    }

    #[cfg(not(feature = "onnx"))]
    async fn embed(&self, _text: &str) -> anyhow::Result<EmbeddingVector> {
        anyhow::bail!(
            "OnnxEmbedding::embed requires the `onnx` feature \
             (model_path = {:?}). Rebuild with `--features onnx` \
             or switch to MockEmbedding for tests.",
            self.model_path
        )
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(feature = "onnx")]
impl OnnxEmbedding {
    async fn ensure_state(&self) -> anyhow::Result<Arc<InferenceState>> {
        let model_path = self.model_path.clone();
        let tokenizer_path = self.tokenizer_path.clone();
        let cell = &self.state;
        let arc = cell
            .get_or_try_init(|| async move {
                tokio::task::spawn_blocking(move || load_state(&model_path, &tokenizer_path))
                    .await
                    .map_err(|e| anyhow::anyhow!("OnnxEmbedding init task panicked: {e}"))?
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
fn run_inference(
    state: &InferenceState,
    text: &str,
    max_seq: usize,
    expected_dim: usize,
) -> anyhow::Result<EmbeddingVector> {
    use ndarray::Array2;
    use ort::value::Value;

    let encoding = state
        .tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("tokenizer encode failed: {e}"))?;
    let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let mut mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .map(|&x| x as i64)
        .collect();
    if ids.len() > max_seq {
        ids.truncate(max_seq);
        mask.truncate(max_seq);
    }
    let seq_len = ids.len();
    if seq_len == 0 {
        anyhow::bail!("tokenizer produced empty sequence for input");
    }

    let input_ids = Value::from_array(Array2::<i64>::from_shape_vec((1, seq_len), ids)?)?;
    let attention_mask =
        Value::from_array(Array2::<i64>::from_shape_vec((1, seq_len), mask.clone())?)?;

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

    // 出力名は graph によって異なる ("last_hidden_state" / "logits" 等)。
    // 最初の出力を採用し、shape [1, seq, hidden] のテンソルとして扱う。
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
            "expected rank-3 output [1, seq, hidden], got shape {:?}",
            dims
        );
    }
    if dims[0] != 1 || dims[1] as usize != seq_len {
        anyhow::bail!(
            "unexpected output shape {:?}: expected [1, {}, hidden]",
            dims,
            seq_len
        );
    }
    let hidden_size = dims[2] as usize;

    // data は row-major [batch=1, seq, hidden] フラット。index = t * hidden + h。
    let mut pooled = vec![0f32; hidden_size];
    let mut total_mask = 0f32;
    for (t, &mask_t) in mask.iter().enumerate().take(seq_len) {
        let m = mask_t as f32;
        if m == 0.0 {
            continue;
        }
        let base = t * hidden_size;
        for (h, p) in pooled.iter_mut().enumerate() {
            *p += data[base + h] * m;
        }
        total_mask += m;
    }
    if total_mask > 0.0 {
        for x in &mut pooled {
            *x /= total_mask;
        }
    }
    let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut pooled {
            *x /= norm;
        }
    }

    if pooled.len() != expected_dim {
        anyhow::bail!(
            "embedding dimension mismatch: got {}, configured {}. \
             Update OnnxEmbedding::new(..., dimension) to match the ONNX model.",
            pooled.len(),
            expected_dim
        );
    }
    Ok(EmbeddingVector::new(pooled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_returns_configured_value() {
        let provider = OnnxEmbedding::new(
            "/tmp/multilingual-e5-small.onnx",
            "/tmp/multilingual-e5-small-tokenizer.json",
            384,
        );
        assert_eq!(provider.dimension(), 384);
    }

    #[test]
    fn builder_sets_max_seq_len_and_prefix() {
        let provider = OnnxEmbedding::new("/tmp/m.onnx", "/tmp/t.json", 384)
            .with_max_sequence_length(256)
            .with_instruction_prefix("passage: ");
        assert_eq!(provider.max_sequence_length, 256);
        assert_eq!(provider.instruction_prefix, "passage: ");
    }

    #[cfg(not(feature = "onnx"))]
    #[tokio::test]
    async fn embed_without_onnx_feature_bails_with_clear_message() {
        let provider = OnnxEmbedding::new("/tmp/m.onnx", "/tmp/t.json", 384);
        let err = provider.embed("hello").await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("`onnx` feature"),
            "expected feature-required bail, got: {msg}"
        );
    }

    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn embed_with_missing_paths_returns_load_error() {
        // 存在しないパスを渡した場合、ロード時点で明確なエラーを返す。
        // "not yet implemented" のような skeleton 文言ではなく、
        // tokenizer / model のロード失敗が伝播することを確認する。
        let provider = OnnxEmbedding::new(
            "/tmp/tsumugi-onnx-test-does-not-exist.onnx",
            "/tmp/tsumugi-onnx-test-does-not-exist.json",
            384,
        );
        let err = provider.embed("hello").await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("tokenizer") || msg.contains("ONNX") || msg.contains("does-not-exist"),
            "expected load error from real ort path, got: {msg}"
        );
        assert!(
            !msg.contains("not yet implemented"),
            "expected real ort impl, but skeleton bail still present: {msg}"
        );
    }

    /// 実重みがある場合の forward 検証。`TSUMUGI_E5_MODEL_PATH` /
    /// `TSUMUGI_E5_TOKENIZER_PATH` 両方が設定されているときのみ走る。
    /// 設定なし環境 (default の cargo test) では skip。
    #[cfg(feature = "onnx")]
    #[tokio::test]
    async fn embed_real_weights_returns_normalized_384_dim_vector() {
        let model_path = match std::env::var("TSUMUGI_E5_MODEL_PATH") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_E5_MODEL_PATH not set");
                return;
            }
        };
        let tokenizer_path = match std::env::var("TSUMUGI_E5_TOKENIZER_PATH") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping: TSUMUGI_E5_TOKENIZER_PATH not set");
                return;
            }
        };

        let provider =
            OnnxEmbedding::new(model_path, tokenizer_path, 384).with_instruction_prefix("query: ");
        let v = provider.embed("hello world").await.unwrap();
        assert_eq!(v.len(), 384);
        let norm: f32 = v.as_slice().iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-3,
            "expected L2-normalized vector, got norm = {norm}"
        );

        // 同じ入力 → 同じベクトル (deterministic)
        let v2 = provider.embed("hello world").await.unwrap();
        assert_eq!(v, v2);

        // 異なる入力 → 異なるベクトル
        let v3 = provider.embed("entirely different topic").await.unwrap();
        assert_ne!(v, v3);
    }
}
