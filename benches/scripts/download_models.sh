#!/usr/bin/env bash
# Download LLM + embedding models into the HF hub cache.
#
# 取得対象 (詳細は docs/ci-benchmark-integration-plan.md):
#
#   LLM 主候補 (smoke test で 1 つに確定):
#     A: Qwen/Qwen3.5-4B-Instruct  (Apache 2.0, 262K ctx, Hybrid Gated DeltaNet)
#     B: google/gemma-4-e4b-it     (Apache 2.0 [2026-03 化], 128K ctx)
#   Embedding:
#     intfloat/multilingual-e5-small  (MIT, ONNX 配布)
#     BAAI/bge-small-en-v1.5          (MIT, fallback)
#
# Phase 4-α Step 1 では両 LLM 候補を取得し smoke test する。確定後は
# 採用したモデルのみダウンロードに変更。

set -euo pipefail

# TODO(Step 1 smoke test 後): 採用 LLM のみに絞る + revision SHA pin。
HF_HUB_CACHE="${HF_HUB_CACHE:-${HOME}/.cache/huggingface/hub}"
mkdir -p "${HF_HUB_CACHE}"

echo "TODO: download Qwen3.5-4B-Instruct GGUF (Q4_K_M)"
echo "TODO: download Gemma 4 E4B-it GGUF (Q4_K_M / UD-Q4_K_XL)"
echo "TODO: download intfloat/multilingual-e5-small ONNX"
echo "(Step 1 smoke test 完了後、HF revision SHA を pin して 1 候補に絞る)"
exit 1
