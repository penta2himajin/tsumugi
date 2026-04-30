//! MemoryAgentBench `Conflict_Resolution` adapter.
//!
//! Phase 4-α Step 3 PR ②: HF dataset `ai-hyz/MemoryAgentBench` の
//! `Conflict_Resolution` split は **8 行 × 60-100 QA / 行**で、各行の
//! `context` は 273k-3.17M chars (約 70K-800K tokens) と一般的な
//! BERT/encoder の context window を大きく超える。
//!
//! 計画書の「全 8 問」は **8 行 × `questions[0]` = 8 評価ケース** と解釈する。
//!
//! Context truncation 戦略: `tsumugi_core::retriever::Bm25Retriever` で
//! chunk_size 1024 tok (≒ 4096 chars) / top_k 10 の retrieval を行い、
//! ~10K tok 程度に圧縮する。BM25 hit が `top_k/2` 未満になった場合の
//! fallback (末尾切り出し) は LLM 削除前の `full` ablation 用に書かれた
//! ロジックなので、retrieval-only path には不要 (retrieved を空配列の
//! まま judge して miss 扱いに)。
//!
//! 正解判定: `answers[i]` は `List[String]` の同義語候補。
//! `substring_match_any` でいずれか 1 つに部分一致すれば correct。
//!
//! データ位置: 既定 `benches/data/memoryagentbench_cr.jsonl`。
//! 環境変数 `MAB_CR_PATH` で override 可。`download_datasets.sh` が
//! parquet → JSONL 変換 (pyarrow 経由) を担当する。

use crate::adapters::common::{
    bm25_retrieve, chunk_text, concat_for_judge, hybrid_retrieve, tier_0_1_2_compress,
};
use crate::metrics::{substring_match_any, CaseMetric};
use crate::report::{IncrementalSectionWriter, SectionReport};
use crate::suite::{Ablation, SuiteRunOptions};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// 1 chunk あたりのターゲット文字数。English ASCII で 4 chars/token と仮定し
/// 1024 token を保守的に約 4096 chars にする。`CR_CHUNK_CHARS` env で override 可。
const DEFAULT_CHUNK_CHARS: usize = 4096;

/// BM25 retrieval の取得件数。`top_k * chunk_chars` が prompt budget の主要部分。
/// `CR_TOP_K` env で override 可。
const DEFAULT_TOP_K: usize = 10;

/// 1 行あたりに評価する questions の数。default=1 で `questions[0]` のみ。
/// `CR_QUESTIONS_PER_ROW` env で 1..N に拡張可能 (各 row × per_row case を生成)。
const DEFAULT_QUESTIONS_PER_ROW: usize = 1;

/// tier-0-1-2 の compressor budget (whitespace tokens)。retrieval の concat に
/// 対して compress を掛けた結果に対し substring match を取る。
/// `CR_COMPRESS_BUDGET_TOKENS` env で override 可。
const DEFAULT_COMPRESS_BUDGET_TOKENS: u32 = 2048;
/// compressor の tail 保持 token 数 (head + " … " + tail のうち
/// tail 側に残す最小 token 数)。
const DEFAULT_COMPRESS_PRESERVE_TAIL: u32 = 256;

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

fn compress_budget_from_env() -> u32 {
    std::env::var("CR_COMPRESS_BUDGET_TOKENS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_COMPRESS_BUDGET_TOKENS)
}

pub fn default_dataset_path() -> PathBuf {
    std::env::var("MAB_CR_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("benches/data/memoryagentbench_cr.jsonl"))
}

pub async fn run_with_dataset_with_ablation(
    opts: &SuiteRunOptions,
    ablation: Ablation,
    path: &Path,
) -> anyhow::Result<SectionReport> {
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
        "[cr/{}] {} cases ({} rows × {} questions/row), chunk_chars={}, top_k={}",
        ablation.name(),
        cases.len(),
        entries.len(),
        per_row,
        chunk_chars,
        top_k
    );

    run_cr_retrieval_only(opts, &cases, ablation, chunk_chars, top_k).await
}

/// 全 ablation 共通 path。retrieval (BM25 / Hybrid) のみで判定し、
/// tier-0-1-2 では retrieval 結果に compressor を適用する。
async fn run_cr_retrieval_only(
    opts: &SuiteRunOptions,
    cases: &[CrCase],
    ablation: Ablation,
    chunk_chars: usize,
    top_k: usize,
) -> anyhow::Result<SectionReport> {
    let mut writer =
        IncrementalSectionWriter::create(&opts.output_dir, "memoryagentbench-cr", ablation)?;
    let budget = compress_budget_from_env();
    let total = cases.len();
    for (idx, case) in cases.iter().enumerate() {
        let chunks = chunk_text(&case.context, chunk_chars);
        let started = std::time::Instant::now();
        let retrieved = match ablation {
            Ablation::Tier0 => bm25_retrieve(&chunks, &case.question, top_k).await?,
            Ablation::Tier01 | Ablation::Tier012 => {
                hybrid_retrieve(&chunks, &case.question, top_k).await?
            }
        };
        let concat = concat_for_judge(&retrieved);
        let retrieval_chars = concat.chars().count();
        let (judge_text, compressed_chars) = if matches!(ablation, Ablation::Tier012) {
            let compressed =
                tier_0_1_2_compress(&concat, budget, DEFAULT_COMPRESS_PRESERVE_TAIL).await?;
            let compressed_len = compressed.chars().count();
            (compressed, Some(compressed_len))
        } else {
            (concat, None)
        };
        let retrieval_latency_ms = started.elapsed().as_millis() as u64;
        let correct = substring_match_any(&judge_text, &case.answers);
        eprintln!(
            "[cr/{}] [{}/{}] case={} chunks={} hits={} retrieval_chars={} compressed_chars={:?} correct={} latency={}ms",
            ablation.name(),
            idx + 1,
            total,
            case.case_id,
            chunks.len(),
            retrieved.len(),
            retrieval_chars,
            compressed_chars,
            correct,
            retrieval_latency_ms
        );
        writer.write_case(CaseMetric {
            case_id: case.case_id.clone(),
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

    fn opts_for(output_dir: PathBuf) -> SuiteRunOptions {
        SuiteRunOptions {
            suite: crate::suite::Suite::Cr,
            output_dir,
            ablations: Ablation::default_set(),
            help: false,
        }
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
    fn questions_per_row_from_env_falls_back_to_default() {
        std::env::remove_var("CR_QUESTIONS_PER_ROW");
        assert_eq!(questions_per_row_from_env(), 1);
    }

    fn write_fixture_dataset(dir: &Path, rows: usize) -> PathBuf {
        let path = dir.join("cr.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for i in 0..rows {
            // context に answer を埋め込み、retrieval で hit するようにする
            let entry = serde_json::json!({
                "context": format!(
                    "Document {} body. Some statement A. Some statement B. ANSWER-row{} appears here.",
                    i, i
                ),
                "questions": [format!("What is the special token for row {}?", i)],
                "answers": [[format!("ANSWER-row{}", i)]],
                "metadata": {}
            });
            f.write_all(entry.to_string().as_bytes()).unwrap();
            f.write_all(b"\n").unwrap();
        }
        path
    }

    #[tokio::test]
    async fn cr_tier0_judges_via_retrieval() {
        let tmp = tempfile::tempdir().unwrap();
        let dataset = write_fixture_dataset(tmp.path(), 3);
        let opts = opts_for(tmp.path().to_path_buf());
        let report = run_with_dataset_with_ablation(&opts, Ablation::Tier0, &dataset)
            .await
            .expect("tier-0 run");
        assert_eq!(report.bench, "memoryagentbench-cr");
        assert_eq!(report.ablation, "tier-0");
        assert_eq!(report.cases.len(), 3);
        // BM25 retrieval は ANSWER-row{i} を含む chunk を必ず top_k 内に出すはず
        // (各 row の context は短く、chunk_text 後も answer を含む chunk が
        // 1 つだけ存在する)。よって全 case correct=true を期待。
        let correct_count = report.cases.iter().filter(|c| c.correct).count();
        assert_eq!(
            correct_count, 3,
            "all retrieval-only cases should match: {:?}",
            report.cases
        );
        for c in &report.cases {
            assert!(c.retrieval_latency_ms.is_some());
            assert!(c.retrieved_chunks.is_some());
            assert!(c.retrieval_chars.is_some());
            assert!(c.compressed_chars.is_none(), "tier-0 should not compress");
            assert!(c.prompt_tokens.is_none());
            assert!(c.completion_tokens.is_none());
        }
    }

    #[tokio::test]
    async fn cr_tier01_uses_hybrid_retrieval() {
        let tmp = tempfile::tempdir().unwrap();
        let dataset = write_fixture_dataset(tmp.path(), 2);
        let opts = opts_for(tmp.path().to_path_buf());
        let report = run_with_dataset_with_ablation(&opts, Ablation::Tier01, &dataset)
            .await
            .expect("tier-0-1 run");
        assert_eq!(report.ablation, "tier-0-1");
        assert_eq!(report.cases.len(), 2);
        for c in &report.cases {
            assert!(c.compressed_chars.is_none(), "tier-0-1 should not compress");
            assert!(c.retrieved_chunks.is_some());
        }
    }

    #[tokio::test]
    async fn cr_tier012_applies_compressor() {
        let tmp = tempfile::tempdir().unwrap();
        // 長めの fixture を作って圧縮効果を観測
        let dataset = {
            let path = tmp.path().join("cr.jsonl");
            let mut f = std::fs::File::create(&path).unwrap();
            for i in 0..2 {
                let body = "lorem ipsum dolor sit amet consectetur adipiscing elit. ".repeat(200);
                let entry = serde_json::json!({
                    "context": format!("{} ANSWER-row{} ", body, i).repeat(3),
                    "questions": [format!("What is the special token for row {}?", i)],
                    "answers": [[format!("ANSWER-row{}", i)]],
                    "metadata": {}
                });
                f.write_all(entry.to_string().as_bytes()).unwrap();
                f.write_all(b"\n").unwrap();
            }
            path
        };
        let opts = opts_for(tmp.path().to_path_buf());
        // 小さい budget で圧縮効果を強制
        std::env::set_var("CR_COMPRESS_BUDGET_TOKENS", "32");
        let report = run_with_dataset_with_ablation(&opts, Ablation::Tier012, &dataset)
            .await
            .expect("tier-0-1-2 run");
        std::env::remove_var("CR_COMPRESS_BUDGET_TOKENS");
        assert_eq!(report.ablation, "tier-0-1-2");
        for c in &report.cases {
            // tier-0-1-2 では compressed_chars が必ず Some
            assert!(
                c.compressed_chars.is_some(),
                "tier-0-1-2 must record compressed_chars: {c:?}"
            );
            // 圧縮後 chars ≤ 圧縮前 chars
            let before = c.retrieval_chars.unwrap();
            let after = c.compressed_chars.unwrap();
            assert!(after <= before, "compressed {after} > original {before}");
        }
    }

    #[tokio::test]
    async fn cr_emits_jsonl_incrementally_for_timeout_safety() {
        // case 完了ごとに jsonl が disk に残ること。IncrementalSectionWriter
        // の per-case fsync 効果を CR adapter 経由でも確認する。
        let tmp = tempfile::tempdir().unwrap();
        let dataset = write_fixture_dataset(tmp.path(), 3);
        let opts = opts_for(tmp.path().to_path_buf());
        run_with_dataset_with_ablation(&opts, Ablation::Tier0, &dataset)
            .await
            .unwrap();
        let jsonl_path = tmp.path().join("memoryagentbench-cr/tier-0.jsonl");
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
