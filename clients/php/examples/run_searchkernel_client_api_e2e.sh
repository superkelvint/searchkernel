#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

source "$ROOT/tools/client-e2e-runtime.sh"
prepare_client_e2e_runtime

HOST="$SEARCHKERNEL_LOCAL_HOST"
WORK="$(mktemp -d)"
DATA_ROOT="$WORK/data"
PID=""

cleanup() {
  if [[ -n "$PID" ]] && kill -0 "$PID" 2>/dev/null; then
    kill -INT "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

command -v php >/dev/null 2>&1 || {
  echo "PHP client E2E requires php" >&2
  exit 1
}

free_port() {
  php -r '
    $server = stream_socket_server("tcp://127.0.0.1:0", $errno, $errstr);
    if ($server === false) {
        fwrite(STDERR, "failed to allocate local port: {$errstr}\n");
        exit(1);
    }
    $name = stream_socket_get_name($server, false);
    fclose($server);
    $separator = strrpos($name, ":");
    echo substr($name, $separator + 1), PHP_EOL;
  '
}

wait_port() {
  local port="$1"
  php -r '
    $port = (int) $argv[1];
    $deadline = microtime(true) + 60.0;
    do {
        $socket = @fsockopen("127.0.0.1", $port, $errno, $errstr, 0.2);
        if ($socket !== false) {
            fclose($socket);
            exit(0);
        }
        usleep(50_000);
    } while (microtime(true) < $deadline);
    fwrite(STDERR, "searchkerneld web listener did not become ready\n");
    exit(1);
  ' "$port"
}

start_host() {
  local grpc_port web_port
  grpc_port="$(free_port)"
  web_port="$(free_port)"
  "$HOST" \
    --data-root "$DATA_ROOT" \
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
export SEARCHKERNEL_E2E_TARGET="php-e2e"

start_host
php "$ROOT/clients/php/examples/try_searchkernel_client_api_e2e.php" initial
stop_host

start_host
php "$ROOT/clients/php/examples/try_searchkernel_client_api_e2e.php" reopen
stop_host

echo "PHP real-engine multi-index E2E + reopen PASS"
