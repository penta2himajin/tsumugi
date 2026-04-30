//! MemoryAgentBench `Conflict_Resolution` adapter.
//!
//! Phase 4-α Step 3 PR ②: HF dataset `ai-hyz/MemoryAgentBench` の
//! `Conflict_Resolution` split は **8 行 × 60-100 QA / 行**で、各行の
//! `context` は 273k-3.17M chars (約 70K-800K tokens) と llama-server
//! `--ctx-size 16384` を大きく超える。
//!
//! 計画書の「全 8 問」は **8 行 × `questions[0]` = 8 評価ケース** と解釈する。
//!
//! Context truncation 戦略: `tsumugi_core::retriever::Bm25Retriever` で
//! chunk_size 1024 tok (≒ 4096 chars) / top_k 10 の retrieval を行い、
//! ~10K tok 程度に圧縮した上で LLM に投げる。BM25 hit が `top_k/2` 未満
//! になった場合は context 末尾 ~10K tok でフォールバック (CR の supersession
//! 仮説と整合: 新しい事実は document 末尾近辺に集中する傾向)。
//!
//! 正解判定: `answers[i]` は `List[String]` の同義語候補。
//! `substring_match_any` でいずれか 1 つに部分一致すれば correct。
//!
//! データ位置: 既定 `benches/data/memoryagentbench_cr.jsonl`。
//! 環境変数 `MAB_CR_PATH` で override 可。`download_datasets.sh` が
//! parquet → JSONL 変換 (pyarrow 経由) を担当する。

use crate::report::SectionReport;
use crate::suite::SuiteRunOptions;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[cfg(feature = "network")]
use crate::metrics::{substring_match_any, CaseMetric};
#[cfg(feature = "network")]
use crate::report::IncrementalSectionWriter;
#[cfg(feature = "network")]
use crate::suite::Ablation;
#[cfg(feature = "network")]
use std::collections::HashMap;
#[cfg(feature = "network")]
use tsumugi_core::domain::ChunkId;
#[cfg(feature = "network")]
use tsumugi_core::providers::OpenAiCompatibleProvider;
#[cfg(feature = "network")]
use tsumugi_core::retriever::Bm25Retriever;
#[cfg(feature = "network")]
use tsumugi_core::traits::llm::{CompletionRequest, LLMProvider};
#[cfg(feature = "network")]
use tsumugi_core::traits::retriever::Retriever;

/// 1 chunk あたりのターゲット文字数。English ASCII で 4 chars/token と仮定し
/// 1024 token を保守的に約 4096 chars にする。`CR_CHUNK_CHARS` env で override 可。
const DEFAULT_CHUNK_CHARS: usize = 4096;

/// BM25 retrieval の取得件数。`top_k * chunk_chars` が prompt budget の主要部分。
/// `CR_TOP_K` env で override 可。
const DEFAULT_TOP_K: usize = 10;

/// BM25 fallback (末尾切り出し) のサイズ。約 10K tokens。
const DEFAULT_FALLBACK_TAIL_CHARS: usize = 40_000;

/// 1 行あたりに評価する questions の数。default=1 で `questions[0]` のみ。
/// `CR_QUESTIONS_PER_ROW` env で 1..N に拡張可能 (各 row × per_row case を生成)。
const DEFAULT_QUESTIONS_PER_ROW: usize = 1;

#[derive(Debug, Deserialize, Clone)]
struct Entry {
    context: String,
    questions: Vec<String>,
    /// 各 question に対する複数正解候補。`answers[i]` が `List[String]`。
    /// 例: `[["yes"], ["Chief of Protocol", "Chief Protocol Officer"]]`
    answers: Vec<Vec<String>>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Debug, Clone)]
struct CrCase {
    case_id: String,
    /// 全文。retrieve は per-case で行うので Vec<String> ではなく String で持つ。
    context: String,
    question: String,
    /// 同義語候補リスト (空配列はあり得ないが defensive)
    answers: Vec<String>,
}

fn chunk_chars_from_env() -> usize {
    std::env::var("CR_CHUNK_CHARS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 256)
        .unwrap_or(DEFAULT_CHUNK_CHARS)
}

fn top_k_from_env() -> usize {
    std::env::var("CR_TOP_K")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_TOP_K)
}

fn questions_per_row_from_env() -> usize {
    std::env::var("CR_QUESTIONS_PER_ROW")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_QUESTIONS_PER_ROW)
}

pub async fn run_conflict_resolution(opts: &SuiteRunOptions) -> anyhow::Result<SectionReport> {
    let dataset_path = std::env::var("MAB_CR_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("benches/data/memoryagentbench_cr.jsonl"));
    run_with_dataset(opts, &dataset_path).await
}

#[cfg(feature = "network")]
async fn run_with_dataset(opts: &SuiteRunOptions, path: &Path) -> anyhow::Result<SectionReport> {
    let entries = load_entries(path)?;
    let per_row = questions_per_row_from_env();
    let cases = build_cases(&entries, per_row);
    if cases.is_empty() {
        anyhow::bail!(
            "MemoryAgentBench CR produced 0 cases (dataset path: {:?}, rows: {}, per_row: {})",
            path,
            entries.len(),
            per_row
        );
    }
    let chunk_chars = chunk_chars_from_env();
    let top_k = top_k_from_env();
    eprintln!(
        "[cr] {} cases ({} rows × {} questions/row), chunk_chars={}, top_k={}",
        cases.len(),
        entries.len(),
        per_row,
        chunk_chars,
        top_k
    );

    let provider = OpenAiCompatibleProvider::new(&opts.llm_base_url, &opts.llm_model);
    let mut writer =
        IncrementalSectionWriter::create(&opts.output_dir, "memoryagentbench-cr", Ablation::Full)?;
    let total = cases.len();
    for (idx, case) in cases.iter().enumerate() {
        let chunks = chunk_text(&case.context, chunk_chars);
        let retrieved = bm25_retrieve(&chunks, &case.question, top_k).await?;
        let used_fallback = retrieved.len() < top_k.div_ceil(2);
        let context_block = if used_fallback {
            tail_chars(&case.context, DEFAULT_FALLBACK_TAIL_CHARS)
        } else {
            retrieved.join("\n\n---\n\n")
        };
        eprintln!(
            "[cr] [{}/{}] case={} chunks={} hits={} ctx_chars={} fallback={} answers={:?}",
            idx + 1,
            total,
            case.case_id,
            chunks.len(),
            retrieved.len(),
            context_block.len(),
            used_fallback,
            case.answers
        );
        let prompt = build_prompt(&case.question, &context_block);
        let request = CompletionRequest {
            prompt,
            max_tokens: Some(128),
            temperature: Some(0.0),
            grammar: None,
            stop: None,
        };
        let started = std::time::Instant::now();
        let resp = provider.complete(&request).await?;
        let latency_ms = started.elapsed().as_millis() as u64;
        let correct = substring_match_any(&resp.text, &case.answers)
            || resp
                .reasoning_text
                .as_deref()
                .is_some_and(|r| substring_match_any(r, &case.answers));
        let response_preview: String = resp.text.chars().take(200).collect();
        let reasoning_preview: String = resp
            .reasoning_text
            .as_deref()
            .map(|r| r.chars().take(200).collect())
            .unwrap_or_default();
        eprintln!(
            "[cr] [{}/{}] -> latency={}ms correct={} prompt_tokens={:?} completion_tokens={:?} response={:?} reasoning={:?}",
            idx + 1,
            total,
            latency_ms,
            correct,
            resp.prompt_tokens,
            resp.completion_tokens,
            response_preview,
            reasoning_preview
        );
        writer.write_case(CaseMetric {
            case_id: case.case_id.clone(),
            correct,
            latency_ms,
            prompt_tokens: resp.prompt_tokens,
            completion_tokens: resp.completion_tokens,
        })?;
    }
    Ok(writer.finish())
}

#[cfg(not(feature = "network"))]
async fn run_with_dataset(_opts: &SuiteRunOptions, _path: &Path) -> anyhow::Result<SectionReport> {
    anyhow::bail!(
        "Suite::Cr requires the `network` feature for the OpenAI-compatible \
         LLM provider. Rebuild `tsumugi-bench` with `--features network`."
    )
}

fn load_entries(path: &Path) -> anyhow::Result<Vec<Entry>> {
    use std::io::{BufRead, BufReader};
    let f = std::fs::File::open(path).map_err(|e| {
        anyhow::anyhow!(
            "failed to open MemoryAgentBench CR dataset at {:?}: {}. \
             benches/scripts/download_datasets.sh で取得し、 \
             MAB_CR_PATH env でパス指定してください。",
            path,
            e
        )
    })?;
    let mut entries = Vec::new();
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Entry = serde_json::from_str(&line)
            .map_err(|e| anyhow::anyhow!("CR JSONL line {}: {}", i + 1, e))?;
        entries.push(entry);
    }
    Ok(entries)
}

fn build_cases(entries: &[Entry], per_row: usize) -> Vec<CrCase> {
    let mut cases = Vec::new();
    for (row_idx, entry) in entries.iter().enumerate() {
        let take = per_row.min(entry.questions.len()).min(entry.answers.len());
        for q_idx in 0..take {
            let answers = entry.answers[q_idx].clone();
            if answers.is_empty() {
                // 答えが無い質問はスキップ (judge できないため)
                continue;
            }
            cases.push(CrCase {
                case_id: format!("cr-row{}-q{}", row_idx, q_idx),
                context: entry.context.clone(),
                question: entry.questions[q_idx].clone(),
                answers,
            });
        }
    }
    cases
}

/// 文区切り (`. ! ? \n`) 優先で context を `target_chars` 程度の chunk に分割する。
/// target を超えた次の sentence boundary で chunk 終了。最終 chunk は target を
/// 超えないよう貪欲に詰める。Unicode multi-byte char は char_indices で安全に扱う。
fn chunk_text(context: &str, target_chars: usize) -> Vec<String> {
    if context.is_empty() {
        return Vec::new();
    }
    // env 経由の override は >= 256 を保証している。低いターゲットは
    // テストでのみ使用するのでフロアを設けない。
    let target = target_chars.max(1);
    let mut chunks = Vec::new();
    let mut current = String::with_capacity(target + 256);
    let bytes = context.as_bytes();
    let mut last_boundary = 0usize;

    for (i, _) in context.char_indices() {
        let b = bytes[i];
        // current に逐次追加
        // (char 単位で push するため slice をまとめて取る)
        if i > last_boundary && current.len() >= target {
            // sentence boundary を超えた最初の位置で chunk を確定
            chunks.push(std::mem::take(&mut current));
        }
        // sentence boundary 検出: '.', '!', '?', '\n'
        if matches!(b, b'.' | b'!' | b'?' | b'\n') {
            last_boundary = i + 1;
        }
        // current に該当 char を追加
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
fn tail_chars(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let target_start = s.len() - n;
    // s.len() - n は byte index、その位置が char boundary でなければ後ろにずらす。
    let mut start = target_start;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    s[start..].to_string()
}

#[cfg(feature = "network")]
async fn bm25_retrieve(
    chunks: &[String],
    query: &str,
    top_k: usize,
) -> anyhow::Result<Vec<String>> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }
    // ChunkId は UUID v4 で重複しないので、index 復元用に lookup map を持つ。
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

fn build_prompt(question: &str, context_block: &str) -> String {
    format!(
        "You are reading excerpts from a long document. Some statements in \
         the document may CONFLICT with each other; in that case, the LATER \
         (more recent) statement supersedes the earlier one. Use the \
         most up-to-date information to answer.\n\n\
         === DOCUMENT EXCERPTS ===\n{}\n=== END EXCERPTS ===\n\n\
         Question: {}\n\
         Answer concisely with only the answer (no explanation).\n\
         Final answer:",
        context_block, question
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_jsonl(path: &Path, entries: &[serde_json::Value]) {
        let mut f = std::fs::File::create(path).unwrap();
        for e in entries {
            f.write_all(e.to_string().as_bytes()).unwrap();
            f.write_all(b"\n").unwrap();
        }
    }

    fn fixture_entry(ctx: &str, questions: &[&str], answers: &[&[&str]]) -> serde_json::Value {
        let qs: Vec<_> = questions.iter().map(|s| s.to_string()).collect();
        let ans: Vec<Vec<String>> = answers
            .iter()
            .map(|cands| cands.iter().map(|s| s.to_string()).collect())
            .collect();
        serde_json::json!({
            "context": ctx,
            "questions": qs,
            "answers": ans,
            "metadata": {}
        })
    }

    #[test]
    fn load_entries_parses_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("cr.jsonl");
        write_jsonl(
            &path,
            &[
                fixture_entry("ctx1", &["q1", "q2"], &[&["a1"], &["a2"]]),
                fixture_entry("ctx2", &["q3"], &[&["a3a", "a3b"]]),
            ],
        );
        let entries = load_entries(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].context, "ctx1");
        assert_eq!(entries[0].questions.len(), 2);
        assert_eq!(
            entries[1].answers[0],
            vec!["a3a".to_string(), "a3b".to_string()]
        );
    }

    #[test]
    fn load_entries_skips_blank_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("cr.jsonl");
        let entry = fixture_entry("ctx", &["q"], &[&["a"]]);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"\n").unwrap();
        f.write_all(entry.to_string().as_bytes()).unwrap();
        f.write_all(b"\n\n").unwrap();
        f.write_all(entry.to_string().as_bytes()).unwrap();
        f.write_all(b"\n").unwrap();
        let entries = load_entries(&path).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn build_cases_default_per_row_is_1() {
        let entries: Vec<Entry> = (0..8)
            .map(|i| Entry {
                context: format!("doc {}", i),
                questions: vec![format!("q{}-0", i), format!("q{}-1", i)],
                answers: vec![vec![format!("a{}-0", i)], vec![format!("a{}-1", i)]],
                metadata: serde_json::Value::Null,
            })
            .collect();
        let cases = build_cases(&entries, 1);
        assert_eq!(cases.len(), 8);
        assert_eq!(cases[0].case_id, "cr-row0-q0");
        assert_eq!(cases[7].case_id, "cr-row7-q0");
        // questions[0] のみ
        for case in &cases {
            assert!(case.case_id.ends_with("-q0"));
        }
    }

    #[test]
    fn build_cases_respects_per_row_env() {
        let entries: Vec<Entry> = (0..3)
            .map(|i| Entry {
                context: format!("doc {}", i),
                questions: (0..5).map(|j| format!("q{}-{}", i, j)).collect(),
                answers: (0..5).map(|j| vec![format!("a{}-{}", i, j)]).collect(),
                metadata: serde_json::Value::Null,
            })
            .collect();
        let cases = build_cases(&entries, 3);
        assert_eq!(cases.len(), 9, "3 rows × 3 per_row = 9");
        assert_eq!(cases[3].case_id, "cr-row1-q0");
    }

    #[test]
    fn build_cases_skips_questions_with_empty_answers() {
        let entries = vec![Entry {
            context: "ctx".into(),
            questions: vec!["q0".into(), "q1".into()],
            answers: vec![vec![], vec!["a1".into()]], // q0 は答え無し
            metadata: serde_json::Value::Null,
        }];
        let cases = build_cases(&entries, 2);
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].case_id, "cr-row0-q1");
    }

    #[test]
    fn chunk_text_splits_at_target_chars_with_sentence_boundary() {
        let text = "First sentence. Second sentence here. Third one. ".repeat(50);
        let chunks = chunk_text(&text, 100);
        // 全 chunk が 200 chars 以下に収まること (target 100 + sentence overflow)
        for c in &chunks {
            assert!(c.len() < 250, "chunk too large: {} chars", c.len());
        }
        // 全 chunk を結合すると元 text に戻る
        let rejoined: String = chunks.join("");
        assert_eq!(rejoined, text);
    }

    #[test]
    fn chunk_text_handles_unicode_safely() {
        // multi-byte char が boundary で切られても panic しない
        let text = "日本語テキストです。これは別の文。さらに次の文。".repeat(20);
        let chunks = chunk_text(&text, 50);
        assert!(!chunks.is_empty());
        // 結合復元
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
        // 切り出された部分が UTF-8 として valid (panic しないこと自体が保証)
        assert!(tail.len() <= text.len());
        assert!(text.ends_with(&tail));
    }

    #[test]
    fn tail_chars_returns_full_string_when_shorter_than_n() {
        assert_eq!(tail_chars("short", 100), "short");
    }

    #[test]
    fn build_prompt_includes_supersession_directive_and_question() {
        let p = build_prompt("Who is X?", "doc body here");
        // CR タスクに必須の supersession 指示が prompt に含まれること
        assert!(
            p.contains("CONFLICT"),
            "prompt missing CONFLICT directive: {p}"
        );
        assert!(
            p.contains("LATER") && p.contains("supersedes"),
            "prompt missing supersession directive: {p}"
        );
        assert!(p.contains("most up-to-date"));
        assert!(p.contains("Who is X?"));
        assert!(p.contains("doc body here"));
        assert!(p.contains("Final answer:"));
    }

    #[test]
    fn questions_per_row_from_env_falls_back_to_default() {
        std::env::remove_var("CR_QUESTIONS_PER_ROW");
        assert_eq!(questions_per_row_from_env(), 1);
    }
}

#[cfg(all(test, feature = "network"))]
mod network_tests {
    use super::*;
    use crate::suite::Suite;
    use std::io::Write;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn write_fixture_dataset(dir: &Path, rows: usize) -> PathBuf {
        let path = dir.join("cr.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 0..rows {
            let entry = serde_json::json!({
                "context": format!("Document {} body. Some statement A. Some statement B.", i),
                "questions": [format!("What about row {}?", i)],
                "answers": [[format!("ANSWER-row{}", i)]],
                "metadata": {}
            });
            f.write_all(entry.to_string().as_bytes()).unwrap();
            f.write_all(b"\n").unwrap();
        }
        path
    }

    fn opts_for(server_uri: String, output_dir: PathBuf) -> SuiteRunOptions {
        SuiteRunOptions {
            suite: Suite::Cr,
            output_dir,
            llm_base_url: server_uri,
            llm_model: "qwen3.5-4b".into(),
            help: false,
        }
    }

    #[tokio::test]
    async fn cr_runs_8_cases_and_marks_correctness() {
        let server = MockServer::start().await;
        // 全 case の正解 "ANSWER-row{i}" を含む応答を返す mock。
        // ただし 1 つだけ含めるので、その row の case は correct=true、他は false。
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "Final answer: ANSWER-row3 (the most up-to-date)"
                    }
                }],
                "usage": { "prompt_tokens": 1000, "completion_tokens": 12 }
            })))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let dataset = write_fixture_dataset(tmp.path(), 8);
        let opts = opts_for(server.uri(), tmp.path().to_path_buf());
        let report = run_with_dataset(&opts, &dataset).await.expect("run");

        assert_eq!(report.bench, "memoryagentbench-cr");
        assert_eq!(report.ablation, "full");
        assert_eq!(report.cases.len(), 8);
        // row3 のみ correct=true
        let correct_ids: Vec<&str> = report
            .cases
            .iter()
            .filter(|c| c.correct)
            .map(|c| c.case_id.as_str())
            .collect();
        assert_eq!(correct_ids, vec!["cr-row3-q0"]);
    }

    #[tokio::test]
    async fn cr_propagates_provider_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let dataset = write_fixture_dataset(tmp.path(), 2);
        let opts = opts_for(server.uri(), tmp.path().to_path_buf());
        let err = run_with_dataset(&opts, &dataset).await.unwrap_err();
        assert!(err.to_string().contains("500"), "got: {err}");
    }

    #[tokio::test]
    async fn cr_emits_jsonl_incrementally_for_timeout_safety() {
        // case 1 完了後 (case 2 で server が止まる前) でも jsonl が
        // disk に残ること。IncrementalSectionWriter の per-case fsync 効果を
        // CR adapter 経由でも確認する。
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{ "message": { "role": "assistant", "content": "ok" } }],
                "usage": { "prompt_tokens": 100, "completion_tokens": 1 }
            })))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let dataset = write_fixture_dataset(tmp.path(), 3);
        let opts = opts_for(server.uri(), tmp.path().to_path_buf());
        run_with_dataset(&opts, &dataset).await.unwrap();
        let jsonl_path = tmp.path().join("memoryagentbench-cr/full.jsonl");
        let content = std::fs::read_to_string(&jsonl_path).unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 3, "expected 3 jsonl lines, got: {content}");
        for line in &lines {
            // 各行が valid JSON で case_id を含む
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(v.get("case_id").is_some());
        }
    }
}
