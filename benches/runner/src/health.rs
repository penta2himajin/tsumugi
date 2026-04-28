//! Phase 4-α Step 1 v0 smoke: LLM 起動健全性 + 生成速度 (tok/s) +
//! 簡易指示追従。
//!
//! 計画書 §「主候補 smoke test (Step 1 で実施)」のうち「起動成功率」
//! 「生成速度」「指示追従性」の 3 軸を最小プロンプトで検証する。
//! RULER NIAH-S と LongMemEval_oracle 5 問の判定は Step 2-3 の adapter
//! 実装に従って `Suite::Smoke` / `Suite::Oracle` で別途回す。
//!
//! `network` feature が無効な場合は明示的に bail する (`tsumugi-core` の
//! `OpenAiCompatibleProvider` が無効化されているため)。

use crate::report::SectionReport;
use crate::suite::SuiteRunOptions;

#[cfg(feature = "network")]
use crate::metrics::{substring_match, CaseMetric};
#[cfg(feature = "network")]
use crate::suite::Ablation;
#[cfg(feature = "network")]
use tsumugi_core::providers::OpenAiCompatibleProvider;
#[cfg(feature = "network")]
use tsumugi_core::traits::llm::{CompletionRequest, LLMProvider};

/// 既定では同一 prompt を 3 回投げて起動安定性 + 生成速度のばらつきを見る。
const DEFAULT_TRIALS: usize = 3;

/// 各 prompt は決定的に判定できるよう「Final answer:」プレフィックスで答えを
/// 抽出する。temperature=0 で seed 固定にしても LLM 側の non-determinism
/// で揺れる可能性があるため、判定は substring match に留める。
const PROBES: &[HealthProbe] = &[
    HealthProbe {
        case_id: "arith-2-plus-2",
        prompt: "Reply with the digit only. What is 2 + 2? Final answer:",
        expected_substring: "4",
    },
    HealthProbe {
        case_id: "capital-of-japan",
        prompt: "Reply with the city name only. What is the capital of Japan? Final answer:",
        expected_substring: "Tokyo",
    },
];

struct HealthProbe {
    case_id: &'static str,
    prompt: &'static str,
    expected_substring: &'static str,
}

pub async fn run_health(opts: &SuiteRunOptions) -> anyhow::Result<SectionReport> {
    run_health_inner(opts, DEFAULT_TRIALS).await
}

#[cfg(feature = "network")]
async fn run_health_inner(opts: &SuiteRunOptions, trials: usize) -> anyhow::Result<SectionReport> {
    let provider = OpenAiCompatibleProvider::new(&opts.llm_base_url, &opts.llm_model);
    let mut cases: Vec<CaseMetric> = Vec::with_capacity(trials * PROBES.len());
    for trial in 0..trials {
        for probe in PROBES {
            let request = CompletionRequest {
                prompt: probe.prompt.into(),
                max_tokens: Some(16),
                temperature: Some(0.0),
                grammar: None,
                stop: None,
            };
            let started = std::time::Instant::now();
            let resp = provider.complete(&request).await?;
            let latency_ms = started.elapsed().as_millis() as u64;
            cases.push(CaseMetric {
                case_id: format!("{}-trial-{}", probe.case_id, trial),
                correct: substring_match(&resp.text, probe.expected_substring),
                latency_ms,
                prompt_tokens: resp.prompt_tokens,
                completion_tokens: resp.completion_tokens,
            });
        }
    }
    Ok(SectionReport::new("llm-health", Ablation::Full, cases))
}

#[cfg(not(feature = "network"))]
async fn run_health_inner(
    _opts: &SuiteRunOptions,
    _trials: usize,
) -> anyhow::Result<SectionReport> {
    anyhow::bail!(
        "Suite::Health requires the `network` feature for the OpenAI-compatible \
         LLM provider. Rebuild `tsumugi-bench` with `--features network`."
    )
}

#[cfg(all(test, feature = "network"))]
mod tests {
    use super::*;
    use crate::suite::Suite;
    use std::path::PathBuf;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn opts_for(server_uri: String) -> SuiteRunOptions {
        SuiteRunOptions {
            suite: Suite::Health,
            output_dir: PathBuf::from("/tmp/ignored"),
            llm_base_url: server_uri,
            llm_model: "qwen3.5-4b-instruct".into(),
            help: false,
        }
    }

    fn mock_response(content: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": content } }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 1 }
        }))
    }

    #[tokio::test]
    async fn health_probes_each_trial_marks_correct_when_substring_matches() {
        let server = MockServer::start().await;
        let scripted = serde_json::json!({
            "choices": [
                { "message": { "role": "assistant", "content": "Final answer: 4" } }
            ],
            "usage": { "prompt_tokens": 10, "completion_tokens": 4 }
        });
        // どの probe にも同じ応答を返す mock — substring 判定は probe 個別。
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(scripted))
            .mount(&server)
            .await;

        let opts = opts_for(server.uri());
        let report = run_health_inner(&opts, 2).await.expect("run_health");
        assert_eq!(report.bench, "llm-health");
        assert_eq!(report.ablation, "full");
        // 2 trials × 2 probes = 4 cases。"4" は arith にマッチ、capital には外れる。
        assert_eq!(report.cases.len(), 4);
        let arith_correct = report
            .cases
            .iter()
            .filter(|c| c.case_id.starts_with("arith") && c.correct)
            .count();
        let capital_correct = report
            .cases
            .iter()
            .filter(|c| c.case_id.starts_with("capital") && c.correct)
            .count();
        assert_eq!(arith_correct, 2, "arith should match `4`");
        assert_eq!(capital_correct, 0, "capital should not match `Tokyo`");
    }

    #[tokio::test]
    async fn health_propagates_provider_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let opts = opts_for(server.uri());
        let err = run_health_inner(&opts, 1).await.unwrap_err();
        assert!(err.to_string().contains("503"), "got: {err}");
    }

    #[tokio::test]
    async fn health_marks_each_probe_correct_when_response_matches_per_probe() {
        let server = MockServer::start().await;
        // POST が 2 回来るので順に異なる応答を返す。
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(mock_response("Final answer: 4"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(mock_response("Final answer: Tokyo"))
            .mount(&server)
            .await;

        let opts = opts_for(server.uri());
        let report = run_health_inner(&opts, 1).await.expect("run_health");
        assert_eq!(report.cases.len(), 2);
        assert!(report.cases.iter().all(|c| c.correct));
        assert_eq!(report.aggregate.correct, 2);
    }
}

#[cfg(all(test, not(feature = "network")))]
mod stub_tests {
    use super::*;
    use crate::suite::Suite;
    use std::path::PathBuf;

    #[tokio::test]
    async fn health_bails_without_network_feature() {
        let opts = SuiteRunOptions {
            suite: Suite::Health,
            output_dir: PathBuf::from("/tmp/ignored"),
            llm_base_url: "http://unreachable".into(),
            llm_model: "qwen3.5-4b-instruct".into(),
            help: false,
        };
        let err = run_health_inner(&opts, 1).await.unwrap_err();
        assert!(err.to_string().contains("`network` feature"));
    }
}
