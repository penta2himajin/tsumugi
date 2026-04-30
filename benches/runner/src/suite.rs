//! Benchmark suite dispatch.

use crate::adapters;
use crate::report::SuiteReport;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suite {
    /// RULER NIAH-S 5 ケースのみ。CI smoke (Step 3 で実装) に使う最短経路。
    Smoke,
    /// LongMemEval_oracle 30 問。
    Oracle,
    /// MemoryAgentBench Conflict_Resolution 8 問。
    Cr,
    /// 上記すべて (smoke + oracle + cr)、計 43 ケース。
    All,
}

impl FromStr for Suite {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        Ok(match s {
            "smoke" => Suite::Smoke,
            "oracle" => Suite::Oracle,
            "cr" => Suite::Cr,
            "all" => Suite::All,
            other => {
                anyhow::bail!("unknown suite `{other}` (expected: smoke|oracle|cr|all)")
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ablation {
    /// BM25 retrieval のみ、判定は substring match (LLM 不使用)。
    Tier0,
    /// BM25 + cosine semantic (`HybridRetriever` + `OnnxEmbedding` ある
    /// いは `MockEmbedding`)、判定は substring match。
    Tier01,
    /// tier-0-1 + `LlmLingua2Compressor` (env 未設定なら `TruncateCompressor`
    /// にフォールバック)、判定は substring match。
    Tier012,
}

impl Ablation {
    pub fn name(&self) -> &'static str {
        match self {
            Ablation::Tier0 => "tier-0",
            Ablation::Tier01 => "tier-0-1",
            Ablation::Tier012 => "tier-0-1-2",
        }
    }

    /// 既定の ablation セット (3 構成すべて)。`BENCH_ABLATIONS` env や
    /// `--ablations` CLI flag が未指定のときに使う。
    pub fn default_set() -> Vec<Ablation> {
        vec![Ablation::Tier0, Ablation::Tier01, Ablation::Tier012]
    }
}

impl FromStr for Ablation {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        Ok(match s.trim() {
            "tier-0" => Ablation::Tier0,
            "tier-0-1" => Ablation::Tier01,
            "tier-0-1-2" => Ablation::Tier012,
            other => {
                anyhow::bail!("unknown ablation `{other}` (expected: tier-0|tier-0-1|tier-0-1-2)")
            }
        })
    }
}

/// CSV (`tier-0,tier-0-1` 等) を `Vec<Ablation>` に変換。空 CSV は空 Vec を返す。
/// 呼び出し側で `is_empty()` をチェックして default_set にフォールバック
/// すること。
pub fn parse_ablation_csv(s: &str) -> anyhow::Result<Vec<Ablation>> {
    let mut out = Vec::new();
    for tok in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        out.push(tok.parse::<Ablation>()?);
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct SuiteRunOptions {
    pub suite: Suite,
    pub output_dir: PathBuf,
    /// 走らせる ablation 一覧。
    pub ablations: Vec<Ablation>,
    pub help: bool,
}

impl SuiteRunOptions {
    pub fn usage() -> &'static str {
        "Usage: tsumugi-bench --suite <smoke|oracle|cr|all> --output <dir> \
         [--ablations <csv>] [--help]\n\n\
         --ablations: CSV of `tier-0|tier-0-1|tier-0-1-2` \
         (env BENCH_ABLATIONS でも指定可、未指定時は 3 構成すべて)。\n\n\
         tsumugi が encoder-only に確定して以降、`full` ablation (LLM \
         answer 生成) は廃止された。判定は全 ablation で substring \
         match (retrieval recall on retrieved chunks)。"
    }

    /// `--ablations` flag (CSV) > `BENCH_ABLATIONS` env (CSV) > default_set
    /// の優先順位で解決する。flag/env が空文字列だった場合も default_set に
    /// fallback する (空指定で 0 ablation を走らせる選択肢は無効)。
    fn resolve_ablations(flag: Option<&str>) -> anyhow::Result<Vec<Ablation>> {
        if let Some(s) = flag {
            let v = parse_ablation_csv(s)?;
            if !v.is_empty() {
                return Ok(v);
            }
        }
        if let Ok(env_s) = std::env::var("BENCH_ABLATIONS") {
            let v = parse_ablation_csv(&env_s)?;
            if !v.is_empty() {
                return Ok(v);
            }
        }
        Ok(Ablation::default_set())
    }

    pub fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut suite: Option<Suite> = None;
        let mut output_dir: Option<PathBuf> = None;
        let mut ablations_flag: Option<String> = None;
        let mut help = false;
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--suite" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--suite requires a value"))?;
                    suite = Some(v.parse()?);
                }
                "--output" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--output requires a value"))?;
                    output_dir = Some(PathBuf::from(v));
                }
                "--ablations" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--ablations requires a CSV value"))?;
                    ablations_flag = Some(v.clone());
                }
                "--help" | "-h" => {
                    help = true;
                }
                other => anyhow::bail!("unknown argument: {other}"),
            }
        }
        let ablations = Self::resolve_ablations(ablations_flag.as_deref())?;
        if help {
            return Ok(Self {
                suite: Suite::Smoke,
                output_dir: PathBuf::new(),
                ablations,
                help: true,
            });
        }
        Ok(Self {
            suite: suite.ok_or_else(|| anyhow::anyhow!("--suite is required"))?,
            output_dir: output_dir.ok_or_else(|| anyhow::anyhow!("--output is required"))?,
            ablations,
            help: false,
        })
    }
}

impl Suite {
    pub async fn run(&self, opts: &SuiteRunOptions) -> anyhow::Result<SuiteReport> {
        let mut report = SuiteReport::new(*self);
        match self {
            Suite::Smoke => {
                for ab in &opts.ablations {
                    report.add_section(adapters::ruler::run_niah_s_with_ablation(opts, *ab).await?);
                }
            }
            Suite::Oracle => {
                let dataset_path = adapters::longmemeval::default_dataset_path();
                for ab in &opts.ablations {
                    report.add_section(
                        adapters::longmemeval::run_oracle_with_ablation(opts, *ab, &dataset_path)
                            .await?,
                    );
                }
            }
            Suite::Cr => {
                let dataset_path = adapters::memoryagentbench::default_dataset_path();
                for ab in &opts.ablations {
                    report.add_section(
                        adapters::memoryagentbench::run_with_dataset_with_ablation(
                            opts,
                            *ab,
                            &dataset_path,
                        )
                        .await?,
                    );
                }
            }
            Suite::All => {
                let lme_path = adapters::longmemeval::default_dataset_path();
                let cr_path = adapters::memoryagentbench::default_dataset_path();
                for ab in &opts.ablations {
                    report.add_section(adapters::ruler::run_niah_s_with_ablation(opts, *ab).await?);
                    report.add_section(
                        adapters::longmemeval::run_oracle_with_ablation(opts, *ab, &lme_path)
                            .await?,
                    );
                    report.add_section(
                        adapters::memoryagentbench::run_with_dataset_with_ablation(
                            opts, *ab, &cr_path,
                        )
                        .await?,
                    );
                }
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ablation_names_are_stable() {
        assert_eq!(Ablation::Tier0.name(), "tier-0");
        assert_eq!(Ablation::Tier01.name(), "tier-0-1");
        assert_eq!(Ablation::Tier012.name(), "tier-0-1-2");
    }

    #[test]
    fn suite_parses_known_values() {
        assert_eq!("smoke".parse::<Suite>().unwrap(), Suite::Smoke);
        assert_eq!("oracle".parse::<Suite>().unwrap(), Suite::Oracle);
        assert_eq!("cr".parse::<Suite>().unwrap(), Suite::Cr);
        assert_eq!("all".parse::<Suite>().unwrap(), Suite::All);
    }

    #[test]
    fn suite_rejects_health_value() {
        // `Suite::Health` was removed when LLM was. Make sure parser doesn't
        // silently accept it.
        let err = "health".parse::<Suite>().unwrap_err();
        assert!(err.to_string().contains("unknown suite"));
    }

    #[test]
    fn ablation_parses_known_values() {
        assert_eq!("tier-0".parse::<Ablation>().unwrap(), Ablation::Tier0);
        assert_eq!("tier-0-1".parse::<Ablation>().unwrap(), Ablation::Tier01);
        assert_eq!("tier-0-1-2".parse::<Ablation>().unwrap(), Ablation::Tier012);
        // 余分な whitespace は trim される
        assert_eq!(
            "  tier-0-1  ".parse::<Ablation>().unwrap(),
            Ablation::Tier01
        );
    }

    #[test]
    fn ablation_rejects_unknown_value() {
        let err = "tier-9".parse::<Ablation>().unwrap_err();
        assert!(
            err.to_string().contains("tier-9"),
            "error should mention input: {err}"
        );
    }

    #[test]
    fn ablation_rejects_full_value() {
        // `Ablation::Full` was removed with LLM.
        let err = "full".parse::<Ablation>().unwrap_err();
        assert!(err.to_string().contains("full"));
    }

    #[test]
    fn ablation_default_set_is_three_in_canonical_order() {
        assert_eq!(
            Ablation::default_set(),
            vec![Ablation::Tier0, Ablation::Tier01, Ablation::Tier012]
        );
    }

    #[test]
    fn parse_ablation_csv_accepts_subset() {
        let v = parse_ablation_csv("tier-0,tier-0-1-2").unwrap();
        assert_eq!(v, vec![Ablation::Tier0, Ablation::Tier012]);
    }

    #[test]
    fn parse_ablation_csv_skips_blank_tokens_and_trims() {
        let v = parse_ablation_csv(" tier-0 , , tier-0-1-2 ").unwrap();
        assert_eq!(v, vec![Ablation::Tier0, Ablation::Tier012]);
    }

    #[test]
    fn parse_ablation_csv_returns_empty_for_empty_input() {
        // 空 CSV / 空白のみは「明示的に 0 件」として返す。fallback 判断は
        // 呼び出し側 (resolve_ablations) で行う。
        assert!(parse_ablation_csv("").unwrap().is_empty());
        assert!(parse_ablation_csv("  ,, ,  ").unwrap().is_empty());
    }

    #[test]
    fn suite_run_options_default_ablations_is_three() {
        // env を unset した状態で --ablations 未指定なら default_set に解決
        std::env::remove_var("BENCH_ABLATIONS");
        let args = [
            "--suite".into(),
            "smoke".into(),
            "--output".into(),
            "/tmp/x".into(),
        ];
        let opts = SuiteRunOptions::parse(&args).unwrap();
        assert_eq!(opts.ablations, Ablation::default_set());
    }

    #[test]
    fn suite_run_options_ablations_flag_overrides_default() {
        std::env::remove_var("BENCH_ABLATIONS");
        let args = [
            "--suite".into(),
            "smoke".into(),
            "--output".into(),
            "/tmp/x".into(),
            "--ablations".into(),
            "tier-0,tier-0-1-2".into(),
        ];
        let opts = SuiteRunOptions::parse(&args).unwrap();
        assert_eq!(opts.ablations, vec![Ablation::Tier0, Ablation::Tier012]);
    }
}
