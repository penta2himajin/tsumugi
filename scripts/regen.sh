#!/usr/bin/env bash
# scripts/regen.sh — regenerate tsumugi-core/src/gen/ from models/ via oxidtr.
#
# Requires a local clone of penta2himajin/oxidtr. Pass its path as the first
# argument, or set OXIDTR_HOME, or default to ../oxidtr.
#
# Usage:
#   scripts/regen.sh                       # uses ../oxidtr
#   scripts/regen.sh /path/to/oxidtr       # explicit path
#   OXIDTR_HOME=/path scripts/regen.sh
#
# The script always rebuilds oxidtr with `cargo build --release` (incremental,
# cheap on warm targets) to guarantee we run a current version.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OXIDTR_HOME="${1:-${OXIDTR_HOME:-${REPO_ROOT}/../oxidtr}}"

if [[ ! -d "${OXIDTR_HOME}" ]]; then
  echo "error: oxidtr repo not found at ${OXIDTR_HOME}" >&2
  echo "  clone it: git clone https://github.com/penta2himajin/oxidtr.git" >&2
  exit 1
fi

echo "==> building oxidtr from ${OXIDTR_HOME}"
(cd "${OXIDTR_HOME}" && cargo build --release --quiet)

OXIDTR_BIN="${OXIDTR_HOME}/target/release/oxidtr"
MAIN_ALS="${REPO_ROOT}/models/tsumugi.als"
OUTPUT_DIR="${REPO_ROOT}/tsumugi-core/src/gen"

echo "==> clearing ${OUTPUT_DIR}"
rm -rf "${OUTPUT_DIR}"
mkdir -p "${OUTPUT_DIR}"

echo "==> generating from ${MAIN_ALS}"
"${OXIDTR_BIN}" generate "${MAIN_ALS}" --target rust --output "${OUTPUT_DIR}"

# oxidtr emits `pub mod` / `pub use` in declaration order; rustfmt sorts them
# alphabetically. Normalize here so the committed state matches what anyone
# (local dev or CI drift check) produces.
echo "==> cargo fmt on tsumugi-core (normalizes gen/)"
(cd "${REPO_ROOT}" && cargo fmt -p tsumugi-core)

echo "==> running cargo check --all-features"
(cd "${REPO_ROOT}" && cargo check --all-features --quiet)

echo "done."
