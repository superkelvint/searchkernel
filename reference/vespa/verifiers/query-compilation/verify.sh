#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
    echo "verifier must run from a Search Kernel Git checkout" >&2
    exit 2
}

exec "$ROOT/reference/vespa/m7/verify-m7.sh" "$@"
