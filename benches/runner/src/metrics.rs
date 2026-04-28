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

#[cfg(test)]
mod tests {
    use super::*;

    fn case(id: &str, ok: bool, ms: u64) -> CaseMetric {
        CaseMetric {
            case_id: id.into(),
            correct: ok,
            latency_ms: ms,
            prompt_tokens: None,
            completion_tokens: None,
        }
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
}
