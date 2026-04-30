#!/usr/bin/env bash
# Export `sshleifer/distilbart-cnn-6-6` to ONNX via HF Optimum and emit
# `TSUMUGI_DISTILBART_DIR` line on stdout for sourcing into `$GITHUB_ENV`
# (or `eval`-ing locally).
#
# Usage (in CI):
#   ./benches/scripts/download_distilbart.sh >> "$GITHUB_ENV"
#
# Why export here instead of pulling pre-built ONNX from HF Hub?
# Sam Shleifer's `sshleifer/distilbart-cnn-6-6` ships PyTorch weights
# only. Optimum's `text2text-generation-with-past` task exports three
# ONNX graphs (encoder, decoder, decoder-with-past) so we can run greedy
# generation with KV cache reuse, which is mandatory for sub-second
# CPU summaries on 1K-tok inputs (without KV cache the decoder loop
# is O(n²) re-attending the encoder hidden states each step).
#
# Note: `summarization-with-past` is NOT a valid Optimum task for BART
# (Optimum only ships the canonical `text2text-generation-with-past`
# task for encoder-decoder graphs). The two produce identical ONNX
# topology — the difference is purely the task label Optimum uses to
# pick the export config.
#
# 出力先 default: `${HOME}/.cache/tsumugi/distilbart-cnn-6-6/`
#   ├── encoder_model.onnx
#   ├── decoder_model.onnx
#   ├── decoder_with_past_model.onnx
#   ├── tokenizer.json
#   └── config.json
#
# 既に export 済みなら skip。
#
# 詳細は docs/llm-free-stack-plan.md § 5.2 (5)。

set -euo pipefail

DISTILBART_REPO="${DISTILBART_REPO:-sshleifer/distilbart-cnn-6-6}"
DISTILBART_DIR="${DISTILBART_DIR:-${HOME}/.cache/tsumugi/distilbart-cnn-6-6}"

mkdir -p "${DISTILBART_DIR}"

required_files=(
  "encoder_model.onnx"
  "decoder_model.onnx"
  "decoder_with_past_model.onnx"
  "tokenizer.json"
  "config.json"
)

all_present=true
for f in "${required_files[@]}"; do
  if [[ ! -e "${DISTILBART_DIR}/${f}" ]]; then
    all_present=false
    break
  fi
done

if [[ "${all_present}" == "true" ]]; then
  echo "DistilBART already exported at ${DISTILBART_DIR}" >&2
else
  if ! python3 -c "import optimum.onnxruntime" 2>/dev/null; then
    echo "Installing optimum + onnxruntime for DistilBART export..." >&2
    pip install --quiet --upgrade "optimum[onnxruntime]>=1.20"
  fi

  echo "Exporting ${DISTILBART_REPO} -> ${DISTILBART_DIR}" >&2
  # `--task text2text-generation-with-past` で encoder + decoder +
  # decoder-with-past の 3 ONNX graph を出力する。BART は Optimum 上
  # `text2text-generation` (encoder-decoder) として扱われ、
  # `summarization-with-past` という task 名は未サポート (Optimum
  # 1.20+ でも同じ)。`-with-past` suffix が KV cache 入出力を graph に
  # 含めるためのフラグ。auto detect は config.json の architectures に
  # `BartForConditionalGeneration` が入っていれば正しく当たるが明示。
  optimum-cli export onnx \
    --model "${DISTILBART_REPO}" \
    --task text2text-generation-with-past \
    "${DISTILBART_DIR}" >&2

  echo "Done. Files in ${DISTILBART_DIR}:" >&2
  ls -la "${DISTILBART_DIR}" >&2
fi

for f in "${required_files[@]}"; do
  if [[ ! -e "${DISTILBART_DIR}/${f}" ]]; then
    echo "error: ${DISTILBART_DIR}/${f} missing after export" >&2
    exit 1
  fi
done

echo "TSUMUGI_DISTILBART_DIR=${DISTILBART_DIR}"
