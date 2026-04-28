//! Report writer: emits JSON Lines under `<output_dir>/<bench>/<ablation>.jsonl`.
//!
//! Phase 4-α Step 1 では構造のみ定義。adapter が `SectionReport` を返す
//! 前提で `SuiteReport` を集約し、Step 3 の Tier ablation matrix で
//! ablation 別 jsonl 出力を埋める。

use crate::metrics::{AggregateMetric, CaseMetric};
use crate::suite::{Ablation, Suite};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct SuiteReport {
    pub suite: &'static str,
    pub sections: Vec<SectionReport>,
}

impl SuiteReport {
    pub fn new(suite: Suite) -> Self {
        Self {
            suite: match suite {
                Suite::Health => "health",
                Suite::Smoke => "smoke",
                Suite::Oracle => "oracle",
                Suite::Cr => "cr",
                Suite::All => "all",
            },
            sections: Vec::new(),
        }
    }

    pub fn add_section(&mut self, section: SectionReport) {
        self.sections.push(section);
    }
}

#[derive(Debug, Serialize)]
pub struct SectionReport {
    pub bench: &'static str,
    pub ablation: &'static str,
    pub aggregate: AggregateMetric,
    pub cases: Vec<CaseMetric>,
}

impl SectionReport {
    pub fn new(bench: &'static str, ablation: Ablation, cases: Vec<CaseMetric>) -> Self {
        let aggregate = AggregateMetric::from_cases(&cases);
        Self {
            bench,
            ablation: ablation.name(),
            aggregate,
            cases,
        }
    }
}

pub fn write(output_dir: &Path, report: &SuiteReport) -> anyhow::Result<()> {
    fs::create_dir_all(output_dir)?;
    for section in &report.sections {
        let bench_dir = output_dir.join(section.bench);
        fs::create_dir_all(&bench_dir)?;
        let path = bench_dir.join(format!("{}.jsonl", section.ablation));
        let mut f = fs::File::create(&path)?;
        for case in &section.cases {
            serde_json::to_writer(&mut f, case)?;
            writeln!(&mut f)?;
        }
    }
    let summary_path = output_dir.join("summary.json");
    let summary = fs::File::create(&summary_path)?;
    serde_json::to_writer_pretty(summary, report)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_creates_per_bench_jsonl_and_summary() {
        let dir = tempdir().expect("tempdir");
        let mut report = SuiteReport::new(Suite::Smoke);
        let cases = vec![CaseMetric {
            case_id: "niah-4k".into(),
            correct: true,
            latency_ms: 1234,
            prompt_tokens: None,
            completion_tokens: None,
        }];
        report.add_section(SectionReport::new("ruler", Ablation::Tier0, cases));
        write(dir.path(), &report).expect("write");
        let jsonl = std::fs::read_to_string(dir.path().join("ruler/tier-0.jsonl")).unwrap();
        assert!(jsonl.contains("niah-4k"));
        let summary = std::fs::read_to_string(dir.path().join("summary.json")).unwrap();
        assert!(summary.contains("\"bench\": \"ruler\""));
    }
}
