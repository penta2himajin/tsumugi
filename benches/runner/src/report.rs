//! Report writer: emits JSON Lines under `<output_dir>/<bench>/<ablation>.jsonl`.
//!
//! Two write modes are supported:
//!
//! 1. **Final write** (`write`): adapter accumulates all `CaseMetric`s in
//!    memory and `Suite::run` calls `write` once at the end. Simple but
//!    loses partial data if the runner is killed mid-suite (CI timeout).
//! 2. **Incremental write** (`IncrementalSectionWriter`): adapter creates
//!    a writer up front and `write_case` after each LLM call. Each case
//!    is `fsync`'d to disk so timeout / crash leaves a partial jsonl in
//!    `<output_dir>/<bench>/<ablation>.jsonl` that the bench artifact
//!    upload step will still capture.
//!
//! Adapters should prefer (2) for any LLM-bound suite (oracle / smoke /
//! cr) where individual cases take minutes. (1) is fine for unit tests
//! and trivially fast suites.

use crate::metrics::{AggregateMetric, CaseMetric};
use crate::suite::{Ablation, Suite};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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

/// Per-case streaming writer for an `(bench, ablation)` pair.
///
/// Created by an adapter at the start of a suite run; each LLM-bound case
/// pushes a `CaseMetric` via `write_case` which is immediately serialized
/// to disk and fsync'd. On `finish`, builds a `SectionReport` from the
/// collected cases. If the process is killed before `finish`, the partial
/// jsonl on disk preserves all completed cases for post-mortem inspection.
pub struct IncrementalSectionWriter {
    file: fs::File,
    path: PathBuf,
    bench: &'static str,
    ablation: &'static str,
    cases: Vec<CaseMetric>,
}

impl IncrementalSectionWriter {
    /// Create a fresh jsonl at `<output_dir>/<bench>/<ablation.name()>.jsonl`,
    /// truncating any existing content so the file represents only this run.
    pub fn create(
        output_dir: &Path,
        bench: &'static str,
        ablation: Ablation,
    ) -> anyhow::Result<Self> {
        let bench_dir = output_dir.join(bench);
        fs::create_dir_all(&bench_dir)?;
        let path = bench_dir.join(format!("{}.jsonl", ablation.name()));
        let file = fs::File::create(&path)?;
        Ok(Self {
            file,
            path,
            bench,
            ablation: ablation.name(),
            cases: Vec::new(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append a single case to the jsonl and fsync. Mid-run safety: if the
    /// process is killed after this call returns, the case is durable on
    /// disk and the artifact upload step captures it.
    pub fn write_case(&mut self, case: CaseMetric) -> anyhow::Result<()> {
        serde_json::to_writer(&mut self.file, &case)?;
        writeln!(&mut self.file)?;
        // sync_data is sufficient (we don't care about metadata),
        // and faster than sync_all on per-case granularity.
        self.file.sync_data()?;
        self.cases.push(case);
        Ok(())
    }

    /// Consume the writer and return a SectionReport. The jsonl on disk is
    /// already up to date; this just produces the in-memory aggregate for
    /// callers that want to include it in `SuiteReport`.
    pub fn finish(self) -> SectionReport {
        let aggregate = AggregateMetric::from_cases(&self.cases);
        SectionReport {
            bench: self.bench,
            ablation: self.ablation,
            aggregate,
            cases: self.cases,
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
        let cases = vec![CaseMetric::for_full("niah-4k", true, 1234, None, None)];
        report.add_section(SectionReport::new("ruler", Ablation::Tier0, cases));
        write(dir.path(), &report).expect("write");
        let jsonl = std::fs::read_to_string(dir.path().join("ruler/tier-0.jsonl")).unwrap();
        assert!(jsonl.contains("niah-4k"));
        let summary = std::fs::read_to_string(dir.path().join("summary.json")).unwrap();
        assert!(summary.contains("\"bench\": \"ruler\""));
    }

    #[test]
    fn incremental_writer_persists_each_case_immediately() {
        // 各 write_case 後に jsonl がディスクに書き出されていることを保証する
        // (timeout / crash 時の partial data 保護)。
        let dir = tempdir().expect("tempdir");
        let mut writer =
            IncrementalSectionWriter::create(dir.path(), "longmemeval-oracle", Ablation::Full)
                .expect("create");
        writer
            .write_case(CaseMetric::for_full("q1", true, 100, Some(10), Some(20)))
            .expect("write q1");
        // q1 だけの状態で別プロセスから読めることを確認
        let intermediate =
            std::fs::read_to_string(dir.path().join("longmemeval-oracle/full.jsonl")).unwrap();
        assert!(
            intermediate.contains("q1"),
            "q1 not durable: {intermediate}"
        );
        assert!(!intermediate.contains("q2"));

        writer
            .write_case(CaseMetric::for_full("q2", false, 200, Some(30), Some(40)))
            .expect("write q2");
        let after_q2 =
            std::fs::read_to_string(dir.path().join("longmemeval-oracle/full.jsonl")).unwrap();
        assert!(after_q2.contains("q1"));
        assert!(after_q2.contains("q2"));
        // 2 行 (改行終端なので split は 3 要素、最後は空)
        assert_eq!(after_q2.split('\n').filter(|l| !l.is_empty()).count(), 2);

        let report = writer.finish();
        assert_eq!(report.bench, "longmemeval-oracle");
        assert_eq!(report.ablation, "full");
        assert_eq!(report.cases.len(), 2);
        assert_eq!(report.aggregate.correct, 1);
    }

    #[test]
    fn incremental_writer_truncates_existing_file() {
        // 同じ output_dir で再実行された場合、以前の jsonl が残らないこと。
        let dir = tempdir().expect("tempdir");
        {
            let mut w =
                IncrementalSectionWriter::create(dir.path(), "ruler-niah-s", Ablation::Full)
                    .unwrap();
            w.write_case(CaseMetric::for_full("old", false, 1, None, None))
                .unwrap();
        }
        let mut w =
            IncrementalSectionWriter::create(dir.path(), "ruler-niah-s", Ablation::Full).unwrap();
        w.write_case(CaseMetric::for_full("new", true, 2, None, None))
            .unwrap();
        let content = std::fs::read_to_string(dir.path().join("ruler-niah-s/full.jsonl")).unwrap();
        assert!(content.contains("new"));
        assert!(!content.contains("old"));
    }
}
