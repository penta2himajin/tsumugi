#!/usr/bin/env bash
# Install a pinned llama.cpp release into ./llama-cpp/.
#
# Phase 4-α Step 1 smoke では Qwen3.5-4B-Instruct を動かすため、計画書
# §「llama.cpp サーバーモード経由」が指す「最新 master 系」build を取る。
# 具体的タグはこの smoke test の結果で確定するため、現状は環境変数で
# 渡す形にし、デフォルトは GitHub Releases API の latest を取得する。

set -euo pipefail

LLAMA_CPP_TAG="${LLAMA_CPP_TAG:-latest}"
INSTALL_DIR="${INSTALL_DIR:-$(pwd)/llama-cpp}"
PLATFORM="${PLATFORM:-ubuntu-x64}"

mkdir -p "${INSTALL_DIR}"

if [[ "${LLAMA_CPP_TAG}" == "latest" ]]; then
  RELEASE_API="https://api.github.com/repos/ggml-org/llama.cpp/releases/latest"
else
  RELEASE_API="https://api.github.com/repos/ggml-org/llama.cpp/releases/tags/${LLAMA_CPP_TAG}"
fi

echo "Resolving llama.cpp release: ${LLAMA_CPP_TAG}"
ASSET_URL=$(curl -fsSL "${RELEASE_API}" \
  | grep -E '"browser_download_url":' \
  | grep -E "llama-.*-bin-${PLATFORM}\.zip" \
  | head -n1 \
  | sed -E 's/.*"(https:[^"]+)".*/\1/')

if [[ -z "${ASSET_URL}" ]]; then
  echo "error: could not find ${PLATFORM} asset for tag ${LLAMA_CPP_TAG}"
  exit 1
fi

echo "Downloading: ${ASSET_URL}"
TMP_ZIP="$(mktemp -t llama-cpp.XXXXXX.zip)"
curl -fSL -o "${TMP_ZIP}" "${ASSET_URL}"
unzip -q -o "${TMP_ZIP}" -d "${INSTALL_DIR}"
rm -f "${TMP_ZIP}"

# release zip 内の build/bin/ を直下に flatten。
if [[ -d "${INSTALL_DIR}/build/bin" ]]; then
  mv "${INSTALL_DIR}/build/bin/"* "${INSTALL_DIR}/"
  rm -rf "${INSTALL_DIR}/build"
fi
chmod +x "${INSTALL_DIR}/llama-server" 2>/dev/null || true

echo "Installed llama.cpp into ${INSTALL_DIR}"
ls -1 "${INSTALL_DIR}" | head -10
