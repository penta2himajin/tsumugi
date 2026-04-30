//! RULER NIAH-S (Single needle in a haystack) adapter.
//!
//! 計画書 §「ベンチマークサブセットの選定」: `niah_single_2` を seq_len ∈
//! {4K, 8K, 16K, 32K, 64K} で各 1 ケース、計 5 ケース。Tier 0 (BM25)
//! baseline の確認用。LLM 削除後は retrieval recall (substring match on
//! retrieved chunks) のみで判定する。
//!
//! 本実装は **CPU smoke 環境向け**の合成生成版:
//! - 公式 RULER は Paul Graham essays を haystack に使う (paper-exact)
//!   が、本 adapter は deterministic な lorem-ipsum 系合成で代替する
//! - 公式 5 サイズ (4K/8K/16K/32K/64K) は CPU + 16K 程度の context 上限に
//!   収まらないため、デフォルトを {2K, 4K, 8K, 12K} の 4 ケースに絞る。
//!   `RULER_SEQ_LENGTHS=4096,8192,16384` 等で env 上書き可能
//!
//! 評価方法: needle に埋め込んだ value が retrieved (or compressed)
//! chunk に substring として残っているかを判定する。

use crate::adapters::common::{
    bm25_retrieve, chunk_text, concat_for_judge, hybrid_retrieve, tier_0_1_2_compress,
};
use crate::metrics::{substring_match, CaseMetric};
use crate::report::{IncrementalSectionWriter, SectionReport};
use crate::suite::{Ablation, SuiteRunOptions};

/// 1 chunk あたりのターゲット文字数。RULER haystack を BM25 / Hybrid に
/// 流すときの粒度。
const RULER_CHUNK_CHARS: usize = 2048;
/// retrieval top_k。
const RULER_TOP_K: usize = 10;
/// tier-0-1-2 compress budget (whitespace tokens)。
/// `RULER_COMPRESS_BUDGET_TOKENS` で override 可。
const RULER_COMPRESS_BUDGET_TOKENS: u32 = 2048;
const RULER_COMPRESS_PRESERVE_TAIL: u32 = 256;

fn compress_budget_from_env() -> u32 {
    std::env::var("RULER_COMPRESS_BUDGET_TOKENS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(RULER_COMPRESS_BUDGET_TOKENS)
}

/// CPU + 16K ctx 向けの保守的サイズ。
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

pub async fn run_niah_s_with_ablation(
    opts: &SuiteRunOptions,
    ablation: Ablation,
) -> anyhow::Result<SectionReport> {
    let seq_lengths = seq_lengths_from_env();
    let cases = build_cases(DEFAULT_SEED, &seq_lengths);
    eprintln!(
        "[smoke/{}] {} RULER NIAH-S cases (seq_lengths={:?})",
        ablation.name(),
        cases.len(),
        seq_lengths
    );
    run_niah_s_retrieval_only(opts, &cases, ablation).await
}

/// 全 ablation 共通 path。haystack を `chunk_text(target=2048)` で分割し、
/// `needle_key` を query に retrieval する。
async fn run_niah_s_retrieval_only(
    opts: &SuiteRunOptions,
    cases: &[NiahCase],
    ablation: Ablation,
) -> anyhow::Result<SectionReport> {
    let mut writer = IncrementalSectionWriter::create(&opts.output_dir, "ruler-niah-s", ablation)?;
    let budget = compress_budget_from_env();
    let total = cases.len();
    for (idx, case) in cases.iter().enumerate() {
        let chunks = chunk_text(&case.haystack, RULER_CHUNK_CHARS);
        let started = std::time::Instant::now();
        let retrieved = match ablation {
            Ablation::Tier0 => bm25_retrieve(&chunks, &case.needle_key, RULER_TOP_K).await?,
            Ablation::Tier01 | Ablation::Tier012 => {
                hybrid_retrieve(&chunks, &case.needle_key, RULER_TOP_K).await?
            }
        };
        let concat = concat_for_judge(&retrieved);
        let retrieval_chars = concat.chars().count();
        let (judge_text, compressed_chars) = if matches!(ablation, Ablation::Tier012) {
            let compressed =
                tier_0_1_2_compress(&concat, budget, RULER_COMPRESS_PRESERVE_TAIL).await?;
            let len = compressed.chars().count();
            (compressed, Some(len))
        } else {
            (concat, None)
        };
        let retrieval_latency_ms = started.elapsed().as_millis() as u64;
        let correct = substring_match(&judge_text, &case.needle_value);
        eprintln!(
            "[smoke/{}] [{}/{}] case={} seq_len={} chunks={} hits={} retrieval_chars={} compressed_chars={:?} correct={} latency={}ms",
            ablation.name(),
            idx + 1,
            total,
            case.case_id,
            case.seq_len,
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
        // 余裕を見て、target ± 200 chars 範囲で生成されること。
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
    fn seq_lengths_from_env_falls_back_to_default() {
        std::env::remove_var("RULER_SEQ_LENGTHS");
        assert_eq!(seq_lengths_from_env(), DEFAULT_SEQ_LENGTHS.to_vec());
    }

    fn opts_for(output_dir: std::path::PathBuf) -> SuiteRunOptions {
        SuiteRunOptions {
            suite: crate::suite::Suite::Smoke,
            output_dir,
            ablations: Ablation::default_set(),
            help: false,
        }
    }

    #[tokio::test]
    async fn niah_s_tier0_retrieves_needle_chunk() {
        // BM25 で needle_key を含む chunk が retrieve できれば correct=true。
        let tmp = tempfile::tempdir().unwrap();
        let opts = opts_for(tmp.path().to_path_buf());
        let report = run_niah_s_with_ablation(&opts, Ablation::Tier0)
            .await
            .expect("tier-0 run");
        assert_eq!(report.bench, "ruler-niah-s");
        assert_eq!(report.ablation, "tier-0");
        assert!(!report.cases.is_empty());
        // 各 case で BM25 retrieval が needle_key を含む chunk を上位に
        // 出すことを期待 (合成 haystack は filler が一様なので BM25 score
        // は needle 文を含む chunk が支配的になる)
        let correct_count = report.cases.iter().filter(|c| c.correct).count();
        assert!(
            correct_count >= 1,
            "expected at least 1 correct retrieval, got {correct_count} of {}: {:?}",
            report.cases.len(),
            report.cases
        );
        for c in &report.cases {
            assert!(c.retrieval_latency_ms.is_some());
            assert!(c.retrieved_chunks.is_some());
            assert!(c.compressed_chars.is_none());
        }
    }

    #[tokio::test]
    async fn niah_s_tier012_records_compressed_chars() {
        let tmp = tempfile::tempdir().unwrap();
        let opts = opts_for(tmp.path().to_path_buf());
        // RULER_COMPRESS_BUDGET_TOKENS は他の test が読まないので race なし。
        // RULER_SEQ_LENGTHS は他の test も読むので mutate しない (default 4 cases)。
        // default budget 2048 tok で under-budget の case は no-op (compressed
        // == retrieval) になる可能性があるが、`compressed_chars.is_some()` と
        // `after ≤ before` は常に成立。
        let report = run_niah_s_with_ablation(&opts, Ablation::Tier012)
            .await
            .expect("tier-0-1-2 run");
        assert_eq!(report.ablation, "tier-0-1-2");
        for c in &report.cases {
            assert!(c.compressed_chars.is_some());
            let before = c.retrieval_chars.unwrap();
            let after = c.compressed_chars.unwrap();
            assert!(after <= before);
        }
    }
}
