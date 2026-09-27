#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"
PYTHON="$(bash "$ROOT/tools/resolve-python.sh")"

source "$ROOT/tools/client-e2e-runtime.sh"
prepare_client_e2e_runtime

HOST="$SEARCHKERNEL_LOCAL_HOST"
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
  "$PYTHON" - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
}

wait_port() {
  local port="$1"
  "$PYTHON" - "$port" <<'PY'
import socket, sys, time
port = int(sys.argv[1])
deadline = time.monotonic() + 15
while time.monotonic() < deadline:
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.2):
            raise SystemExit(0)
    except OSError:
        time.sleep(0.05)
raise SystemExit("searchkerneld web listener did not become ready")
PY
}

start_host() {
  local grpc_port web_port
  grpc_port="$(free_port)"
  web_port="$(free_port)"
  "$HOST" \
    --index "$INDEX" \
    --listen "127.0.0.1:$grpc_port" \
    --web-listen "127.0.0.1:$web_port" \
    >"$WORK/searchkerneld.log" 2>&1 &
  PID="$!"
  if ! wait_port "$web_port"; then
    echo "searchkerneld failed to start; daemon output:" >&2
    cat "$WORK/searchkerneld.log" >&2
    return 1
  fi
  if ! kill -0 "$PID" 2>/dev/null; then
    echo "searchkerneld exited during startup; daemon output:" >&2
    cat "$WORK/searchkerneld.log" >&2
    return 1
  fi
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
