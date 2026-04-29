//! RULER NIAH-S (Single needle in a haystack) adapter.
//!
//! 計画書 §「ベンチマークサブセットの選定」: `niah_single_2` を seq_len ∈
//! {4K, 8K, 16K, 32K, 64K} で各 1 ケース、計 5 ケース。Tier 0 (BM25) baseline
//! の確認用。
//!
//! 本実装は **CPU smoke 環境向け**の合成生成版:
//! - 公式 RULER は Paul Graham essays を haystack に使う (paper-exact)
//!   が、本 adapter は deterministic な lorem-ipsum 系合成で代替する
//! - 公式 5 サイズ (4K/8K/16K/32K/64K) は CPU + 4B model + 16K llama-server
//!   ctx_size に収まらないため、デフォルトを {2K, 4K, 8K, 12K} の 4 ケースに
//!   絞る。GPU 環境で大きな ctx_size が使える場合は env `RULER_SEQ_LENGTHS`
//!   で `4096,8192,16384` 等に切替可能 (ただし llama-server の `--ctx-size`
//!   も合わせて引き上げること)
//!
//! 評価方法: needle に埋め込んだ value を期待値とし、応答に含まれていれば
//! correct (substring match)。Tier 0 baseline として、LLM 不使用の
//! `Bm25Retriever` でも同 needle 検出をテストする ablation は Step 3 後半
//! (Tier ablation matrix) で対応する。

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

/// CPU + 4B model + 16K llama-server ctx 向けの保守的サイズ。
/// `RULER_SEQ_LENGTHS=2048,4096,8192,12288` の形で env 上書き可能。
const DEFAULT_SEQ_LENGTHS: &[usize] = &[2048, 4096, 8192, 12288];

/// case 生成の deterministic seed (実機 smoke 結果を再現可能にするため固定)。
const DEFAULT_SEED: u64 = 0x52554c4552u64; // "RULER"

/// haystack に埋め込む needle の文。`{key}` / `{value}` をプレースホルダ
/// として使う。
const NEEDLE_TEMPLATE: &str =
    "The magic key {key} has the special value {value}. Remember this fact.";

/// haystack を埋める filler 単語のプール。決定的選択でランダム性をシミュレート
/// する。実 RULER の Paul Graham essays とは比較できないが、長い context
/// 内に needle を埋めるという smoke 検証としては十分。
const FILLER_WORDS: &[&str] = &[
    "the",
    "of",
    "and",
    "to",
    "in",
    "is",
    "for",
    "with",
    "as",
    "by",
    "are",
    "was",
    "were",
    "be",
    "ai",
    "system",
    "memory",
    "long",
    "context",
    "language",
    "model",
    "evaluation",
    "benchmark",
    "retrieval",
    "agent",
    "task",
    "data",
    "training",
    "inference",
    "prompt",
    "token",
    "vector",
    "embedding",
    "store",
    "database",
    "query",
    "index",
    "score",
    "rank",
    "compress",
    "summarize",
    "session",
    "chunk",
    "fact",
    "pending",
    "decision",
    "review",
    "write",
    "read",
    "save",
    "load",
];

#[derive(Debug, Clone)]
struct NiahCase {
    case_id: String,
    seq_len: usize,
    haystack: String,
    needle_key: String,
    needle_value: String,
}

fn seq_lengths_from_env() -> Vec<usize> {
    std::env::var("RULER_SEQ_LENGTHS")
        .ok()
        .and_then(|s| {
            s.split(',')
                .map(|x| x.trim().parse::<usize>().ok())
                .collect::<Option<Vec<usize>>>()
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_SEQ_LENGTHS.to_vec())
}

fn fnv1a_hash(bytes: &[u8], seed: u64) -> u64 {
    let mut hash: u64 = seed ^ 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 1 ステップ進めた xorshift64 状の deterministic generator。
fn next_seed(state: u64) -> u64 {
    let mut x = state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

/// 約 `target_chars` 文字の filler text を生成する。
fn synthesize_haystack(target_chars: usize, seed: u64) -> String {
    let mut buf = String::with_capacity(target_chars + 64);
    let mut state = seed;
    while buf.len() < target_chars {
        state = next_seed(state);
        let idx = (state as usize) % FILLER_WORDS.len();
        if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(FILLER_WORDS[idx]);
    }
    buf
}

/// (key, value, full_needle_sentence) を返す。
fn synthesize_needle(seed: u64) -> (String, String, String) {
    // key: 8 桁 hex、value: 6 桁 alphanumeric (deterministic seed から導出)
    let key_hash = fnv1a_hash(b"key", seed);
    let value_hash = fnv1a_hash(b"value", seed);
    let key = format!("{:08x}", key_hash & 0xffff_ffff);
    let value = format!("{:08x}", value_hash & 0xffff_ffff)
        .chars()
        .take(8)
        .collect::<String>();
    let needle = NEEDLE_TEMPLATE
        .replace("{key}", &key)
        .replace("{value}", &value);
    (key, value, needle)
}

fn build_cases(seed: u64, seq_lengths: &[usize]) -> Vec<NiahCase> {
    seq_lengths
        .iter()
        .enumerate()
        .map(|(i, &seq_len)| {
            let case_seed = fnv1a_hash(format!("niah-{}", seq_len).as_bytes(), seed ^ i as u64);
            let (needle_key, needle_value, needle_sentence) = synthesize_needle(case_seed);
            // ~4 chars/token 換算で target chars。needle 分は後で挿入されて加算される。
            let target_chars = seq_len.saturating_mul(4);
            let mut haystack = synthesize_haystack(target_chars, case_seed);
            // needle を中間付近 (deterministic) に挿入。単語境界で挿入するため
            // 直前のスペースを探す。
            let raw_pos = (case_seed as usize) % haystack.len().max(1);
            let insert_pos = haystack[..raw_pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
            haystack.insert_str(insert_pos, &format!("{} ", needle_sentence));
            NiahCase {
                case_id: format!("niah-{}k", seq_len / 1024),
                seq_len,
                haystack,
                needle_key,
                needle_value,
            }
        })
        .collect()
}

fn build_prompt(case: &NiahCase) -> String {
    format!(
        "Read the following document carefully. \
         Some special key-value pairs are hidden inside the text.\n\n\
         === DOCUMENT START ===\n{}\n=== DOCUMENT END ===\n\n\
         Question: What is the special value associated with the key '{}' in the document?\n\
         Answer with only the value (no explanation).\n\
         Final answer:",
        case.haystack, case.needle_key
    )
}

pub async fn run_niah_s(opts: &SuiteRunOptions) -> anyhow::Result<SectionReport> {
    run_niah_s_inner(opts).await
}

#[cfg(feature = "network")]
async fn run_niah_s_inner(opts: &SuiteRunOptions) -> anyhow::Result<SectionReport> {
    let seq_lengths = seq_lengths_from_env();
    let cases = build_cases(DEFAULT_SEED, &seq_lengths);
    eprintln!(
        "[smoke] {} RULER NIAH-S cases (seq_lengths={:?})",
        cases.len(),
        seq_lengths
    );

    let provider = OpenAiCompatibleProvider::new(&opts.llm_base_url, &opts.llm_model);
    let mut metrics = Vec::with_capacity(cases.len());
    let total = cases.len();
    for (idx, case) in cases.iter().enumerate() {
        eprintln!(
            "[smoke] [{}/{}] case={} seq_len={} haystack_chars={} needle_key={} needle_value={}",
            idx + 1,
            total,
            case.case_id,
            case.seq_len,
            case.haystack.len(),
            case.needle_key,
            case.needle_value
        );
        let prompt = build_prompt(case);
        let request = CompletionRequest {
            prompt,
            max_tokens: Some(64),
            temperature: Some(0.0),
            grammar: None,
            stop: None,
        };
        let started = std::time::Instant::now();
        let resp = provider.complete(&request).await?;
        let latency_ms = started.elapsed().as_millis() as u64;
        let correct = substring_match(&resp.text, &case.needle_value)
            || resp
                .reasoning_text
                .as_deref()
                .is_some_and(|r| substring_match(r, &case.needle_value));
        let response_preview: String = resp.text.chars().take(200).collect();
        let reasoning_preview: String = resp
            .reasoning_text
            .as_deref()
            .map(|r| r.chars().take(200).collect())
            .unwrap_or_default();
        eprintln!(
            "[smoke] [{}/{}] -> latency={}ms correct={} prompt_tokens={:?} completion_tokens={:?} response={:?} reasoning={:?}",
            idx + 1,
            total,
            latency_ms,
            correct,
            resp.prompt_tokens,
            resp.completion_tokens,
            response_preview,
            reasoning_preview
        );
        metrics.push(CaseMetric {
            case_id: case.case_id.clone(),
            correct,
            latency_ms,
            prompt_tokens: resp.prompt_tokens,
            completion_tokens: resp.completion_tokens,
        });
    }
    Ok(SectionReport::new("ruler-niah-s", Ablation::Full, metrics))
}

#[cfg(not(feature = "network"))]
async fn run_niah_s_inner(_opts: &SuiteRunOptions) -> anyhow::Result<SectionReport> {
    anyhow::bail!(
        "Suite::Smoke requires the `network` feature for the OpenAI-compatible \
         LLM provider. Rebuild `tsumugi-bench` with `--features network`."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_cases_uses_default_seq_lengths() {
        let cases = build_cases(DEFAULT_SEED, DEFAULT_SEQ_LENGTHS);
        assert_eq!(cases.len(), DEFAULT_SEQ_LENGTHS.len());
        let ids: Vec<&str> = cases.iter().map(|c| c.case_id.as_str()).collect();
        assert_eq!(ids, vec!["niah-2k", "niah-4k", "niah-8k", "niah-12k"]);
    }

    #[test]
    fn build_cases_is_deterministic_for_same_seed() {
        let a = build_cases(DEFAULT_SEED, DEFAULT_SEQ_LENGTHS);
        let b = build_cases(DEFAULT_SEED, DEFAULT_SEQ_LENGTHS);
        for (ca, cb) in a.iter().zip(b.iter()) {
            assert_eq!(ca.needle_key, cb.needle_key);
            assert_eq!(ca.needle_value, cb.needle_value);
            assert_eq!(ca.haystack, cb.haystack);
        }
    }

    #[test]
    fn haystack_size_approx_target() {
        let cases = build_cases(DEFAULT_SEED, &[4096]);
        let case = &cases[0];
        // target は seq_len * 4 = 16384 chars。needle 文 (~80 chars) 分の
        // 余裕を見て、target ± 100 chars 範囲で生成されること。
        let target = 4096 * 4;
        let actual = case.haystack.len();
        assert!(
            actual >= target && actual < target + 200,
            "haystack chars {} not in [{}..{})",
            actual,
            target,
            target + 200
        );
    }

    #[test]
    fn haystack_contains_needle_key_and_value() {
        let cases = build_cases(DEFAULT_SEED, &[2048]);
        let case = &cases[0];
        assert!(
            case.haystack.contains(&case.needle_key),
            "haystack missing needle key {}",
            case.needle_key
        );
        assert!(
            case.haystack.contains(&case.needle_value),
            "haystack missing needle value {}",
            case.needle_value
        );
    }

    #[test]
    fn build_prompt_includes_question_and_key() {
        let cases = build_cases(DEFAULT_SEED, &[2048]);
        let p = build_prompt(&cases[0]);
        assert!(p.contains(&cases[0].needle_key));
        assert!(p.contains("Final answer:"));
        assert!(p.contains("=== DOCUMENT START ==="));
    }

    #[test]
    fn seq_lengths_from_env_falls_back_to_default() {
        std::env::remove_var("RULER_SEQ_LENGTHS");
        assert_eq!(seq_lengths_from_env(), DEFAULT_SEQ_LENGTHS.to_vec());
    }
}

#[cfg(all(test, feature = "network"))]
mod network_tests {
    use super::*;
    use crate::suite::Suite;
    use std::path::PathBuf;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn opts_for(server_uri: String) -> SuiteRunOptions {
        SuiteRunOptions {
            suite: Suite::Smoke,
            output_dir: PathBuf::from("/tmp/ignored"),
            llm_base_url: server_uri,
            llm_model: "qwen3.5-4b".into(),
            help: false,
        }
    }

    #[tokio::test]
    async fn niah_s_runs_each_case_and_marks_correctness() {
        let server = MockServer::start().await;
        // 全リクエストに固定 value で応答。1 件目の case の needle_value が
        // 含まれる前提なので、その 1 件だけ correct=true、他は false に
        // なる確率が高い。
        let cases = build_cases(DEFAULT_SEED, DEFAULT_SEQ_LENGTHS);
        let lucky_value = cases[0].needle_value.clone();
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": format!("The value is {}", lucky_value),
                    }
                }],
                "usage": { "prompt_tokens": 1000, "completion_tokens": 6 }
            })))
            .mount(&server)
            .await;

        let opts = opts_for(server.uri());
        let report = run_niah_s_inner(&opts).await.expect("run");
        assert_eq!(report.bench, "ruler-niah-s");
        assert_eq!(report.ablation, "full");
        assert_eq!(report.cases.len(), DEFAULT_SEQ_LENGTHS.len());
        // 1 件目の case の needle_value が応答に含まれるので少なくとも 1 件 correct
        assert!(report.cases.iter().any(|c| c.correct));
        // それ以外は別の needle_value (case ごとに異なる) なので不一致
        let first_correct_id = report.cases.iter().find(|c| c.correct).map(|c| &c.case_id);
        assert_eq!(first_correct_id, Some(&"niah-2k".to_string()));
    }

    #[tokio::test]
    async fn niah_s_propagates_provider_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let opts = opts_for(server.uri());
        let err = run_niah_s_inner(&opts).await.unwrap_err();
        assert!(err.to_string().contains("500"), "got: {err}");
    }
}
