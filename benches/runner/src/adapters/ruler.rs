//! RULER NIAH-S (Single needle) adapter.
//!
//! Phase 4-α Step 3 で実装予定。`niah_single_2` を seq_len ∈
//! {4K, 8K, 16K, 32K, 64K} で各 1 ケース、計 5 ケース。Tier 0 (BM25)
//! baseline の確認に使う。詳細は `docs/ci-benchmark-integration-plan.md`。

use crate::report::SectionReport;
use crate::suite::SuiteRunOptions;

pub async fn run_niah_s(_opts: &SuiteRunOptions) -> anyhow::Result<SectionReport> {
    anyhow::bail!("RULER NIAH-S adapter is not yet implemented (Phase 4-α Step 3)")
}
