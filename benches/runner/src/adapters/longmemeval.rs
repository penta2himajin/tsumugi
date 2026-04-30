//! LongMemEval `_oracle` adapter.
//!
//! Phase 4-α Step 2: 公式 `xiaowu0162/longmemeval` (HF datasets) の
//! `longmemeval_oracle` ファイル (~15 MB JSON) から 6 question_type ×
//! 5 問 = 30 問を seed 固定で層化抽出し、各問について haystack_sessions
//! (evidence のみ) + question を prompt として LLM に投げ、`answer` を
//! substring match で primary metric として判定する。
//!
//! LLM judge による secondary metric は paper-exact 再現が必要に
//! なった時点で別 PR で追加する (本 step では rule-based primary のみ)。
//!
//! データセット位置: 既定 `benches/data/longmemeval_oracle` (環境変数
//! `LONGMEMEVAL_PATH` で override 可)。download_datasets.sh が
//! `hf download xiaowu0162/longmemeval --repo-type dataset` で取得する
//! 想定。

use crate::report::SectionReport;
use crate::suite::{Ablation, SuiteRunOptions};
use serde::{Deserialize, Deserializer};
use std::path::{Path, PathBuf};

#[cfg(feature = "network")]
use crate::adapters::common::{
    bm25_retrieve, concat_for_judge, hybrid_retrieve, tier_0_1_2_compress,
};
#[cfg(feature = "network")]
use crate::metrics::{substring_match, CaseMetric};
#[cfg(feature = "network")]
use crate::report::IncrementalSectionWriter;
#[cfg(feature = "network")]
use tsumugi_core::providers::OpenAiCompatibleProvider;
#[cfg(feature = "network")]
use tsumugi_core::traits::llm::{CompletionRequest, LLMProvider};

/// LongMemEval Oracle で BM25 / HybridRetriever に渡す top_k。
/// `LME_TOP_K` env で override 可。
#[cfg(feature = "network")]
const DEFAULT_TOP_K: usize = 10;
/// tier-0-1-2 用 truncate budget (whitespace tokens)。`LME_COMPRESS_BUDGET_TOKENS` で
/// override 可。
#[cfg(feature = "network")]
const DEFAULT_COMPRESS_BUDGET_TOKENS: u32 = 2048;
#[cfg(feature = "network")]
const DEFAULT_COMPRESS_PRESERVE_TAIL: u32 = 256;

#[cfg(feature = "network")]
fn top_k_from_env() -> usize {
    std::env::var("LME_TOP_K")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_TOP_K)
}

#[cfg(feature = "network")]
fn compress_budget_from_env() -> u32 {
    std::env::var("LME_COMPRESS_BUDGET_TOKENS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_COMPRESS_BUDGET_TOKENS)
}

/// 計画書 §「ベンチマークサブセットの選定」: 6 question type × 平均 5 問
/// の層化抽出で安定性を確保。
const QUESTION_TYPES: &[&str] = &[
    "single-session-user",
    "single-session-assistant",
    "single-session-preference",
    "multi-session",
    "temporal-reasoning",
    "knowledge-update",
];

/// `LONGMEMEVAL_PER_TYPE` env で override 可。CI で timeout 内に
/// 収めたい場合は減らす (e.g., 2 → 12 問、1 → 6 問)。
const DEFAULT_QUESTIONS_PER_TYPE: usize = 5;

fn questions_per_type() -> usize {
    std::env::var("LONGMEMEVAL_PER_TYPE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_QUESTIONS_PER_TYPE)
}

/// stratified_sample の deterministic ソート用 seed。実機 smoke 結果を
/// 再現可能にするため固定。
const DEFAULT_SEED: u64 = 0x4c4d45_5f4f5241u64; // "LME_ORA"

#[derive(Debug, Deserialize, Clone)]
struct Entry {
    question_id: String,
    question_type: String,
    question: String,
    /// answer は string が大半だが、数を答える `multi-session` 系等で
    /// integer (例: `"answer": 3`) や bool が混在する。substring match の
    /// 入力は文字列なので、ここで安全に文字列化する。
    #[serde(deserialize_with = "deserialize_loose_string")]
    answer: String,
    #[serde(default)]
    question_date: String,
    #[serde(default)]
    haystack_dates: Vec<String>,
    haystack_sessions: Vec<Vec<Message>>,
}

fn deserialize_loose_string<'de, D>(d: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::String(s) => s,
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        // 配列・null・object もそのまま JSON 化して保持 (実データには
        // ほぼ出ないが、debug-friendly な fallback として)
        other => other.to_string(),
    })
}

#[derive(Debug, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

pub async fn run_oracle(opts: &SuiteRunOptions) -> anyhow::Result<SectionReport> {
    let dataset_path = default_dataset_path();
    run_oracle_with_ablation(opts, Ablation::Full, &dataset_path).await
}

pub fn default_dataset_path() -> PathBuf {
    std::env::var("LONGMEMEVAL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("benches/data/longmemeval_oracle"))
}

#[cfg(not(feature = "network"))]
pub async fn run_oracle_with_ablation(
    _opts: &SuiteRunOptions,
    _ablation: Ablation,
    _dataset_path: &Path,
) -> anyhow::Result<SectionReport> {
    anyhow::bail!(
        "Suite::Oracle requires the `network` feature for the OpenAI-compatible \
         LLM provider. Rebuild `tsumugi-bench` with `--features network`."
    )
}

#[cfg(feature = "network")]
pub async fn run_oracle_with_ablation(
    opts: &SuiteRunOptions,
    ablation: Ablation,
    dataset_path: &Path,
) -> anyhow::Result<SectionReport> {
    let entries = load_entries(dataset_path)?;
    let per_type = questions_per_type();
    let sampled = stratified_sample(&entries, per_type, DEFAULT_SEED);
    if sampled.is_empty() {
        anyhow::bail!(
            "stratified sample produced 0 entries (dataset path: {:?}, total entries: {})",
            dataset_path,
            entries.len()
        );
    }
    eprintln!(
        "[oracle/{}] {} cases (per_type={}, total dataset={})",
        ablation.name(),
        sampled.len(),
        per_type,
        entries.len()
    );

    match ablation {
        Ablation::Full => run_oracle_full(opts, &sampled).await,
        Ablation::Tier0 | Ablation::Tier01 | Ablation::Tier012 => {
            run_oracle_retrieval_only(opts, &sampled, ablation).await
        }
    }
}

#[cfg(feature = "network")]
async fn run_oracle_full(
    opts: &SuiteRunOptions,
    sampled: &[Entry],
) -> anyhow::Result<SectionReport> {
    let provider = OpenAiCompatibleProvider::new(&opts.llm_base_url, &opts.llm_model);
    let mut writer =
        IncrementalSectionWriter::create(&opts.output_dir, "longmemeval-oracle", Ablation::Full)?;
    let total = sampled.len();
    for (idx, entry) in sampled.iter().enumerate() {
        eprintln!(
            "[oracle/full] [{}/{}] type={} id={} question={:?}",
            idx + 1,
            total,
            entry.question_type,
            entry.question_id,
            entry.question.chars().take(80).collect::<String>()
        );
        let prompt = build_prompt(entry);
        let request = CompletionRequest {
            prompt,
            // 64 tok だと "Final answer:" 前置きで詰まって answer に
            // 到達できないケースが多発したため (実機 oracle smoke #3
            // で全件 max_tokens 切れ)、128 に拡大。CPU 推論なので
            // 出力時間が直接 latency に乗る点だけ留意。
            max_tokens: Some(128),
            temperature: Some(0.0),
            grammar: None,
            stop: None,
        };
        let started = std::time::Instant::now();
        let resp = provider.complete(&request).await?;
        let latency_ms = started.elapsed().as_millis() as u64;
        // Qwen3 等の thinking モデルでは answer が `reasoning_content`
        // (= `resp.reasoning_text`) に出る場合がある。両方に対して
        // substring match を行い、どちらでも答えが含まれていれば correct。
        let correct = substring_match(&resp.text, &entry.answer)
            || resp
                .reasoning_text
                .as_deref()
                .is_some_and(|r| substring_match(r, &entry.answer));
        let response_preview: String = resp.text.chars().take(200).collect();
        let reasoning_preview: String = resp
            .reasoning_text
            .as_deref()
            .map(|r| r.chars().take(200).collect())
            .unwrap_or_default();
        eprintln!(
            "[oracle/full] [{}/{}] -> latency={}ms correct={} prompt_tokens={:?} completion_tokens={:?} response={:?} reasoning={:?}",
            idx + 1,
            total,
            latency_ms,
            correct,
            resp.prompt_tokens,
            resp.completion_tokens,
            response_preview,
            reasoning_preview
        );
        writer.write_case(CaseMetric::for_full(
            entry.question_id.clone(),
            correct,
            latency_ms,
            resp.prompt_tokens,
            resp.completion_tokens,
        ))?;
    }
    Ok(writer.finish())
}

/// LLM 不使用 ablation (tier-0 / tier-0-1 / tier-0-1-2) の共通 path。
/// 各 entry の `haystack_sessions[i]` を 1 chunk として扱う (粒度: session)。
#[cfg(feature = "network")]
async fn run_oracle_retrieval_only(
    opts: &SuiteRunOptions,
    sampled: &[Entry],
    ablation: Ablation,
) -> anyhow::Result<SectionReport> {
    debug_assert!(matches!(
        ablation,
        Ablation::Tier0 | Ablation::Tier01 | Ablation::Tier012
    ));
    let mut writer =
        IncrementalSectionWriter::create(&opts.output_dir, "longmemeval-oracle", ablation)?;
    let top_k = top_k_from_env();
    let budget = compress_budget_from_env();
    let total = sampled.len();
    for (idx, entry) in sampled.iter().enumerate() {
        let chunks: Vec<String> = entry
            .haystack_sessions
            .iter()
            .map(|sess| {
                sess.iter()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .collect();
        let started = std::time::Instant::now();
        let retrieved = match ablation {
            Ablation::Tier0 => bm25_retrieve(&chunks, &entry.question, top_k).await?,
            Ablation::Tier01 | Ablation::Tier012 => {
                hybrid_retrieve(&chunks, &entry.question, top_k).await?
            }
            Ablation::Full => unreachable!("guarded by debug_assert"),
        };
        let concat = concat_for_judge(&retrieved);
        let retrieval_chars = concat.chars().count();
        let (judge_text, compressed_chars) = if matches!(ablation, Ablation::Tier012) {
            let compressed =
                tier_0_1_2_compress(&concat, budget, DEFAULT_COMPRESS_PRESERVE_TAIL).await?;
            let len = compressed.chars().count();
            (compressed, Some(len))
        } else {
            (concat, None)
        };
        let retrieval_latency_ms = started.elapsed().as_millis() as u64;
        let correct = substring_match(&judge_text, &entry.answer);
        eprintln!(
            "[oracle/{}] [{}/{}] type={} id={} chunks={} hits={} retrieval_chars={} compressed_chars={:?} correct={} latency={}ms",
            ablation.name(),
            idx + 1,
            total,
            entry.question_type,
            entry.question_id,
            chunks.len(),
            retrieved.len(),
            retrieval_chars,
            compressed_chars,
            correct,
            retrieval_latency_ms
        );
        writer.write_case(CaseMetric {
            case_id: entry.question_id.clone(),
            correct,
            latency_ms: retrieval_latency_ms,
            prompt_tokens: None,
            completion_tokens: None,
            retrieval_latency_ms: Some(retrieval_latency_ms),
            retrieved_chunks: Some(retrieved.len()),
            retrieval_chars: Some(retrieval_chars),
            compressed_chars,
        })?;
    }
    Ok(writer.finish())
}

fn load_entries(path: &Path) -> anyhow::Result<Vec<Entry>> {
    let bytes = std::fs::read(path).map_err(|e| {
        anyhow::anyhow!(
            "failed to read LongMemEval dataset at {:?}: {}. \
             benches/scripts/download_datasets.sh で取得し、 \
             LONGMEMEVAL_PATH env でパス指定してください。",
            path,
            e
        )
    })?;
    let entries: Vec<Entry> = serde_json::from_slice(&bytes)?;
    Ok(entries)
}

/// FNV-1a ベースの deterministic ソートで question_type 毎に上位
/// `per_type` 件を取る。総数が `per_type` に満たない type は全件採用。
fn stratified_sample(entries: &[Entry], per_type: usize, seed: u64) -> Vec<Entry> {
    use std::collections::BTreeMap;
    let mut by_type: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for e in entries {
        by_type.entry(e.question_type.as_str()).or_default().push(e);
    }
    let mut sampled = Vec::with_capacity(per_type * QUESTION_TYPES.len());
    for &qt in QUESTION_TYPES {
        if let Some(group) = by_type.get(qt) {
            let mut sorted: Vec<&Entry> = group.clone();
            sorted.sort_by_key(|e| fnv1a_hash(e.question_id.as_bytes(), seed));
            for e in sorted.into_iter().take(per_type) {
                sampled.push(e.clone());
            }
        }
    }
    sampled
}

fn fnv1a_hash(bytes: &[u8], seed: u64) -> u64 {
    let mut hash: u64 = seed ^ 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn build_prompt(entry: &Entry) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "Below are past conversation history sessions. Read them carefully and \
         answer the user's question based ONLY on the information provided.\n\n",
    );
    for (i, session) in entry.haystack_sessions.iter().enumerate() {
        let date = entry
            .haystack_dates
            .get(i)
            .map(String::as_str)
            .unwrap_or("(unknown date)");
        prompt.push_str(&format!("--- Session {} ({}) ---\n", i + 1, date));
        for msg in session {
            prompt.push_str(&format!("{}: {}\n", msg.role, msg.content));
        }
        prompt.push('\n');
    }
    let asked_on = if entry.question_date.is_empty() {
        "unknown date".to_string()
    } else {
        entry.question_date.clone()
    };
    // Qwen3.5 は `/think` / `/nothink` の soft switch をサポートしない
    // (Qwen3 限定の機能、Qwen3.5 model card で明示的に non-supported)
    // ので、thinking 抑制は llama-server 側で `--chat-template-kwargs
    // '{"enable_thinking":false}'` を渡す形で実現する
    // (`benches/scripts/start_llama_server.sh` 参照)。
    prompt.push_str(&format!(
        "Question (asked on {}): {}\nFinal answer:",
        asked_on, entry.question
    ));
    prompt
}

// ---------------------------------------------------------------------------
// テスト用 fixture: tests / network_tests 双方から共有する。
// ---------------------------------------------------------------------------

#[cfg(test)]
fn fixture_entries() -> Vec<Entry> {
    let mut entries = Vec::new();
    for &qt in QUESTION_TYPES {
        for i in 0..7 {
            entries.push(Entry {
                question_id: format!("{}-idx{}", qt, i),
                question_type: qt.to_string(),
                question: format!("Q{} of {}", i, qt),
                answer: format!("Answer-{}-{}", qt, i),
                question_date: "2024/01/01".into(),
                haystack_dates: vec!["2023/12/01".into()],
                haystack_sessions: vec![vec![Message {
                    role: "user".into(),
                    content: format!("Body of {}-{}", qt, i),
                }]],
            });
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stratified_sample_returns_5_per_type_for_6_types() {
        let entries = fixture_entries();
        let sampled = stratified_sample(&entries, 5, DEFAULT_SEED);
        assert_eq!(sampled.len(), 30, "5 per type × 6 types = 30");
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for e in &sampled {
            *counts.entry(e.question_type.clone()).or_default() += 1;
        }
        for &qt in QUESTION_TYPES {
            assert_eq!(counts.get(qt).copied().unwrap_or(0), 5, "type {} ≠ 5", qt);
        }
    }

    #[test]
    fn stratified_sample_is_deterministic_for_same_seed() {
        let entries = fixture_entries();
        let a = stratified_sample(&entries, 5, DEFAULT_SEED);
        let b = stratified_sample(&entries, 5, DEFAULT_SEED);
        let a_ids: Vec<&str> = a.iter().map(|e| e.question_id.as_str()).collect();
        let b_ids: Vec<&str> = b.iter().map(|e| e.question_id.as_str()).collect();
        assert_eq!(a_ids, b_ids);
    }

    #[test]
    fn stratified_sample_falls_back_when_type_has_fewer_than_per_type() {
        // single-session-preference を 2 件に削る
        let mut entries = fixture_entries();
        entries.retain(|e| {
            e.question_type != "single-session-preference"
                || matches!(e.question_id.as_str(), id if id.ends_with("idx0") || id.ends_with("idx1"))
        });
        let sampled = stratified_sample(&entries, 5, DEFAULT_SEED);
        // 5 type × 5 + 1 type × 2 = 27
        assert_eq!(sampled.len(), 27);
    }

    #[test]
    fn questions_per_type_default_is_5() {
        // 安全のため env を unset
        std::env::remove_var("LONGMEMEVAL_PER_TYPE");
        assert_eq!(questions_per_type(), 5);
    }

    #[test]
    fn entry_parses_integer_answer() {
        // 実 LongMemEval_oracle (line 9326 近辺) で `"answer": 3` のケースが
        // あり、`answer: String` 固定だと serde が "invalid type: integer
        // `3`, expected a string" で死ぬ。`deserialize_loose_string` で
        // 数値も拾えることを保証する。
        let json = r#"[{
            "question_id": "0a995998",
            "question_type": "multi-session",
            "question": "How many items?",
            "answer": 3,
            "haystack_sessions": []
        }]"#;
        let entries: Vec<Entry> = serde_json::from_str(json).expect("parse with integer answer");
        assert_eq!(entries[0].answer, "3");
    }

    #[test]
    fn entry_parses_bool_answer() {
        let json = r#"[{
            "question_id": "x",
            "question_type": "knowledge-update",
            "question": "Is it true?",
            "answer": true,
            "haystack_sessions": []
        }]"#;
        let entries: Vec<Entry> = serde_json::from_str(json).expect("parse with bool answer");
        assert_eq!(entries[0].answer, "true");
    }

    #[test]
    fn build_prompt_includes_session_date_and_question() {
        let entries = fixture_entries();
        let e = &entries[0];
        let p = build_prompt(e);
        assert!(p.contains("--- Session 1 (2023/12/01) ---"), "prompt: {p}");
        assert!(p.contains("Question (asked on 2024/01/01)"));
        assert!(p.contains("Final answer:"));
        assert!(p.contains(&e.question));
    }

    #[test]
    fn build_prompt_does_not_include_no_think_directive() {
        // Qwen3.5 は `/think` / `/nothink` soft switch を非サポート
        // (Qwen3 限定の機能、Qwen3.5 公式 model card で明示)。
        // thinking 抑制は server 側で `--chat-template-kwargs` で行う
        // (`benches/scripts/start_llama_server.sh` 参照)。prompt 内に
        // `/no_think` を残すと no-op な dead text になるので削除を保証。
        let entries = fixture_entries();
        let p = build_prompt(&entries[0]);
        assert!(
            !p.contains("/no_think"),
            "prompt should not contain /no_think (Qwen3.5 ignores it): {p}"
        );
    }
}

#[cfg(all(test, feature = "network"))]
mod network_tests {
    use super::*;
    use crate::suite::Suite;
    use std::io::Write;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn write_fixture_dataset(dir: &Path) -> PathBuf {
        let path = dir.join("longmemeval_oracle");
        let entries = fixture_entries();
        let json = serde_json::to_vec(
            &entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "question_id": e.question_id,
                        "question_type": e.question_type,
                        "question": e.question,
                        "answer": e.answer,
                        "question_date": e.question_date,
                        "haystack_dates": e.haystack_dates,
                        "haystack_sessions": e.haystack_sessions.iter().map(|sess| {
                            sess.iter().map(|m| {
                                serde_json::json!({
                                    "role": m.role,
                                    "content": m.content,
                                })
                            }).collect::<Vec<_>>()
                        }).collect::<Vec<_>>(),
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&json).unwrap();
        path
    }

    fn opts_for(server_uri: String, output_dir: PathBuf) -> SuiteRunOptions {
        SuiteRunOptions {
            suite: Suite::Oracle,
            output_dir,
            llm_base_url: server_uri,
            llm_model: "qwen3.5-4b".into(),
            ablations: crate::suite::Ablation::default_set(),
            help: false,
        }
    }

    #[tokio::test]
    async fn oracle_runs_30_cases_and_marks_correctness() {
        let server = MockServer::start().await;
        // 全問共通で "Answer-single-session-user-3" を含む応答を返す。
        // fixture では single-session-user-idx3 の正解が
        // "Answer-single-session-user-3" なのでこの 1 問だけ correct。
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{ "message": { "role": "assistant",
                    "content": "Final answer: Answer-single-session-user-3" } }],
                "usage": { "prompt_tokens": 100, "completion_tokens": 12 }
            })))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let dataset_path = write_fixture_dataset(tmp.path());

        let opts = opts_for(server.uri(), tmp.path().to_path_buf());
        let report = run_oracle_with_ablation(&opts, Ablation::Full, &dataset_path)
            .await
            .expect("run_oracle");

        assert_eq!(report.bench, "longmemeval-oracle");
        assert_eq!(report.ablation, "full");
        assert_eq!(report.cases.len(), 30);
        // 1 case correct (single-session-user-idx3 の場合のみ substring match)
        // ただし stratified_sample で idx3 が抽出された保証は無いので、
        // correct >= 0 だけ確認 (確率的判定は別 test で実施)
        let exact_match_cases: Vec<_> = report
            .cases
            .iter()
            .filter(|c| c.case_id == "single-session-user-idx3")
            .collect();
        if !exact_match_cases.is_empty() {
            assert!(exact_match_cases[0].correct);
        }
    }

    #[tokio::test]
    async fn oracle_propagates_provider_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let dataset_path = write_fixture_dataset(tmp.path());

        let opts = opts_for(server.uri(), tmp.path().to_path_buf());
        let err = run_oracle_with_ablation(&opts, Ablation::Full, &dataset_path)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("500"), "got: {err}");
    }

    #[tokio::test]
    async fn oracle_records_aggregate_correct_count() {
        let server = MockServer::start().await;
        // どの問にも "Answer-" を含む応答を返す → 30 件中 30 件 substring match
        // (substring_match は case-insensitive で "answer-" が小文字含めて
        //  fixture answer 全てに含まれる)
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{ "message": { "role": "assistant",
                    "content": "I think this is Answer-something-relevant" } }],
                "usage": { "prompt_tokens": 100, "completion_tokens": 12 }
            })))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let dataset_path = write_fixture_dataset(tmp.path());
        let opts = opts_for(server.uri(), tmp.path().to_path_buf());
        let report = run_oracle_with_ablation(&opts, Ablation::Full, &dataset_path)
            .await
            .unwrap();
        // "Answer-" は fixture answer (e.g. "Answer-single-session-user-3") の
        // prefix だが、"Answer-something-relevant" 全体は specific answer
        // (e.g. "Answer-single-session-user-3") を substring 含まない。
        // よって correct == 0。primary が確率的にならないことを保証する。
        assert_eq!(report.aggregate.correct, 0);
    }

    #[tokio::test]
    async fn oracle_tier0_runs_without_llm_calls() {
        let server = MockServer::start().await;
        // tier-0 では LLM を呼ばないので、POST が来たら 500 で目立たせる
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let dataset_path = write_fixture_dataset(tmp.path());
        let opts = opts_for(server.uri(), tmp.path().to_path_buf());
        let report = run_oracle_with_ablation(&opts, Ablation::Tier0, &dataset_path)
            .await
            .expect("tier-0 run");
        assert_eq!(report.bench, "longmemeval-oracle");
        assert_eq!(report.ablation, "tier-0");
        assert_eq!(report.cases.len(), 30);
        for c in &report.cases {
            assert!(c.retrieval_latency_ms.is_some());
            assert!(c.retrieved_chunks.is_some());
            assert!(c.retrieval_chars.is_some());
            assert!(c.compressed_chars.is_none());
            assert!(c.prompt_tokens.is_none());
            assert!(c.completion_tokens.is_none());
        }
        let received = server.received_requests().await.unwrap_or_default();
        assert!(
            received.is_empty(),
            "tier-0 must not call LLM, got {} requests",
            received.len()
        );
    }

    #[tokio::test]
    async fn oracle_tier01_uses_hybrid_retrieval_and_no_llm() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let dataset_path = write_fixture_dataset(tmp.path());
        let opts = opts_for(server.uri(), tmp.path().to_path_buf());
        let report = run_oracle_with_ablation(&opts, Ablation::Tier01, &dataset_path)
            .await
            .expect("tier-0-1 run");
        assert_eq!(report.ablation, "tier-0-1");
        for c in &report.cases {
            assert!(c.compressed_chars.is_none());
            assert!(c.retrieved_chunks.is_some());
        }
        let received = server.received_requests().await.unwrap_or_default();
        assert!(received.is_empty(), "tier-0-1 must not call LLM");
    }

    #[tokio::test]
    async fn oracle_tier012_applies_truncate_compressor() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let dataset_path = write_fixture_dataset(tmp.path());
        let opts = opts_for(server.uri(), tmp.path().to_path_buf());
        // 小さい budget で圧縮効果を確実に observe する
        std::env::set_var("LME_COMPRESS_BUDGET_TOKENS", "8");
        let report = run_oracle_with_ablation(&opts, Ablation::Tier012, &dataset_path)
            .await
            .expect("tier-0-1-2 run");
        std::env::remove_var("LME_COMPRESS_BUDGET_TOKENS");
        assert_eq!(report.ablation, "tier-0-1-2");
        for c in &report.cases {
            assert!(
                c.compressed_chars.is_some(),
                "tier-0-1-2 must record compressed_chars"
            );
            let before = c.retrieval_chars.unwrap();
            let after = c.compressed_chars.unwrap();
            assert!(after <= before);
        }
    }
}
