#!/usr/bin/env bash
# Install a pinned llama.cpp release into ./llama-cpp/.
#
# Phase 4-α Step 1 では skeleton。tag pin は smoke test の結果で確定する
# (Qwen3.5-4B が動く master 系か、Gemma 4 E4B が動けば足りる安定 tag か)。
# 詳細は docs/ci-benchmark-integration-plan.md §「llama.cpp サーバーモード経由」。

set -euo pipefail

# TODO(Step 1 smoke test): 主候補確定後に具体的 tag に置換。
LLAMA_CPP_TAG="${LLAMA_CPP_TAG:-PLACEHOLDER}"
INSTALL_DIR="${INSTALL_DIR:-$(pwd)/llama-cpp}"

if [[ "${LLAMA_CPP_TAG}" == "PLACEHOLDER" ]]; then
  echo "error: LLAMA_CPP_TAG is not pinned yet. Set it to a concrete release tag."
  echo "       (Step 1 smoke test で確定する)"
  exit 1
fi

mkdir -p "${INSTALL_DIR}"
echo "Pinned llama.cpp tag: ${LLAMA_CPP_TAG}"
echo "Install dir:           ${INSTALL_DIR}"
echo "TODO: download release binary from ggml-org/llama.cpp Releases."
exit 1
