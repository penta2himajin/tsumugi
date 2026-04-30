#!/usr/bin/env bash
# Download embedding weights (multilingual-e5-small ONNX) into the HF
# hub cache. tier-0-1 (HybridRetriever + OnnxEmbedding) で消費される。
#
# LLM-related downloads (Qwen3.5-4B GGUF / llama.cpp) は LLM 削除と共に
# このスクリプトから除外された。LLMLingua-2 と DistilBART は別 script
# (`download_llmlingua2.sh`, `download_distilbart.sh`) で個別に export する。
#
# 2026-04 時点で `huggingface-cli` は deprecated (no longer works) に
# 降格しており、新 CLI `hf` に置き換える必要がある。
# 本スクリプトは `hf` のみを使う。
#
# 詳細は docs/ci-benchmark-integration-plan.md。

set -euo pipefail

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

echo "Downloading embedding: ${E5_REPO}@${E5_REVISION} (ONNX subdir + tokenizer/config)"
# `--include` を 1 度に複数 pattern 渡すと、後続の値が positional
# `filenames` に吸われて `Ignoring --include since filenames have being
# explicitly set.` の警告で onnx/* が無視される (huggingface_hub
# argparse の挙動)。pattern 毎に flag を repeat する。
hf download \
  "${E5_REPO}" \
  --revision "${E5_REVISION}" \
  --include "onnx/*" \
  --include "tokenizer.json" \
  --include "config.json" \
  --quiet

echo "Done. Cache: ${HF_HUB_CACHE}"
