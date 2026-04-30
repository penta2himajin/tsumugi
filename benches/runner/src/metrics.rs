//! Common metric primitives shared across adapters.
//!
//! 規則ベース primary metric は (substring / exact / fuzzy match) の
//! 単純な関数として表現し、LLM judge secondary metric は別経路で記録する。
//! 詳細は `docs/ci-benchmark-integration-plan.md` §「メトリクス」。

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CaseMetric {
    pub case_id: String,
    pub correct: bool,
    pub latency_ms: u64,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    /// retrieval (BM25 / Hybrid) 段の wall time。tier-0 系で記録、
    /// `full` (既存 Step 2/3 経路) では retrieval を adapter 内で行わ
    /// ない場合 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_latency_ms: Option<u64>,
    /// retrieve した chunk 数 (top_k で打ち切り後)。tier-0 系で記録。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieved_chunks: Option<usize>,
    /// retrieve した chunks の concatenation の文字数。compression
    /// ratio 計算の分母。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_chars: Option<usize>,
    /// compressor 適用後の文字数。tier-0-1-2 のみ Some。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressed_chars: Option<usize>,
}

impl CaseMetric {
    /// LLM-bound (Full) ablation 用のコンストラクタ。retrieval-side
    /// フィールドは None で埋める。
    pub fn for_full(
        case_id: impl Into<String>,
        correct: bool,
        latency_ms: u64,
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
    ) -> Self {
        Self {
            case_id: case_id.into(),
            correct,
            latency_ms,
            prompt_tokens,
            completion_tokens,
            retrieval_latency_ms: None,
            retrieved_chunks: None,
            retrieval_chars: None,
            compressed_chars: None,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct AggregateMetric {
    pub total: usize,
    pub correct: usize,
    pub mean_latency_ms: f64,
}

impl AggregateMetric {
    pub fn from_cases(cases: &[CaseMetric]) -> Self {
        if cases.is_empty() {
            return Self::default();
        }
        let total = cases.len();
        let correct = cases.iter().filter(|c| c.correct).count();
        let sum_latency: u64 = cases.iter().map(|c| c.latency_ms).sum();
        Self {
            total,
            correct,
            mean_latency_ms: sum_latency as f64 / total as f64,
        }
    }

    pub fn accuracy(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.correct as f64 / self.total as f64
        }
    }
}

/// 大文字小文字を無視した部分一致。LongMemEval / MemoryAgentBench で
/// regex ベースの簡易判定に使う。
pub fn substring_match(answer: &str, expected: &str) -> bool {
    answer.to_lowercase().contains(&expected.to_lowercase())
}

/// 候補のいずれか 1 つ以上に部分一致すれば true。MemoryAgentBench
/// `Conflict_Resolution` の `answers[i]: List[String]` (同義語候補) に
/// 対応する。空配列に対しては常に false。
pub fn substring_match_any(answer: &str, candidates: &[String]) -> bool {
    candidates.iter().any(|c| substring_match(answer, c))
}

/// 圧縮率 = compressed / original。original が 0 のときは 1.0
/// (no-op として扱う)。Tier ablation matrix で tier-0-1-2 の
/// `TruncateCompressor` 適用前後の比率を記録するときに使う。
pub fn compression_ratio(original_chars: usize, compressed_chars: usize) -> f64 {
    if original_chars == 0 {
        return 1.0;
    }
    compressed_chars as f64 / original_chars as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(id: &str, ok: bool, ms: u64) -> CaseMetric {
        CaseMetric::for_full(id, ok, ms, None, None)
    }

    #[test]
    fn aggregate_handles_empty() {
        let agg = AggregateMetric::from_cases(&[]);
        assert_eq!(agg.total, 0);
        assert_eq!(agg.accuracy(), 0.0);
    }

    #[test]
    fn aggregate_computes_accuracy_and_mean_latency() {
        let cases = vec![
            case("a", true, 100),
            case("b", false, 300),
            case("c", true, 200),
        ];
        let agg = AggregateMetric::from_cases(&cases);
        assert_eq!(agg.total, 3);
        assert_eq!(agg.correct, 2);
        assert!((agg.accuracy() - 2.0 / 3.0).abs() < 1e-6);
        assert!((agg.mean_latency_ms - 200.0).abs() < 1e-6);
    }

    #[test]
    fn substring_match_is_case_insensitive() {
        assert!(substring_match("The Final Answer is FOO", "foo"));
        assert!(!substring_match("nope", "foo"));
    }

    #[test]
    fn substring_match_any_returns_true_when_any_candidate_matches() {
        let cands = vec![
            "Chief of Protocol".to_string(),
            "Protocol Officer".to_string(),
        ];
        assert!(substring_match_any(
            "Final answer: Chief of Protocol of the United States",
            &cands
        ));
        // 2nd 候補のみマッチ
        assert!(substring_match_any(
            "He served as a Protocol Officer until 1975.",
            &cands
        ));
    }

    #[test]
    fn substring_match_any_returns_false_when_no_candidate_matches() {
        let cands = vec!["Chief of Protocol".to_string()];
        assert!(!substring_match_any("Some other answer entirely", &cands));
    }

    #[test]
    fn substring_match_any_returns_false_for_empty_candidates() {
        // 候補が空 = 期待値なし → 常に false (false negative にしない)
        assert!(!substring_match_any("anything", &[]));
    }

    #[test]
    fn compression_ratio_returns_one_for_empty_original() {
        assert!((compression_ratio(0, 0) - 1.0).abs() < 1e-9);
        assert!((compression_ratio(0, 100) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compression_ratio_computes_fractional() {
        assert!((compression_ratio(1000, 250) - 0.25).abs() < 1e-9);
        assert!((compression_ratio(400, 400) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn case_metric_for_full_leaves_retrieval_fields_none() {
        let c = CaseMetric::for_full("q1", true, 100, Some(10), Some(20));
        assert!(c.retrieval_latency_ms.is_none());
        assert!(c.retrieved_chunks.is_none());
        assert!(c.retrieval_chars.is_none());
        assert!(c.compressed_chars.is_none());
    }

    #[test]
    fn case_metric_serializes_without_optional_retrieval_fields() {
        // 既存 jsonl (Step 2 で取得済) との互換維持: 新 Optional フィールド
        // が None のとき JSON 出力に key が出ないこと。
        let c = CaseMetric::for_full("q1", true, 100, Some(10), Some(20));
        let s = serde_json::to_string(&c).unwrap();
        assert!(!s.contains("retrieval_latency_ms"));
        assert!(!s.contains("retrieved_chunks"));
        assert!(!s.contains("retrieval_chars"));
        assert!(!s.contains("compressed_chars"));
    }
}
