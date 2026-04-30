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
#   - MemoryAgentBench (`ai-hyz/MemoryAgentBench`, MIT) HF datasets
#       評価対象は `Conflict_Resolution` split のみ (1.5 MB parquet、8 行)。
#       parquet → JSONL 変換を pyarrow で実施 (Rust adapter は serde_json で読む)。
#       配置先: ./benches/data/memoryagentbench_cr.jsonl
#   - RULER は合成生成スクリプトのみ呼び出し、生データは保持しない
#       (Apache 2.0、データ非配布方針)
#
# 環境変数:
#   DATA_DIR              : install destination, default ./benches/data
#   LONGMEMEVAL_REPO      : HF dataset repo, default xiaowu0162/longmemeval
#   LONGMEMEVAL_REVISION  : revision pin, default main (Step 2 安定後に SHA pin)
#   MAB_REPO              : HF dataset repo, default ai-hyz/MemoryAgentBench
#   MAB_REVISION          : revision pin, default main (Step 3 安定後に SHA pin)

set -euo pipefail

DATA_DIR="${DATA_DIR:-$(pwd)/benches/data}"
LONGMEMEVAL_REPO="${LONGMEMEVAL_REPO:-xiaowu0162/longmemeval}"
LONGMEMEVAL_REVISION="${LONGMEMEVAL_REVISION:-main}"
MAB_REPO="${MAB_REPO:-ai-hyz/MemoryAgentBench}"
MAB_REVISION="${MAB_REVISION:-main}"

mkdir -p "${DATA_DIR}"

if ! command -v hf >/dev/null 2>&1; then
  echo "Installing huggingface-hub (provides 'hf' CLI)..."
  pip install --quiet --upgrade "huggingface-hub>=0.34"
fi

# pyarrow は parquet 読み込みに必要 (MemoryAgentBench)。huggingface-hub の
# 推奨 extras では入らないので explicit install。約 70 MB だが download
# step に閉じる (Rust runtime には無関係)。
if ! python3 -c "import pyarrow" >/dev/null 2>&1; then
  echo "Installing pyarrow (for parquet → JSONL conversion)..."
  pip install --quiet pyarrow
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

echo "Downloading dataset: ${MAB_REPO}@${MAB_REVISION} (Conflict_Resolution.parquet)"
hf download \
  "${MAB_REPO}" \
  "data/Conflict_Resolution-00000-of-00001.parquet" \
  --repo-type dataset \
  --revision "${MAB_REVISION}" \
  --local-dir "${DATA_DIR}/memoryagentbench" \
  --quiet

CR_PARQUET="${DATA_DIR}/memoryagentbench/data/Conflict_Resolution-00000-of-00001.parquet"
CR_JSONL="${DATA_DIR}/memoryagentbench_cr.jsonl"

if [[ ! -f "${CR_PARQUET}" ]]; then
  echo "error: expected parquet at ${CR_PARQUET} not found"
  exit 1
fi

echo "Converting parquet → JSONL: ${CR_PARQUET} → ${CR_JSONL}"
SRC="${CR_PARQUET}" DST="${CR_JSONL}" python3 <<'PY'
import json
import os
import sys

import pyarrow.parquet as pq

src = os.environ["SRC"]
dst = os.environ["DST"]
table = pq.read_table(src)
rows = table.to_pylist()
with open(dst, "w", encoding="utf-8") as f:
    for r in rows:
        # answers は List[List[str]] でそのまま JSON 化可能。
        # metadata 内に pyarrow 固有の型が紛れていた場合に備えて
        # default=str で文字列化フォールバック。
        f.write(json.dumps(r, ensure_ascii=False, default=str) + "\n")
print(f"wrote {len(rows)} rows to {dst}", file=sys.stderr)
PY

# round-trip 検証: 出力 JSONL が valid JSON で context/questions/answers を
# 含むことを確認。conversion bug の早期検出。
python3 <<PY
import json
with open("${CR_JSONL}") as f:
    rows = [json.loads(l) for l in f if l.strip()]
assert len(rows) > 0, "JSONL empty"
for i, r in enumerate(rows):
    for key in ("context", "questions", "answers"):
        assert key in r, f"row {i} missing key {key}"
    assert isinstance(r["questions"], list), f"row {i} questions not list"
    assert isinstance(r["answers"], list), f"row {i} answers not list"
print(f"round-trip OK: {len(rows)} rows")
PY

echo "Done. Datasets: ${DATA_DIR}"
ls -la "${DATA_DIR}" 2>/dev/null | head -10
