#!/usr/bin/env bash
# Download LLM (Qwen3.5-4B-Instruct GGUF) + embedding (multilingual-e5-small ONNX)
# into the HF hub cache. Phase 4-α Step 1 smoke の対象モデルのみ。
#
# 本フェーズは Qwen のみ評価し、Gemma 4 E4B との 2 候補比較は
# Step 1 smoke が安定したら別 PR で再導入する (計画書方針通り)。
#
# 詳細は docs/ci-benchmark-integration-plan.md。

set -euo pipefail

# revision SHA は Step 1 smoke が安定した時点で具体値に pin する。
# それまでは "main" を使い、CI 側で workflow_dispatch input から override 可。
QWEN_REPO="${QWEN_REPO:-Qwen/Qwen3.5-4B-Instruct-GGUF}"
QWEN_REVISION="${QWEN_REVISION:-main}"
QWEN_QUANT_FILE="${QWEN_QUANT_FILE:-Qwen3.5-4B-Instruct-Q4_K_M.gguf}"

E5_REPO="${E5_REPO:-intfloat/multilingual-e5-small}"
E5_REVISION="${E5_REVISION:-main}"

HF_HUB_CACHE="${HF_HUB_CACHE:-${HOME}/.cache/huggingface/hub}"
mkdir -p "${HF_HUB_CACHE}"

if ! command -v huggingface-cli >/dev/null 2>&1; then
  echo "Installing huggingface-hub CLI..."
  pip install --quiet --upgrade "huggingface-hub>=0.25"
fi

echo "Downloading LLM: ${QWEN_REPO}@${QWEN_REVISION} (file: ${QWEN_QUANT_FILE})"
huggingface-cli download \
  "${QWEN_REPO}" \
  "${QWEN_QUANT_FILE}" \
  --revision "${QWEN_REVISION}" \
  --local-dir-use-symlinks False \
  --quiet

echo "Downloading embedding: ${E5_REPO}@${E5_REVISION} (ONNX subdir)"
huggingface-cli download \
  "${E5_REPO}" \
  --revision "${E5_REVISION}" \
  --include "onnx/*" "tokenizer.json" "config.json" \
  --local-dir-use-symlinks False \
  --quiet

echo "Done. Cache: ${HF_HUB_CACHE}"
