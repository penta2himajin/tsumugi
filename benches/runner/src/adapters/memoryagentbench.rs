//! MemoryAgentBench `Conflict_Resolution` adapter.
//!
//! Phase 4-α Step 3 で実装予定。8 問全問、Mayu の supersession 直接検証
//! 軸として最重要。詳細は `docs/ci-benchmark-integration-plan.md`。

use crate::report::SectionReport;
use crate::suite::SuiteRunOptions;

pub async fn run_conflict_resolution(_opts: &SuiteRunOptions) -> anyhow::Result<SectionReport> {
    anyhow::bail!(
        "MemoryAgentBench Conflict_Resolution adapter is not yet implemented \
         (Phase 4-α Step 3)"
    )
}
