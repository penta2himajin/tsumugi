#!/usr/bin/env bash
# Poll the llama-server /health endpoint until it reports "ok".
#
# Usage: wait_for_health.sh <url> [timeout-seconds]
# Example: wait_for_health.sh http://localhost:8080/health 120

set -euo pipefail

URL="${1:?usage: $0 <url> [timeout-seconds]}"
TIMEOUT="${2:-120}"
INTERVAL="${INTERVAL:-2}"

deadline=$(( $(date +%s) + TIMEOUT ))
while [[ $(date +%s) -lt ${deadline} ]]; do
  status="$(curl -fsS "${URL}" 2>/dev/null | tr -d '[:space:]' || true)"
  case "${status}" in
    *\"status\":\"ok\"*|ok)
      echo "${URL} is healthy"
      exit 0
      ;;
  esac
  sleep "${INTERVAL}"
done

echo "error: ${URL} did not become healthy within ${TIMEOUT}s"
exit 1
