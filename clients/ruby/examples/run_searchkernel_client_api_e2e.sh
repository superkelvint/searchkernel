#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"
CARGO="$(bash "$ROOT/tools/resolve-rust.sh" cargo)"

./dev native-sdk ensure
"$CARGO" build -p searchkerneld

TARGET_DIR="${CARGO_TARGET_DIR:-target/host}"
if [[ "$TARGET_DIR" != /* ]]; then
  TARGET_DIR="$ROOT/$TARGET_DIR"
fi
HOST="${SEARCHKERNEL_LOCAL_HOST:-$TARGET_DIR/debug/searchkerneld}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

command -v ruby >/dev/null 2>&1 || {
  echo "Ruby client E2E requires ruby" >&2
  exit 1
}

export SEARCHKERNEL_LOCAL_HOST="$HOST"
export SEARCHKERNEL_RUBY_E2E_INDEX="$WORK/index"
ruby "$ROOT/clients/ruby/examples/try_searchkernel_client_api_e2e.rb"
