#!/usr/bin/env bash
# usage: compare_snapshot.sh <baseline.json> <current.json>
#
# tsumugi の bench summary.json (main 最後の通過 snapshot vs 現 PR の
# 結果) を比較し、PASS から FAIL に転じた case があれば exit 1 で job
# を fail させる。新規 case の追加 / 削除は無視する (スキーマ進化を
# 許容)。改善 (FAIL→PASS) は無条件で許容、log のみ。
#
# baseline が無い (= main で初めて bench が走る前の PR) 場合は exit 0。
# data が無い時は無制限に通すというユーザー指定の挙動を実装。
#
# 詳細は `.github/workflows/bench.yml` の `Compare snapshot` step。

set -euo pipefail

BASELINE="${1:-}"
CURRENT="${2:-}"

if [[ -z "$BASELINE" || -z "$CURRENT" ]]; then
  echo "usage: $0 <baseline.json> <current.json>" >&2
  exit 2
fi

if [[ ! -e "$BASELINE" ]]; then
  echo "compare_snapshot: baseline '$BASELINE' not found — initial run, no regression check" >&2
  exit 0
fi
if [[ ! -e "$CURRENT" ]]; then
  echo "compare_snapshot: current '$CURRENT' missing" >&2
  exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "compare_snapshot: jq is required but not installed" >&2
  exit 2
fi

# `summary.json` 構造: { "suite": "...", "sections": [ { "ablation": "...",
# "cases": [ { "case_id": "...", "correct": bool, ... } ] } ] }
#
# 比較粒度は (ablation, case_id)。両 snapshot 共に correct=true だった場合は
# pass、baseline=true & current=false なら regression (exit 1)、
# baseline=false なら無条件 pass (改善判定はスコープ外)。
#
# 新 ablation / 新 case_id の追加は invisibility で許容 (current 側で対応する
# entry が無いだけなら baseline=true でも regression として数えない判断もありうるが、
# ここでは厳密に「baseline で PASS だった (ablation, id) は current に同じ key で
# PASS が無ければ regression」と扱う)。

key_passes() {
  jq -r '
    .sections[]
    | .ablation as $ab
    | .cases[]
    | select(.correct == true)
    | "\($ab)\t\(.case_id)"
  ' "$1" | sort
}

base_passes=$(key_passes "$BASELINE")
cur_passes=$(key_passes "$CURRENT")

# Set difference: in base but not in current → regression.
regressions=$(comm -23 <(echo "$base_passes") <(echo "$cur_passes") || true)

if [[ -z "$regressions" ]]; then
  cur_total=$(echo "$cur_passes" | grep -c '' || true)
  base_total=$(echo "$base_passes" | grep -c '' || true)
  delta=$((cur_total - base_total))
  echo "compare_snapshot: no regression detected (baseline pass=$base_total, current pass=$cur_total, delta=$delta)"
  exit 0
fi

echo "compare_snapshot: REGRESSIONS DETECTED — cases that went PASS -> FAIL:" >&2
while IFS=$'\t' read -r ab id; do
  echo "  - ablation=$ab case_id=$id" >&2
done <<< "$regressions"
exit 1
