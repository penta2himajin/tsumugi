#!/usr/bin/env bash
# scripts/regen.sh — regenerate tsumugi-core/src/gen/ and tsumugi-ts/src/gen/
# from models/ via oxidtr.
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
RUST_OUT="${REPO_ROOT}/tsumugi-core/src/gen"
TS_OUT="${REPO_ROOT}/tsumugi-ts/src/gen"

# --- Rust ----------------------------------------------------------------
echo "==> clearing ${RUST_OUT}"
rm -rf "${RUST_OUT}"
mkdir -p "${RUST_OUT}"

echo "==> generating Rust from ${MAIN_ALS}"
"${OXIDTR_BIN}" generate "${MAIN_ALS}" --target rust --output "${RUST_OUT}"

# oxidtr emits `pub mod` / `pub use` in declaration order; rustfmt sorts them
# alphabetically. Normalize here so the committed state matches what anyone
# (local dev or CI drift check) produces.
echo "==> cargo fmt on tsumugi-core (normalizes gen/)"
(cd "${REPO_ROOT}" && cargo fmt -p tsumugi-core)

# --- TypeScript ----------------------------------------------------------
# Only the types subtree (models.ts + helpers.ts) is wired into tsumugi-ts;
# `operations.ts` / `fixtures.ts` / `validators.ts` / `tests.ts` ship Error
# stubs or rely on fixtures we don't use in Phase 3. They're gitignored at
# tsumugi-ts/src/gen/ so they can exist on disk without polluting history.
TS_STAGING="$(mktemp -d)"
trap 'rm -rf "${TS_STAGING}"' EXIT
echo "==> generating TypeScript from ${MAIN_ALS} (staging)"
"${OXIDTR_BIN}" generate "${MAIN_ALS}" --target ts --output "${TS_STAGING}"

mkdir -p "${TS_OUT}"
cp "${TS_STAGING}/models.ts" "${TS_OUT}/models.ts"
cp "${TS_STAGING}/helpers.ts" "${TS_OUT}/helpers.ts"

# --- Verify --------------------------------------------------------------
echo "==> running cargo check --all-features"
(cd "${REPO_ROOT}" && cargo check --all-features --quiet)

if command -v bun >/dev/null 2>&1; then
  echo "==> running tsumugi-ts typecheck"
  (cd "${REPO_ROOT}/tsumugi-ts" && bun run typecheck)
else
  echo "==> skipping tsumugi-ts typecheck (bun not found on PATH)"
fi

echo "done."
