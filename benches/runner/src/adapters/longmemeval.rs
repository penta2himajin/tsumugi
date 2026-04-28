//! LongMemEval `_oracle` adapter.
//!
//! Phase 4-α Step 2 で実装予定。30 問層化抽出 (6 question type × 5 問、
//! seed 固定) + 規則ベース primary metric + LLM judge secondary metric。
//! 詳細は `docs/ci-benchmark-integration-plan.md` §「段階的実装計画」。

use crate::report::SectionReport;
use crate::suite::SuiteRunOptions;

pub async fn run_oracle(_opts: &SuiteRunOptions) -> anyhow::Result<SectionReport> {
    anyhow::bail!("LongMemEval oracle adapter is not yet implemented (Phase 4-α Step 2)")
}
