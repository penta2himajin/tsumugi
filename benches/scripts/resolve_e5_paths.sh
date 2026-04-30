#!/usr/bin/env bash
# Resolve `intfloat/multilingual-e5-small` ONNX model + tokenizer paths
# from the HF hub cache and emit them as KEY=VALUE lines suitable for
# sourcing into `$GITHUB_ENV` (or eval-ing into a local shell).
#
# Usage (in CI):
#   ./benches/scripts/resolve_e5_paths.sh >> "$GITHUB_ENV"
#
# Pre-condition: `download_models.sh` has already been executed and the
# e5 ONNX weights live under `${HF_HUB_CACHE}/models--intfloat--multilingual-e5-small/snapshots/*/`.
#
# The runner reads these env vars to switch tier-0-1 embedding from the
# default MockEmbedding (FNV-1a 64-dim) to OnnxEmbedding (e5-small 384-dim).
# 詳細は docs/llm-free-stack-plan.md § 5.2 (1)。

set -euo pipefail

HF_HUB_CACHE="${HF_HUB_CACHE:-${HOME}/.cache/huggingface/hub}"
REPO_DIR="${HF_HUB_CACHE}/models--intfloat--multilingual-e5-small"

if [[ ! -d "${REPO_DIR}" ]]; then
  echo "error: ${REPO_DIR} not found. Run download_models.sh first." >&2
  exit 1
fi

# snapshots/<sha>/ にモデル本体が配置される。通常は 1 sha のみだが、
# 念のため最初に見つかったものを採用。
SNAPSHOT_DIR=$(ls -d "${REPO_DIR}/snapshots/"*/ 2>/dev/null | head -1 || true)
if [[ -z "${SNAPSHOT_DIR}" ]]; then
  echo "error: no snapshot directory under ${REPO_DIR}/snapshots/" >&2
  echo "       cache contents:" >&2
  ls -la "${REPO_DIR}/" >&2 || true
  exit 1
fi

# onnx subdir には model.onnx / model_quantized.onnx / model_O1.onnx 等が
# 並びうる。canonical な model.onnx を最優先し、なければ非 quantized
# な最初の .onnx ファイルを採用する。Phase 4-γ Step 1 では精度優先で
# 量子化版は使わない。
MODEL_PATH=""
if [[ -e "${SNAPSHOT_DIR}onnx/model.onnx" ]]; then
  MODEL_PATH="${SNAPSHOT_DIR}onnx/model.onnx"
else
  MODEL_PATH=$(ls "${SNAPSHOT_DIR}onnx/"*.onnx 2>/dev/null \
    | grep -v quantized \
    | head -1 || true)
fi
if [[ -z "${MODEL_PATH}" ]]; then
  echo "error: no usable .onnx file under ${SNAPSHOT_DIR}onnx/" >&2
  echo "       directory contents:" >&2
  ls -la "${SNAPSHOT_DIR}onnx/" >&2 || true
  exit 1
fi

TOKENIZER_PATH="${SNAPSHOT_DIR}tokenizer.json"
if [[ ! -e "${TOKENIZER_PATH}" ]]; then
  echo "error: ${TOKENIZER_PATH} not found" >&2
  exit 1
fi

echo "TSUMUGI_E5_MODEL_PATH=${MODEL_PATH}"
echo "TSUMUGI_E5_TOKENIZER_PATH=${TOKENIZER_PATH}"
