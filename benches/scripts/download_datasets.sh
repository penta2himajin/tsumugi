#!/usr/bin/env bash
# Download benchmark datasets into ./benches/data/.
#
# 取得対象 (ライセンスはすべて Apache 2.0 / MIT、詳細は
# docs/evaluation-datasets.md):
#
#   - LongMemEval (`xiaowu0162/longmemeval`, MIT) HF datasets
#       評価対象は `_oracle` サブセットのみ (evidence-only)、
#       30 問層化抽出は runner 側で実施
#       配置先: ./benches/data/longmemeval_oracle (~15 MB JSON)
#   - MemoryAgentBench (Step 3 で追加)
#   - RULER は合成生成スクリプトのみ呼び出し、生データは保持しない
#       (Apache 2.0、データ非配布方針)
#
# 環境変数:
#   DATA_DIR              : install destination, default ./benches/data
#   LONGMEMEVAL_REPO      : HF dataset repo, default xiaowu0162/longmemeval
#   LONGMEMEVAL_REVISION  : revision pin, default main (Step 2 安定後に SHA pin)

set -euo pipefail

DATA_DIR="${DATA_DIR:-$(pwd)/benches/data}"
LONGMEMEVAL_REPO="${LONGMEMEVAL_REPO:-xiaowu0162/longmemeval}"
LONGMEMEVAL_REVISION="${LONGMEMEVAL_REVISION:-main}"

mkdir -p "${DATA_DIR}"

if ! command -v hf >/dev/null 2>&1; then
  echo "Installing huggingface-hub (provides 'hf' CLI)..."
  pip install --quiet --upgrade "huggingface-hub>=0.34"
fi

echo "Downloading dataset: ${LONGMEMEVAL_REPO}@${LONGMEMEVAL_REVISION} (longmemeval_oracle)"
# `--repo-type dataset` で datasets 名前空間を明示。
# `local-dir` で benches/data/ 直下に配置 (HF cache を経由せず展開)。
hf download \
  "${LONGMEMEVAL_REPO}" \
  longmemeval_oracle \
  --repo-type dataset \
  --revision "${LONGMEMEVAL_REVISION}" \
  --local-dir "${DATA_DIR}/longmemeval" \
  --quiet

# runner は LONGMEMEVAL_PATH (default benches/data/longmemeval_oracle)
# を読むので shim symlink を張る。
ln -sf "${DATA_DIR}/longmemeval/longmemeval_oracle" "${DATA_DIR}/longmemeval_oracle"

echo "Done. Datasets: ${DATA_DIR}"
ls -la "${DATA_DIR}" 2>/dev/null | head -10
