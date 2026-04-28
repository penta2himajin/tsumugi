#!/usr/bin/env bash
# Download benchmark datasets into ./benches/data/.
#
# 取得対象 (ライセンスはすべて Apache 2.0 / MIT、詳細は
# docs/evaluation-datasets.md):
#
#   - LongMemEval (`xiaowu0162/longmemeval`, MIT)
#       評価対象は `_oracle` サブセットのみ (evidence-only)、30 問層化抽出
#   - MemoryAgentBench (`memory-agent-bench/MemoryAgentBench`, MIT)
#       `Conflict_Resolution` split 全 8 問
#   - RULER は合成生成スクリプトのみ呼び出し、生データは保持しない
#       (Apache 2.0、データ非配布方針)
#
# Phase 4-α Step 1 では skeleton。HF revision SHA pin は Step 2 で実装。

set -euo pipefail

DATA_DIR="${DATA_DIR:-$(pwd)/benches/data}"
mkdir -p "${DATA_DIR}"

# TODO(Step 2): HF datasets revision SHA を pin、curl / hf CLI で取得
echo "TODO: download LongMemEval / MemoryAgentBench with pinned HF revisions."
echo "      RULER は scripts 経由で合成生成 (raw data は配布しない)."
exit 1
