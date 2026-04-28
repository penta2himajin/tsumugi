//! `tsumugi-bench` — CI benchmark runner.
//!
//! 詳細は `docs/ci-benchmark-integration-plan.md`。Phase 4-α Step 1 では
//! CLI surface と suite dispatch を skeleton として整備し、各 adapter
//! (longmemeval / memoryagentbench / ruler) は段階的に肉付けする。
//!
//! Step 1 段階では `Ablation` / `metrics::*` / `report::SectionReport::new`
//! などが未配線のため `dead_code` を crate 全体で許容する。Step 3 で
//! adapter 群がそれらを叩くようになった時点で外す。

#![allow(dead_code)]

use std::process::ExitCode;

mod adapters;
mod health;
mod metrics;
mod report;
mod suite;

use suite::SuiteRunOptions;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let opts = match SuiteRunOptions::parse(&args[1..]) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("error: {e}\n\n{}", SuiteRunOptions::usage());
            return ExitCode::from(2);
        }
    };
    if opts.help {
        println!("{}", SuiteRunOptions::usage());
        return ExitCode::SUCCESS;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async move {
        match opts.suite.run(&opts).await {
            Ok(report) => {
                if let Err(e) = report::write(&opts.output_dir, &report) {
                    eprintln!("error: failed to write report: {e}");
                    return ExitCode::FAILURE;
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e:#}");
                ExitCode::FAILURE
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use suite::Suite;

    #[test]
    fn parse_minimal_args() {
        let opts = SuiteRunOptions::parse(&[
            "--suite".into(),
            "smoke".into(),
            "--output".into(),
            "/tmp/out".into(),
        ])
        .expect("parse");
        assert!(matches!(opts.suite, Suite::Smoke));
        assert_eq!(opts.output_dir, std::path::PathBuf::from("/tmp/out"));
    }

    #[test]
    fn parse_help_flag() {
        let opts = SuiteRunOptions::parse(&["--help".into()]).expect("parse");
        assert!(opts.help);
    }

    #[test]
    fn parse_rejects_unknown_suite() {
        let err = SuiteRunOptions::parse(&[
            "--suite".into(),
            "bogus".into(),
            "--output".into(),
            "/tmp/o".into(),
        ])
        .unwrap_err();
        assert!(err.to_string().contains("unknown suite"), "got: {err}");
    }
}
