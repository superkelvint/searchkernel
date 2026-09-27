#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
: "${SEARCHKERNEL_LOCAL_HOST_BINARY:?client E2E host binary was not prepared}"

exec "$ROOT/tools/run-runtime.sh" native "$SEARCHKERNEL_LOCAL_HOST_BINARY" "$@"
