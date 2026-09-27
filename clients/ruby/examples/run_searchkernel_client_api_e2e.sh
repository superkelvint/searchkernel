#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"
source "$ROOT/tools/client-e2e-runtime.sh"
prepare_client_e2e_runtime

HOST="$SEARCHKERNEL_LOCAL_HOST"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT INT TERM

command -v ruby >/dev/null 2>&1 || {
  echo "Ruby client E2E requires ruby" >&2
  exit 1
}

export SEARCHKERNEL_LOCAL_HOST="$HOST"
export SEARCHKERNEL_RUBY_E2E_INDEX="$WORK/index"
ruby "$ROOT/clients/ruby/examples/try_searchkernel_client_api_e2e.rb"
