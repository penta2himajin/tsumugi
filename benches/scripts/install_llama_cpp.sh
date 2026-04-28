#!/usr/bin/env bash
# Install a pinned llama.cpp release into ./llama-cpp/.
#
# Phase 4-α Step 1 smoke では Qwen3.5-4B-Instruct を動かすため、計画書
# §「llama.cpp サーバーモード経由」が指す「最新 master 系」build を取る。
# 具体的タグはこの smoke test の結果で確定するため、現状は環境変数で
# 渡す形にし、デフォルトは GitHub Releases API の latest を取得する。
#
# 環境変数:
#   LLAMA_CPP_TAG : release tag, default `latest`
#   INSTALL_DIR   : install destination, default `./llama-cpp`
#   PLATFORM      : asset filename infix, default `ubuntu-x64`
#   GITHUB_TOKEN  : (optional) bearer token to avoid 60/h rate limit on
#                   anonymous api.github.com calls. CI 上では Actions の
#                   `secrets.GITHUB_TOKEN` を env 経由で渡すと安定する。

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

JSON_FILE="$(mktemp)"
trap 'rm -f "${JSON_FILE}"' EXIT

CURL_AUTH=()
if [[ -n "${GITHUB_TOKEN:-}" ]]; then
  CURL_AUTH=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
fi

echo "Resolving llama.cpp release: ${LLAMA_CPP_TAG}"
echo "  api: ${RELEASE_API}"
echo "  auth: $([[ -n "${GITHUB_TOKEN:-}" ]] && echo 'bearer token' || echo 'anonymous')"

if ! curl -fsSL \
    "${CURL_AUTH[@]}" \
    -H "Accept: application/vnd.github+json" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    "${RELEASE_API}" \
    -o "${JSON_FILE}"; then
  echo "error: failed to fetch ${RELEASE_API}"
  echo "       (rate-limited? try setting GITHUB_TOKEN)"
  exit 1
fi

# 失敗解析を容易にするため、まず利用可能な asset 名を全部出す。
echo "Available assets in this release:"
jq -r '.assets[]?.name' "${JSON_FILE}" | sed 's/^/  /' | head -30

ASSET_NAME_PATTERN="llama-.*-bin-${PLATFORM}\\.tar\\.gz$"
ASSET_URL=$(jq -r --arg pat "${ASSET_NAME_PATTERN}" \
  '.assets[]? | select(.name | test($pat)) | .browser_download_url' \
  "${JSON_FILE}" \
  | head -n1 || true)

if [[ -z "${ASSET_URL}" ]]; then
  echo "error: no asset matched /${ASSET_NAME_PATTERN}/ for tag ${LLAMA_CPP_TAG}"
  echo "       上記の available asset 一覧から PLATFORM env を選び直してください。"
  exit 1
fi

echo "Downloading: ${ASSET_URL}"
TMP_TARBALL="$(mktemp -t llama-cpp.XXXXXX.tar.gz)"
curl -fSL -o "${TMP_TARBALL}" "${ASSET_URL}"
tar -xzf "${TMP_TARBALL}" -C "${INSTALL_DIR}"
rm -f "${TMP_TARBALL}"

# release tarball の internal layout は時期によって異なる:
#   旧: ${INSTALL_DIR}/build/bin/llama-server (... と libllama.so 等)
#   新 (b8950+): ${INSTALL_DIR}/llama-bNNNN/llama-server (...)
# llama-server を `find` で見つけ、その親ディレクトリを INSTALL_DIR
# 直下に flatten する。共有ライブラリも同じ親に居る前提。
SERVER_PATH="$(find "${INSTALL_DIR}" -type f -name "llama-server" 2>/dev/null | head -n1)"
if [[ -z "${SERVER_PATH}" ]]; then
  echo "error: llama-server binary not found in extracted tarball"
  echo "       extracted contents (first 30):"
  find "${INSTALL_DIR}" -maxdepth 3 -printf '  %p\n' | head -30
  exit 1
fi
SERVER_DIR="$(dirname "${SERVER_PATH}")"
if [[ "${SERVER_DIR}" != "${INSTALL_DIR}" ]]; then
  echo "Flattening: ${SERVER_DIR} -> ${INSTALL_DIR}"
  # mv は dotfiles を漏らすので、shopt dotglob 込みで対応。
  shopt -s dotglob nullglob
  mv "${SERVER_DIR}"/* "${INSTALL_DIR}/"
  shopt -u dotglob nullglob
  # 空になった旧階層 (build/bin/, llama-bNNNN/ 等) を最大 2 段まで掃除
  rmdir "${SERVER_DIR}" 2>/dev/null || true
  rmdir "$(dirname "${SERVER_DIR}")" 2>/dev/null || true
fi
chmod +x "${INSTALL_DIR}/llama-server" 2>/dev/null || true

echo "Installed llama.cpp into ${INSTALL_DIR}"
ls -1 "${INSTALL_DIR}" | head -20
