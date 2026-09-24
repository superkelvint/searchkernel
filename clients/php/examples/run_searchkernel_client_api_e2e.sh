#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
PYTHON="${CI_PYTHON:-python3}"
TARGET_DIR="${CARGO_TARGET_DIR:-target/host}"
if [[ "$TARGET_DIR" != /* ]]; then
  TARGET_DIR="$ROOT/$TARGET_DIR"
fi
HOST="${SEARCHKERNEL_LOCAL_HOST:-$TARGET_DIR/debug/searchkerneld}"
WORK="$(mktemp -d)"
INDEX="$WORK/showcase-index"
PID=""

cleanup() {
  if [[ -n "$PID" ]] && kill -0 "$PID" 2>/dev/null; then
    kill -INT "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

free_port() {
  "$PYTHON" "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/run_searchkernel_client_api_e2e-python-1.py"
}

wait_port() {
  local port="$1"
  "$PYTHON" "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)/run_searchkernel_client_api_e2e-python-2.py" "$port"
}

start_host() {
  local grpc_port web_port
  grpc_port="$(free_port)"
  web_port="$(free_port)"
  "$HOST" \
    --index "$INDEX" \
    --listen "127.0.0.1:$grpc_port" \
    --web-listen "127.0.0.1:$web_port" \
    >/dev/null 2>&1 &
  PID="$!"
  wait_port "$web_port"
  export SEARCHKERNEL_HTTP_URL="http://127.0.0.1:$web_port"
}

stop_host() {
  kill -INT "$PID"
  wait "$PID"
  PID=""
}

export SEARCHKERNEL_LOCAL_HOST="$HOST"
"$PYTHON" "$ROOT/clients/php/examples/prepare_searchkernel_client_api_e2e.py" "$INDEX"

start_host
php "$ROOT/clients/php/examples/try_searchkernel_client_api_e2e.php" initial
stop_host

start_host
php "$ROOT/clients/php/examples/try_searchkernel_client_api_e2e.php" reopen
stop_host

echo "PHP real-engine E2E + reopen PASS"
