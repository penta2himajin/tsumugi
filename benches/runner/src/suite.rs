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
}

#[derive(Debug, Clone)]
pub struct SuiteRunOptions {
    pub suite: Suite,
    pub output_dir: PathBuf,
    pub llm_base_url: String,
    pub llm_model: String,
    pub help: bool,
}

impl SuiteRunOptions {
    pub fn usage() -> &'static str {
        "Usage: tsumugi-bench --suite <health|smoke|oracle|cr|all> --output <dir> \
         [--llm-base-url <url>] [--llm-model <name>] [--help]\n\n\
         Phase 4-α Step 1: --suite health のみ実装済み \
         (LLM 起動健全性 + 生成速度 + 簡易指示追従)。他の suite は \
         Step 2-3 で順次実装。"
    }

    pub fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut suite: Option<Suite> = None;
        let mut output_dir: Option<PathBuf> = None;
        let mut llm_base_url = String::from("http://localhost:8080/v1");
        let mut llm_model = String::from("Qwen/Qwen3.5-4B-Instruct");
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
                "--help" | "-h" => {
                    help = true;
                }
                other => anyhow::bail!("unknown argument: {other}"),
            }
        }
        if help {
            return Ok(Self {
                suite: Suite::Health,
                output_dir: PathBuf::new(),
                llm_base_url,
                llm_model,
                help: true,
            });
        }
        Ok(Self {
            suite: suite.ok_or_else(|| anyhow::anyhow!("--suite is required"))?,
            output_dir: output_dir.ok_or_else(|| anyhow::anyhow!("--output is required"))?,
            llm_base_url,
            llm_model,
            help: false,
        })
    }
}

impl Suite {
    pub async fn run(&self, opts: &SuiteRunOptions) -> anyhow::Result<SuiteReport> {
        let mut report = SuiteReport::new(*self);
        match self {
            Suite::Health => {
                report.add_section(crate::health::run_health(opts).await?);
            }
            Suite::Smoke => {
                report.add_section(adapters::ruler::run_niah_s(opts).await?);
            }
            Suite::Oracle => {
                report.add_section(adapters::longmemeval::run_oracle(opts).await?);
            }
            Suite::Cr => {
                report
                    .add_section(adapters::memoryagentbench::run_conflict_resolution(opts).await?);
            }
            Suite::All => {
                report.add_section(adapters::ruler::run_niah_s(opts).await?);
                report.add_section(adapters::longmemeval::run_oracle(opts).await?);
                report
                    .add_section(adapters::memoryagentbench::run_conflict_resolution(opts).await?);
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
}
