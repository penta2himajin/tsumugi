#!/usr/bin/env bash
# Start llama-server in text-only mode for the Phase 4-α Step 1 smoke test.
#
# 本フェーズは Qwen3.5-4B のみを起動する。Gemma 4 E4B との 2 候補並列
# 評価は smoke が安定したら別 PR で再導入する。
#
# 環境変数:
#   PORT         : llama-server bind port (default: 8080)
#   CTX_SIZE     : context window (default: 16384、Step 1 smoke は短文のみ)
#   THREADS      : CPU threads (default: 4、CI runner 標準)
#   LLAMA_BIN    : path to llama-server (default: ./llama-cpp/llama-server)
#   QWEN_REPO    : HF repo id (default: unsloth/Qwen3.5-4B-GGUF)
#   QWEN_QUANT   : llama.cpp -hf 形式の quant tag (default: Q4_K_M)
#   HF_HUB_CACHE : HF hub cache root (default: $HOME/.cache/huggingface/hub)
#                  download_models.sh と一致させること。
#
# 起動戦略: HF hub cache に GGUF が既にあれば `-m` で直接ロード。
# 無ければ `-hf` で fallback (llama.cpp が自前 cache に取得する経路、
# 別個に ~3GB 再ダウンロードが発生する)。

set -euo pipefail

PORT="${PORT:-8080}"
CTX_SIZE="${CTX_SIZE:-16384}"
THREADS="${THREADS:-4}"
LLAMA_BIN="${LLAMA_BIN:-./llama-cpp/llama-server}"
QWEN_REPO="${QWEN_REPO:-unsloth/Qwen3.5-4B-GGUF}"
QWEN_QUANT="${QWEN_QUANT:-Q4_K_M}"
HF_HUB_CACHE="${HF_HUB_CACHE:-${HOME}/.cache/huggingface/hub}"

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

# HF hub cache から download 済み GGUF を探す。`hf download` は
# `<HF_HUB_CACHE>/models--<owner>--<repo>/snapshots/<sha>/<file>` 配置。
QWEN_REPO_DIR_NAME="models--${QWEN_REPO//\//--}"
GGUF_PATH="$(find "${HF_HUB_CACHE}/${QWEN_REPO_DIR_NAME}/snapshots" \
  -type f -name "*${QWEN_QUANT}*.gguf" 2>/dev/null \
  | head -n1 || true)"

# Qwen3.5 系は thinking mode をデフォルトで有効にしており、出力の
# `<think>...</think>` 部分が llama-server の `--reasoning-format auto`
# (default) で `reasoning_content` フィールドに分離される。max_tokens 内
# に answer に到達せず終了するケースを避けるため、`enable_thinking=false`
# を chat template に渡して thinking 自体を無効化する。
# (Qwen3.5 公式: `/no_think` directive は **サポート外**、API 側で
#  `chat_template_kwargs` を渡すのが正規のやり方)
COMMON_ARGS=(
  --port "${PORT}"
  --ctx-size "${CTX_SIZE}"
  --threads "${THREADS}"
  --chat-template-kwargs '{"enable_thinking":false}'
)

echo "  LD_LIBRARY_PATH=${LD_LIBRARY_PATH}"
if [[ -n "${GGUF_PATH}" && -f "${GGUF_PATH}" ]]; then
  echo "Starting llama-server: -m ${GGUF_PATH} on :${PORT} (ctx=${CTX_SIZE}, threads=${THREADS}, thinking=off)"
  exec "${LLAMA_BIN}" \
    -m "${GGUF_PATH}" \
    "${COMMON_ARGS[@]}"
else
  echo "warning: GGUF not found in HF cache (${HF_HUB_CACHE}/${QWEN_REPO_DIR_NAME})"
  echo "         falling back to -hf (will re-download via llama.cpp's own cache)"
  echo "Starting llama-server: -hf ${QWEN_REPO}:${QWEN_QUANT} on :${PORT} (ctx=${CTX_SIZE}, threads=${THREADS}, thinking=off)"
  exec "${LLAMA_BIN}" \
    -hf "${QWEN_REPO}:${QWEN_QUANT}" \
    "${COMMON_ARGS[@]}"
fi
