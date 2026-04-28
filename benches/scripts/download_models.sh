#!/usr/bin/env bash
# Download LLM (Qwen3.5-4B GGUF) + embedding (multilingual-e5-small ONNX)
# into the HF hub cache. Phase 4-α Step 1 smoke の対象モデルのみ。
#
# 本フェーズは Qwen のみ評価し、Gemma 4 E4B との 2 候補比較は
# Step 1 smoke が安定したら別 PR で再導入する (計画書方針通り)。
#
# 2026-04 時点で `huggingface-cli` は deprecated (no longer works) に
# 降格しており、新 CLI `hf` に置き換える必要がある。
# 本スクリプトは `hf` のみを使う。
#
# 詳細は docs/ci-benchmark-integration-plan.md。

set -euo pipefail

# revision SHA は Step 1 smoke が安定した時点で具体値に pin する。
# それまでは "main" を使い、CI 側で workflow_dispatch input から override 可。
#
# Qwen3.5-4B の HF リポジトリ位置: https://huggingface.co/Qwen/Qwen3.5-4B
# (Qwen 公式は "-Instruct" / "-GGUF" のサフィックスを使わずベース repo に
#  GGUF を同梱する配布形態のため、`*-GGUF` 別 repo は存在しない、2026-04 時点)
QWEN_REPO="${QWEN_REPO:-Qwen/Qwen3.5-4B}"
QWEN_REVISION="${QWEN_REVISION:-main}"
QWEN_QUANT="${QWEN_QUANT:-Q4_K_M}"

E5_REPO="${E5_REPO:-intfloat/multilingual-e5-small}"
E5_REVISION="${E5_REVISION:-main}"

HF_HUB_CACHE="${HF_HUB_CACHE:-${HOME}/.cache/huggingface/hub}"
mkdir -p "${HF_HUB_CACHE}"

# Actions ubuntu-latest は Python + huggingface_hub をプリインストール
# 済みのことが多い (`hf` が利用可能)。不在時のみ pip で install する。
if ! command -v hf >/dev/null 2>&1; then
  echo "Installing huggingface-hub (provides 'hf' CLI)..."
  pip install --quiet --upgrade "huggingface-hub>=0.34"
fi

echo "Downloading LLM: ${QWEN_REPO}@${QWEN_REVISION} (quant: ${QWEN_QUANT})"
# 具体的な GGUF ファイル名は repo によって命名が揺れるため、quant 名を含む
# `.gguf` 全てを include パターンで取得する。typical 命名:
#   Qwen3.5-4B-Q4_K_M.gguf, Qwen3.5-4B-Instruct-Q4_K_M.gguf 等。
if ! hf download \
    "${QWEN_REPO}" \
    --revision "${QWEN_REVISION}" \
    --include "*${QWEN_QUANT}*.gguf" \
    --quiet; then
  echo "error: failed to download GGUF for ${QWEN_REPO}@${QWEN_REVISION}"
  echo "       available files in repo (HF tree API, first 30):"
  curl -fsSL "https://huggingface.co/api/models/${QWEN_REPO}/tree/${QWEN_REVISION}" 2>/dev/null \
    | jq -r '.[].path' 2>/dev/null \
    | head -30 \
    || echo "       (HF tree API call failed — repo may not exist or be private)"
  exit 1
fi

echo "Downloading embedding: ${E5_REPO}@${E5_REVISION} (ONNX subdir + tokenizer/config)"
hf download \
  "${E5_REPO}" \
  --revision "${E5_REVISION}" \
  --include "onnx/*" "tokenizer.json" "config.json" \
  --quiet

echo "Done. Cache: ${HF_HUB_CACHE}"
