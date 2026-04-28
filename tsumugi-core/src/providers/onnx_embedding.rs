//! ONNX-backed embedding provider.
//!
//! When the `onnx` feature is enabled, an `ort::Session` is initialized
//! lazily on the first `embed` call from `model_path` + `tokenizer_path`,
//! then text → token ids → forward → mean-pooled vector. Without the
//! feature, the type is a surface-only stub that returns a clear error
//! from `embed` so consumers can declare an `EmbeddingProvider` of type
//! `OnnxEmbedding` even in default-feature builds.
//!
//! Phase 4-α Step 1 では trait 面のみ先行追加。実 ort 統合は CI 統合と
//! 並行で進む (`docs/ci-benchmark-integration-plan.md` 参照)。
//!
//! 主想定モデル: `intfloat/multilingual-e5-small` (384 dim, MIT, ONNX 配布あり)。

use crate::traits::embedding::{EmbeddingProvider, EmbeddingVector};
use async_trait::async_trait;
use std::path::PathBuf;

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
    async fn embed(&self, _text: &str) -> anyhow::Result<EmbeddingVector> {
        anyhow::bail!(
            "OnnxEmbedding::embed is not yet implemented (model_path = {:?}). \
             Phase 4-α Step 1 では trait 面のみ先行追加されており、ort crate \
             統合は CI 統合と並行で進む。MockEmbedding を fallback として \
             利用してください。",
            self.model_path
        )
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

    #[tokio::test]
    async fn embed_returns_error_in_skeleton() {
        // Trait 面のみ skeleton: feature on/off ともに bail させる。
        // ort 統合後に feature on 側の bail を本物の推論で置き換える。
        let provider = OnnxEmbedding::new("/tmp/m.onnx", "/tmp/t.json", 384);
        let err = provider.embed("hello").await.unwrap_err();
        let msg = err.to_string();
        #[cfg(feature = "onnx")]
        assert!(
            msg.contains("not yet implemented"),
            "expected skeleton bail, got: {msg}"
        );
        #[cfg(not(feature = "onnx"))]
        assert!(
            msg.contains("`onnx` feature"),
            "expected feature-required bail, got: {msg}"
        );
    }
}
