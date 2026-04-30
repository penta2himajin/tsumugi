//! Cross-adapter helpers: chunking, retrieval, compression.
//!
//! Phase 4-α Step 3 PR ③ で導入。LongMemEval / RULER / MemoryAgentBench
//! の各 adapter で共有するユーティリティを集約する。
//!
//! 設計方針:
//! - tier-0 / tier-0-1 / tier-0-1-2 ablation は **LLM 不使用**。
//!   retrieval / compression のみで判定する。これらの ablation で使う
//!   helper を 1 ファイルに集めた。
//! - tier-0-1 の embedding は default で MockEmbedding (FNV-1a 64-dim、
//!   deterministic) を使い、`onnx` feature 有効 + `TSUMUGI_E5_MODEL_PATH`
//!   と `TSUMUGI_E5_TOKENIZER_PATH` の両方が設定されている場合は
//!   OnnxEmbedding (multilingual-e5-small ONNX) に切り替える。
//!   Phase 4-γ Step 1 で導入。
//! - tier-0-1-2 の compressor は default で `TruncateCompressor` (head +
//!   tail tokens with ellipsis、LLM 不使用)。`onnx` feature 有効 +
//!   `TSUMUGI_LLMLINGUA2_MODEL_PATH` / `TSUMUGI_LLMLINGUA2_TOKENIZER_PATH`
//!   の両方が設定されていれば `LlmLingua2Compressor` (per-token classifier、
//!   paper-faithful、LLM 不使用) に切り替わる。Phase 4-γ Step 2 で導入。
//!   `LlmDelegationCompressor` (旧 `LlmLinguaCompressor`) は LLM 委譲版で
//!   ablation の "LLM 不使用 baseline" 軸を破壊するため不採用。
//!
//! 詳細は `docs/ci-benchmark-integration-plan.md` §「Tier 別 ablation の分離」。

/// 文区切り (`. ! ? \n`) 優先で context を `target_chars` 程度の chunk に分割する。
/// target を超えた次の sentence boundary で chunk 終了。最終 chunk は target を
/// 超えないよう貪欲に詰める。Unicode multi-byte char は char_indices で安全に扱う。
pub fn chunk_text(context: &str, target_chars: usize) -> Vec<String> {
    if context.is_empty() {
        return Vec::new();
    }
    // 呼び出し側 (env override) で >= 256 を保証している。低いターゲットは
    // テストでのみ使用するのでフロアを設けない。
    let target = target_chars.max(1);
    let mut chunks = Vec::new();
    let mut current = String::with_capacity(target + 256);
    let bytes = context.as_bytes();
    let mut last_boundary = 0usize;

    for (i, _) in context.char_indices() {
        let b = bytes[i];
        if i > last_boundary && current.len() >= target {
            chunks.push(std::mem::take(&mut current));
        }
        if matches!(b, b'.' | b'!' | b'?' | b'\n') {
            last_boundary = i + 1;
        }
        let ch_end = next_char_boundary(context, i);
        current.push_str(&context[i..ch_end]);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn next_char_boundary(s: &str, i: usize) -> usize {
    let mut j = i + 1;
    while j < s.len() && !s.is_char_boundary(j) {
        j += 1;
    }
    j
}

/// 末尾 `n` chars を char boundary に揃えて切り出す。文字列が短い場合は全体を返す。
pub fn tail_chars(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let target_start = s.len() - n;
    let mut start = target_start;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    s[start..].to_string()
}

/// LLM 不使用 ablation で retrieved chunks をひとつの判定対象テキストに
/// まとめる際の区切り。`substring_match[_any]` の対象に渡す。
pub fn concat_for_judge(chunks: &[String]) -> String {
    chunks.join("\n\n---\n\n")
}

#[cfg(feature = "network")]
mod retrieve {
    use std::collections::HashMap;
    use std::sync::Arc;

    #[cfg(feature = "onnx")]
    use tsumugi_core::compressor::LlmLingua2Compressor;
    use tsumugi_core::compressor::TruncateCompressor;
    use tsumugi_core::domain::ChunkId;
    use tsumugi_core::providers::MockEmbedding;
    #[cfg(feature = "onnx")]
    use tsumugi_core::providers::OnnxEmbedding;
    use tsumugi_core::retriever::{Bm25Retriever, CosineRetriever, HybridRetriever};
    use tsumugi_core::traits::compressor::{CompressionHint, PromptCompressor};
    use tsumugi_core::traits::embedding::{EmbeddingProvider, EmbeddingVector};
    use tsumugi_core::traits::retriever::Retriever;

    /// MockEmbedding の dimension。FNV-1a hash で各 token を bucket に
    /// 振るので、低次元すぎると衝突で cosine が BM25 と区別つかなくなる。
    /// 64 は MockEmbedding の Default、ablation 効果検出には十分な検出力。
    const MOCK_EMBEDDING_DIM: usize = 64;

    /// OnnxEmbedding を使う場合の default 次元 (multilingual-e5-small)。
    /// `TSUMUGI_E5_DIM` env で override 可。
    #[cfg(feature = "onnx")]
    const DEFAULT_E5_DIM: usize = 384;

    /// `TSUMUGI_E5_MODEL_PATH` と `TSUMUGI_E5_TOKENIZER_PATH` の両方が
    /// 設定されている場合のみ OnnxEmbedding を返す。それ以外 (env 未設定
    /// または `onnx` feature 無効) は MockEmbedding に fallback する。
    /// 実 embedding は L2-normalize 済みの 384-dim (default)、Mock は 64-dim。
    fn make_embedding_provider() -> Arc<dyn EmbeddingProvider> {
        #[cfg(feature = "onnx")]
        {
            let model = std::env::var("TSUMUGI_E5_MODEL_PATH").ok();
            let tokenizer = std::env::var("TSUMUGI_E5_TOKENIZER_PATH").ok();
            if let (Some(m), Some(t)) = (model, tokenizer) {
                let dim = std::env::var("TSUMUGI_E5_DIM")
                    .ok()
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(DEFAULT_E5_DIM);
                // 単一プロバイダで passages / query 双方を embed するため
                // prefix は空のまま。e5 の "passage: " / "query: " 区別を
                // 完全に活かすには CosineRetriever 側で role を分ける改修
                // が必要 (follow-up)。
                return Arc::new(OnnxEmbedding::new(m, t, dim));
            }
        }
        Arc::new(MockEmbedding::new(MOCK_EMBEDDING_DIM))
    }

    /// BM25 retrieval を `chunks` に対して走らせ、上位 `top_k` chunk を
    /// テキストとして返す。空 corpus は空 Vec。
    pub async fn bm25_retrieve(
        chunks: &[String],
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<Vec<String>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }
        let pairs: Vec<(ChunkId, String)> =
            chunks.iter().map(|c| (ChunkId::new(), c.clone())).collect();
        let lookup: HashMap<ChunkId, String> = pairs.iter().cloned().collect();
        let retriever = Bm25Retriever::new(pairs);
        let hits = retriever.retrieve(query, top_k).await?;
        Ok(hits
            .into_iter()
            .filter_map(|h| lookup.get(&h.chunk_id).cloned())
            .collect())
    }

    /// BM25 + cosine の HybridRetriever で chunks を検索する。embedding は
    /// `onnx` feature 有効かつ `TSUMUGI_E5_MODEL_PATH` /
    /// `TSUMUGI_E5_TOKENIZER_PATH` が設定されていれば OnnxEmbedding
    /// (multilingual-e5-small)、それ以外は MockEmbedding (FNV-1a 64-dim、
    /// deterministic) にフォールバック。
    pub async fn hybrid_retrieve(
        chunks: &[String],
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<Vec<String>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }
        let pairs: Vec<(ChunkId, String)> =
            chunks.iter().map(|c| (ChunkId::new(), c.clone())).collect();
        let lookup: HashMap<ChunkId, String> = pairs.iter().cloned().collect();
        let provider: Arc<dyn EmbeddingProvider> = make_embedding_provider();
        // pre-compute embeddings (CosineRetriever は embed 済み corpus を要求)
        let mut embeddings: Vec<(ChunkId, EmbeddingVector)> = Vec::with_capacity(pairs.len());
        for (id, text) in &pairs {
            let v = provider.embed(text).await?;
            embeddings.push((*id, v));
        }
        let bm25: Arc<dyn Retriever> = Arc::new(Bm25Retriever::new(pairs));
        let cos: Arc<dyn Retriever> = Arc::new(CosineRetriever::new(embeddings, provider.clone()));
        let hybrid = HybridRetriever::new(bm25, cos);
        let hits = hybrid.retrieve(query, top_k).await?;
        Ok(hits
            .into_iter()
            .filter_map(|h| lookup.get(&h.chunk_id).cloned())
            .collect())
    }

    /// `TruncateCompressor` でテキストを `budget_tokens` (whitespace token
    /// count) に切り詰める。`preserve_tail_tokens` 分は末尾を保持する
    /// (head + " … " + tail の形)。tier-0-1-2 ablation の deterministic
    /// fallback として常に利用可能、ユニットテストでも直接呼ばれる。
    pub async fn truncate_compress(
        text: &str,
        budget_tokens: u32,
        preserve_tail_tokens: u32,
    ) -> anyhow::Result<String> {
        let hint = CompressionHint::new(budget_tokens, preserve_tail_tokens);
        TruncateCompressor.compress(text, hint).await
    }

    /// tier-0-1-2 ablation 用の compressor 切替えポイント。
    ///
    /// `onnx` feature 有効かつ `TSUMUGI_LLMLINGUA2_MODEL_PATH` /
    /// `TSUMUGI_LLMLINGUA2_TOKENIZER_PATH` が両方設定されている場合は
    /// `LlmLingua2Compressor` (per-token classifier、110M mBERT-base) を
    /// 使う。それ以外は `truncate_compress` にフォールバック。
    /// `keep_class_index` は `TSUMUGI_LLMLINGUA2_KEEP_CLASS` env (0|1) で
    /// override 可、default は 1 (paper の preserve label)。
    pub async fn tier_0_1_2_compress(
        text: &str,
        budget_tokens: u32,
        preserve_tail_tokens: u32,
    ) -> anyhow::Result<String> {
        #[cfg(feature = "onnx")]
        {
            let model = std::env::var("TSUMUGI_LLMLINGUA2_MODEL_PATH").ok();
            let tokenizer = std::env::var("TSUMUGI_LLMLINGUA2_TOKENIZER_PATH").ok();
            if let (Some(m), Some(t)) = (model, tokenizer) {
                let keep_idx = std::env::var("TSUMUGI_LLMLINGUA2_KEEP_CLASS")
                    .ok()
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(1);
                let compressor = LlmLingua2Compressor::new(m, t).with_keep_class_index(keep_idx);
                let hint = CompressionHint::new(budget_tokens, preserve_tail_tokens);
                return compressor.compress(text, hint).await;
            }
        }
        truncate_compress(text, budget_tokens, preserve_tail_tokens).await
    }
}

#[cfg(feature = "network")]
pub use retrieve::{bm25_retrieve, hybrid_retrieve, tier_0_1_2_compress};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_text_splits_at_target_chars_with_sentence_boundary() {
        let text = "First sentence. Second sentence here. Third one. ".repeat(50);
        let chunks = chunk_text(&text, 100);
        for c in &chunks {
            assert!(c.len() < 250, "chunk too large: {} chars", c.len());
        }
        let rejoined: String = chunks.join("");
        assert_eq!(rejoined, text);
    }

    #[test]
    fn chunk_text_handles_unicode_safely() {
        let text = "日本語テキストです。これは別の文。さらに次の文。".repeat(20);
        let chunks = chunk_text(&text, 50);
        assert!(!chunks.is_empty());
        let rejoined: String = chunks.join("");
        assert_eq!(rejoined, text);
    }

    #[test]
    fn chunk_text_empty_returns_empty() {
        assert_eq!(chunk_text("", 100), Vec::<String>::new());
    }

    #[test]
    fn tail_chars_respects_char_boundary() {
        let text = "日本語テキスト";
        let tail = tail_chars(text, 6);
        assert!(tail.len() <= text.len());
        assert!(text.ends_with(&tail));
    }

    #[test]
    fn tail_chars_returns_full_string_when_shorter_than_n() {
        assert_eq!(tail_chars("short", 100), "short");
    }

    #[test]
    fn concat_for_judge_uses_separator() {
        let chunks = vec!["alpha".to_string(), "beta".to_string()];
        let s = concat_for_judge(&chunks);
        assert!(s.contains("alpha"));
        assert!(s.contains("beta"));
        assert!(s.contains("---"));
    }

    #[test]
    fn concat_for_judge_empty_returns_empty_string() {
        assert_eq!(concat_for_judge(&[]), "");
    }
}

#[cfg(all(test, feature = "network"))]
mod retrieve_tests {
    use super::*;
    // truncate_compress は tier-0-1-2 fallback 専用なので pub use 経由では
    // export していない。テストでは inner mod から直接参照する。
    use super::retrieve::truncate_compress;

    #[tokio::test]
    async fn bm25_retrieve_returns_hits_for_keyword() {
        let chunks = vec![
            "the quick brown fox jumps over the lazy dog".to_string(),
            "rust is a systems programming language".to_string(),
            "tokio is an async runtime for rust".to_string(),
        ];
        let hits = bm25_retrieve(&chunks, "rust language", 2).await.unwrap();
        assert!(!hits.is_empty(), "expected at least one hit");
        // BM25 は "rust language" を含む chunk を上位に出すはず
        assert!(
            hits.iter().any(|h| h.contains("systems programming")),
            "got: {hits:?}"
        );
    }

    #[tokio::test]
    async fn bm25_retrieve_empty_corpus_returns_empty() {
        let hits = bm25_retrieve(&[], "anything", 5).await.unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn bm25_retrieve_caps_at_top_k() {
        let chunks: Vec<String> = (0..20).map(|i| format!("sentence number {i}")).collect();
        let hits = bm25_retrieve(&chunks, "sentence", 3).await.unwrap();
        assert!(hits.len() <= 3, "got {} hits, expected ≤ 3", hits.len());
    }

    #[tokio::test]
    async fn hybrid_retrieve_returns_hits() {
        let chunks = vec![
            "the cat sat on the mat".to_string(),
            "quantum mechanics is hard".to_string(),
            "machine learning models".to_string(),
        ];
        let hits = hybrid_retrieve(&chunks, "cat mat", 2).await.unwrap();
        assert!(!hits.is_empty());
    }

    #[tokio::test]
    async fn hybrid_retrieve_empty_corpus_returns_empty() {
        let hits = hybrid_retrieve(&[], "anything", 5).await.unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn truncate_compress_keeps_under_budget_unchanged() {
        let s = "alpha beta gamma";
        let out = truncate_compress(s, 100, 10).await.unwrap();
        assert_eq!(out, s);
    }

    #[tokio::test]
    async fn truncate_compress_shortens_over_budget_text() {
        let words: Vec<String> = (0..100).map(|i| format!("w{i}")).collect();
        let s = words.join(" ");
        let out = truncate_compress(&s, 10, 3).await.unwrap();
        // 元 100 token > 10 budget なので必ず ellipsis が入る
        assert!(out.contains("…"), "got: {out}");
        // 末尾 token (w99) は preserve_tail_tokens で残る
        assert!(out.contains("w99"));
        // 削減されている
        assert!(out.len() < s.len());
    }
}
