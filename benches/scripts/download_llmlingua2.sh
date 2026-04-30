#!/usr/bin/env bash
# Export `microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank` to
# ONNX via HF Optimum and emit `TSUMUGI_LLMLINGUA2_MODEL_PATH` /
# `TSUMUGI_LLMLINGUA2_TOKENIZER_PATH` lines on stdout for sourcing into
# `$GITHUB_ENV` (or `eval`-ing locally).
#
# Usage (in CI):
#   ./benches/scripts/download_llmlingua2.sh >> "$GITHUB_ENV"
#
# Why export here instead of pulling pre-built ONNX from HF Hub?
# Microsoft does not ship an ONNX variant of the LLMLingua-2 mBERT model;
# the published artifact is HF safetensors. Exporting at CI install time
# keeps the dependency surface inside the bench job and avoids relying on
# third-party mirrors.
#
# 出力先 default: `${HOME}/.cache/tsumugi/llmlingua2-mbert/{model.onnx,tokenizer.json}`
# 既に export 済みなら skip して env line だけ印字する。
#
# 詳細は docs/llm-free-stack-plan.md § 5.2 (2)。

set -euo pipefail

LLMLINGUA2_REPO="${LLMLINGUA2_REPO:-microsoft/llmlingua-2-bert-base-multilingual-cased-meetingbank}"
LLMLINGUA2_DIR="${LLMLINGUA2_DIR:-${HOME}/.cache/tsumugi/llmlingua2-mbert}"

mkdir -p "${LLMLINGUA2_DIR}"

if [[ -e "${LLMLINGUA2_DIR}/model.onnx" && -e "${LLMLINGUA2_DIR}/tokenizer.json" ]]; then
  echo "LLMLingua-2 already exported at ${LLMLINGUA2_DIR}" >&2
else
  if ! python3 -c "import optimum.onnxruntime" 2>/dev/null; then
    echo "Installing optimum + onnxruntime for LLMLingua-2 export..." >&2
    pip install --quiet --upgrade "optimum[onnxruntime]>=1.20"
  fi

  echo "Exporting ${LLMLINGUA2_REPO} -> ${LLMLINGUA2_DIR}" >&2
  # `--task token-classification` で per-token binary head 付きで export。
  # default の auto detect は LLMLingua-2 では誤検出の余地があるため明示。
  optimum-cli export onnx \
    --model "${LLMLINGUA2_REPO}" \
    --task token-classification \
    "${LLMLINGUA2_DIR}" >&2

  echo "Done. Files in ${LLMLINGUA2_DIR}:" >&2
  ls -la "${LLMLINGUA2_DIR}" >&2
fi

if [[ ! -e "${LLMLINGUA2_DIR}/model.onnx" ]]; then
  echo "error: ${LLMLINGUA2_DIR}/model.onnx missing after export" >&2
  exit 1
fi
if [[ ! -e "${LLMLINGUA2_DIR}/tokenizer.json" ]]; then
  echo "error: ${LLMLINGUA2_DIR}/tokenizer.json missing after export" >&2
  exit 1
fi

echo "TSUMUGI_LLMLINGUA2_MODEL_PATH=${LLMLINGUA2_DIR}/model.onnx"
echo "TSUMUGI_LLMLINGUA2_TOKENIZER_PATH=${LLMLINGUA2_DIR}/tokenizer.json"
