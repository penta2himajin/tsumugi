#!/usr/bin/env bash
# Start llama-server in text-only mode for the smoke test or full bench run.
#
# Usage:
#   start_llama_server.sh qwen3.5-4b   # 主候補 A
#   start_llama_server.sh gemma4-e4b   # 主候補 B
#
# mmproj を渡さない (vision encoder 非ロード) ことで text-only 動作させる。
# 詳細は docs/ci-benchmark-integration-plan.md §「llama.cpp サーバーモード経由」。

set -euo pipefail

MODEL_ID="${1:-}"
PORT="${PORT:-8080}"
CTX_SIZE="${CTX_SIZE:-16384}"
THREADS="${THREADS:-4}"
LLAMA_BIN="${LLAMA_BIN:-./llama-cpp/llama-server}"

case "${MODEL_ID}" in
  qwen3.5-4b)
    HF_REPO="Qwen/Qwen3.5-4B-Instruct"
    QUANT="Q4_K_M"
    ;;
  gemma4-e4b)
    HF_REPO="unsloth/gemma-4-E4B-it-GGUF"
    QUANT="UD-Q4_K_XL"
    ;;
  *)
    echo "usage: $0 <qwen3.5-4b|gemma4-e4b>"
    exit 2
    ;;
esac

echo "Starting llama-server: ${HF_REPO}:${QUANT} on :${PORT} (ctx=${CTX_SIZE}, threads=${THREADS})"
# TODO(Step 1): bg launch + PID file 管理、log redirect。
exec "${LLAMA_BIN}" \
  -hf "${HF_REPO}:${QUANT}" \
  --port "${PORT}" \
  --ctx-size "${CTX_SIZE}" \
  --threads "${THREADS}"
