#!/usr/bin/env bash
# Start llama-server in text-only mode for the Phase 4-α Step 1 smoke test.
#
# 本フェーズは Qwen3.5-4B-Instruct のみを起動する。Gemma 4 E4B との
# 2 候補並列評価は smoke が安定したら別 PR で再導入する。
#
# 環境変数:
#   PORT         : llama-server bind port (default: 8080)
#   CTX_SIZE     : context window (default: 16384、Step 1 smoke は短文のみ)
#   THREADS      : CPU threads (default: 4、CI runner 標準)
#   LLAMA_BIN    : path to llama-server (default: ./llama-cpp/llama-server)
#   QWEN_REPO    : HF repo id (default: Qwen/Qwen3.5-4B-Instruct-GGUF)
#   QWEN_QUANT   : llama.cpp -hf 形式の quant tag (default: Q4_K_M)

set -euo pipefail

PORT="${PORT:-8080}"
CTX_SIZE="${CTX_SIZE:-16384}"
THREADS="${THREADS:-4}"
LLAMA_BIN="${LLAMA_BIN:-./llama-cpp/llama-server}"
QWEN_REPO="${QWEN_REPO:-Qwen/Qwen3.5-4B}"
QWEN_QUANT="${QWEN_QUANT:-Q4_K_M}"

if [[ ! -x "${LLAMA_BIN}" ]]; then
  echo "error: llama-server binary not found at ${LLAMA_BIN}"
  echo "       run benches/scripts/install_llama_cpp.sh first"
  exit 1
fi

# llama.cpp の Linux release tarball は libllama.so / libggml*.so を
# 含む。binary に $ORIGIN RUNPATH が無い build があるため、防御的に
# LD_LIBRARY_PATH に binary 同居ディレクトリを足す。
LLAMA_BIN_DIR="$(cd "$(dirname "${LLAMA_BIN}")" && pwd)"
export LD_LIBRARY_PATH="${LLAMA_BIN_DIR}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"

echo "Starting llama-server: ${QWEN_REPO}:${QWEN_QUANT} on :${PORT} (ctx=${CTX_SIZE}, threads=${THREADS})"
echo "  LD_LIBRARY_PATH=${LD_LIBRARY_PATH}"
# mmproj を渡さないことで vision encoder ロードを回避し、純テキスト推論で動かす。
exec "${LLAMA_BIN}" \
  -hf "${QWEN_REPO}:${QWEN_QUANT}" \
  --port "${PORT}" \
  --ctx-size "${CTX_SIZE}" \
  --threads "${THREADS}"
