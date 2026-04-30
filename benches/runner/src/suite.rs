//! Benchmark suite dispatch.

use crate::adapters;
use crate::report::SuiteReport;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suite {
    /// Phase 4-α Step 1 v0 smoke: LLM 起動健全性 + 生成速度 (tok/s) +
    /// 簡易指示追従。RULER / LongMemEval を呼ばないため llama-server を
    /// 立ち上げただけの環境で 1-2 分で完走する。
    Health,
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
            "health" => Suite::Health,
            "smoke" => Suite::Smoke,
            "oracle" => Suite::Oracle,
            "cr" => Suite::Cr,
            "all" => Suite::All,
            other => {
                anyhow::bail!("unknown suite `{other}` (expected: health|smoke|oracle|cr|all)")
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ablation {
    Tier0,
    Tier01,
    Tier012,
    Full,
}

impl Ablation {
    pub fn name(&self) -> &'static str {
        match self {
            Ablation::Tier0 => "tier-0",
            Ablation::Tier01 => "tier-0-1",
            Ablation::Tier012 => "tier-0-1-2",
            Ablation::Full => "full",
        }
    }

    /// 既定の ablation セット (4 構成すべて)。`BENCH_ABLATIONS` env や
    /// `--ablations` CLI flag が未指定のときに使う。
    pub fn default_set() -> Vec<Ablation> {
        vec![
            Ablation::Tier0,
            Ablation::Tier01,
            Ablation::Tier012,
            Ablation::Full,
        ]
    }
}

impl FromStr for Ablation {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        Ok(match s.trim() {
            "tier-0" => Ablation::Tier0,
            "tier-0-1" => Ablation::Tier01,
            "tier-0-1-2" => Ablation::Tier012,
            "full" => Ablation::Full,
            other => anyhow::bail!(
                "unknown ablation `{other}` (expected: tier-0|tier-0-1|tier-0-1-2|full)"
            ),
        })
    }
}

/// CSV (`tier-0,full` 等) を `Vec<Ablation>` に変換。空 CSV は空 Vec を返す。
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
    pub llm_base_url: String,
    pub llm_model: String,
    /// 走らせる ablation 一覧。`Suite::Health` は ablation 概念を持たない
    /// (Full のみ) ので無視される。
    pub ablations: Vec<Ablation>,
    pub help: bool,
}

impl SuiteRunOptions {
    pub fn usage() -> &'static str {
        "Usage: tsumugi-bench --suite <health|smoke|oracle|cr|all> --output <dir> \
         [--llm-base-url <url>] [--llm-model <name>] \
         [--ablations <csv>] [--help]\n\n\
         --ablations: CSV of `tier-0|tier-0-1|tier-0-1-2|full` \
         (env BENCH_ABLATIONS でも指定可、未指定時は 4 構成すべて)。\n\n\
         Phase 4-α Step 3 PR ③: Tier ablation matrix (`tier-0` / \
         `tier-0-1` / `tier-0-1-2` / `full`) を smoke / oracle / cr に \
         適用可能。"
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
        let mut llm_base_url = String::from("http://localhost:8080/v1");
        let mut llm_model = String::from("Qwen/Qwen3.5-4B-Instruct");
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
                "--llm-base-url" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--llm-base-url requires a value"))?;
                    llm_base_url = v.clone();
                }
                "--llm-model" => {
                    let v = iter
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("--llm-model requires a value"))?;
                    llm_model = v.clone();
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
                suite: Suite::Health,
                output_dir: PathBuf::new(),
                llm_base_url,
                llm_model,
                ablations,
                help: true,
            });
        }
        Ok(Self {
            suite: suite.ok_or_else(|| anyhow::anyhow!("--suite is required"))?,
            output_dir: output_dir.ok_or_else(|| anyhow::anyhow!("--output is required"))?,
            llm_base_url,
            llm_model,
            ablations,
            help: false,
        })
    }
}

impl Suite {
    pub async fn run(&self, opts: &SuiteRunOptions) -> anyhow::Result<SuiteReport> {
        let mut report = SuiteReport::new(*self);
        match self {
            Suite::Health => {
                // Health は ablation 概念なし (LLM 起動健全性のみ測定)。
                report.add_section(crate::health::run_health(opts).await?);
            }
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
        assert_eq!(Ablation::Full.name(), "full");
    }

    #[test]
    fn suite_parses_known_values() {
        assert_eq!("smoke".parse::<Suite>().unwrap(), Suite::Smoke);
        assert_eq!("oracle".parse::<Suite>().unwrap(), Suite::Oracle);
        assert_eq!("cr".parse::<Suite>().unwrap(), Suite::Cr);
        assert_eq!("all".parse::<Suite>().unwrap(), Suite::All);
    }

    #[test]
    fn ablation_parses_known_values() {
        assert_eq!("tier-0".parse::<Ablation>().unwrap(), Ablation::Tier0);
        assert_eq!("tier-0-1".parse::<Ablation>().unwrap(), Ablation::Tier01);
        assert_eq!("tier-0-1-2".parse::<Ablation>().unwrap(), Ablation::Tier012);
        assert_eq!("full".parse::<Ablation>().unwrap(), Ablation::Full);
        // 余分な whitespace は trim される
        assert_eq!("  full  ".parse::<Ablation>().unwrap(), Ablation::Full);
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
    fn ablation_default_set_is_all_four_in_canonical_order() {
        assert_eq!(
            Ablation::default_set(),
            vec![
                Ablation::Tier0,
                Ablation::Tier01,
                Ablation::Tier012,
                Ablation::Full
            ]
        );
    }

    #[test]
    fn parse_ablation_csv_accepts_subset() {
        let v = parse_ablation_csv("tier-0,full").unwrap();
        assert_eq!(v, vec![Ablation::Tier0, Ablation::Full]);
    }

    #[test]
    fn parse_ablation_csv_skips_blank_tokens_and_trims() {
        let v = parse_ablation_csv(" tier-0 , , full ").unwrap();
        assert_eq!(v, vec![Ablation::Tier0, Ablation::Full]);
    }

    #[test]
    fn parse_ablation_csv_returns_empty_for_empty_input() {
        // 空 CSV / 空白のみは「明示的に 0 件」として返す。fallback 判断は
        // 呼び出し側 (resolve_ablations) で行う。
        assert!(parse_ablation_csv("").unwrap().is_empty());
        assert!(parse_ablation_csv("  ,, ,  ").unwrap().is_empty());
    }

    #[test]
    fn suite_run_options_default_ablations_is_all_four() {
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
            "tier-0,full".into(),
        ];
        let opts = SuiteRunOptions::parse(&args).unwrap();
        assert_eq!(opts.ablations, vec![Ablation::Tier0, Ablation::Full]);
    }
}

#[cfg(all(test, feature = "network"))]
mod dispatch_tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Suite::Smoke を 2 ablation で回し、`<output>/ruler-niah-s/` に
    /// 各 ablation 名の jsonl が並ぶことを確認する。tier-0 は LLM を
    /// 呼ばず、Full のみ呼ぶ → 受信リクエスト数で切り分けられる。
    #[tokio::test]
    async fn smoke_dispatch_runs_each_ablation_and_writes_per_section_jsonl() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": { "role": "assistant", "content": "value answer" }
                }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 2 }
            })))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let opts = SuiteRunOptions {
            suite: Suite::Smoke,
            output_dir: tmp.path().to_path_buf(),
            llm_base_url: server.uri(),
            llm_model: "qwen3.5-4b".into(),
            ablations: vec![Ablation::Tier0, Ablation::Full],
            help: false,
        };
        let report = Suite::Smoke.run(&opts).await.expect("run");
        // 2 sections (tier-0 / full) があり、それぞれ default 4 cases を持つ
        assert_eq!(report.sections.len(), 2);
        let names: Vec<&str> = report.sections.iter().map(|s| s.ablation).collect();
        assert_eq!(names, vec!["tier-0", "full"]);

        // disk に各 ablation の jsonl が並んでいる
        for ab in ["tier-0", "full"] {
            let p = tmp.path().join(format!("ruler-niah-s/{ab}.jsonl"));
            assert!(p.exists(), "{p:?} should exist");
            let content = std::fs::read_to_string(&p).unwrap();
            assert!(!content.is_empty(), "{p:?} should contain cases");
        }
    }
}
