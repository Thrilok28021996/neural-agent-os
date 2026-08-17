#!/usr/bin/env bash
# Live-API integration test harness.
# Starts the release app against an isolated temporary data directory, runs
# tests/integration/api.test.ts against it, then shuts the app down.
set -euo pipefail
cd "$(dirname "$0")/.."

RELEASE_BIN="src-tauri/target/release/neural-agent-os"
BUILD="${NAO_LIVE_API_BUILD:-1}"
if [[ "${BUILD}" == "1" ]]; then
  echo "[live-api] building release binary..."
  (cd src-tauri && cargo build --release)
fi

TEST_HOME="$(mktemp -d)"
echo "[live-api] isolated data dir: ${TEST_HOME}"

# Shared secret for the Teams-bot endpoints under test (app and test use the
# same value; the integration suite also reads it from the environment).
export NAO_TEAMS_BOT_SECRET="${NAO_TEAMS_BOT_SECRET:-nao-live-api-test-secret}"

cleanup() {
  if [[ -n "${APP_PID:-}" ]]; then kill "${APP_PID}" 2>/dev/null || true; wait "${APP_PID}" 2>/dev/null || true; fi
  rm -rf "${TEST_HOME}"
}
trap cleanup EXIT

HOME="${TEST_HOME}" "${RELEASE_BIN}" >/tmp/nao-live-api.log 2>&1 &
APP_PID=$!

# Wait for the API server
for i in $(seq 1 60); do
  if curl -sf --max-time 2 http://127.0.0.1:8787/health >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
curl -sf --max-time 2 http://127.0.0.1:8787/health >/dev/null 2>&1 || { echo "[live-api] app did not start"; cat /tmp/nao-live-api.log; exit 1; }
echo "[live-api] app is up"

npx vitest run tests/integration/api.test.ts
